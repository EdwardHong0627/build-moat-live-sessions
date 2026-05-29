//! axum handlers — thin glue. Each one validates input, delegates to `repo` /
//! `url_validator` / `token`, and shapes a response. Domain rules (which status code
//! for which situation) live here; SQL does not.

use std::io::Cursor;
use std::net::SocketAddr;

use axum::extract::{ConnectInfo, Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

use crate::error::AppError;
use crate::repo;
use crate::schemas::{
    AnalyticsResponse, CreateRequest, CreateResponse, QRInfoResponse, UpdateRequest,
};
use crate::state::{AppState, BASE_URL};
use crate::url_validator::validate_url;

/// POST /api/qr/create
pub async fn create_qr(
    State(state): State<AppState>,
    Json(req): Json<CreateRequest>,
) -> Result<Json<CreateResponse>, AppError> {
    let normalized = validate_url(&req.url)?;

    let mapping = repo::insert_mapping(
        &state.pool,
        state.token_gen.as_ref(),
        &normalized,
        req.expires_at,
    )
    .await?;

    // Warm the cache so the first scan skips the DB.
    state.cache_put(&mapping.token, &mapping.original_url, mapping.expires_at);

    Ok(Json(CreateResponse {
        short_url: format!("{BASE_URL}/r/{}", mapping.token),
        qr_code_url: format!("{BASE_URL}/api/qr/{}/image", mapping.token),
        original_url: mapping.original_url,
        token: mapping.token,
    }))
}

/// GET /r/{token} — the redirect. Cache -> DB -> 404/410 fallthrough.
pub async fn redirect(
    State(state): State<AppState>,
    Path(token): Path<String>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    // 1. Cache hit: serve immediately — but still honour expiry (the cache carries it).
    if let Some(target) = state.cache_get(&token) {
        if target.is_expired() {
            state.cache_invalidate(&token);
            return Err(AppError::Gone("This link has expired".into())); // -> 410
        }
        record_scan(&state, &token, &headers, addr).await;
        return Ok(found(&target.url));
    }

    // 2. Cache miss: consult the DB and classify.
    let mapping = repo::find_by_token(&state.pool, &token)
        .await?
        .ok_or(AppError::NotFound)?; // never existed -> 404

    if mapping.is_deleted {
        return Err(AppError::Gone("This link has been deleted".into())); // -> 410
    }
    if mapping.is_expired() {
        return Err(AppError::Gone("This link has expired".into())); // -> 410
    }

    // 3. Valid: backfill the cache and redirect.
    state.cache_put(&mapping.token, &mapping.original_url, mapping.expires_at);
    record_scan(&state, &token, &headers, addr).await;
    Ok(found(&mapping.original_url))
}

/// GET /api/qr/{token}
pub async fn get_qr_info(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Result<Json<QRInfoResponse>, AppError> {
    let mapping = get_active_or_404(&state, &token).await?;
    Ok(Json(mapping.into()))
}

/// PATCH /api/qr/{token}
pub async fn update_qr(
    State(state): State<AppState>,
    Path(token): Path<String>,
    Json(req): Json<UpdateRequest>,
) -> Result<Json<QRInfoResponse>, AppError> {
    let mut mapping = get_active_or_404(&state, &token).await?;

    if let Some(url) = req.url {
        mapping.original_url = validate_url(&url)?;
        // The cached value is now stale — drop it so the next scan re-resolves.
        state.cache_invalidate(&token);
    }
    if let Some(expires_at) = req.expires_at {
        mapping.expires_at = Some(expires_at);
    }

    repo::update_mapping(&state.pool, &mapping).await?;

    // Re-read to return canonical timestamps.
    let fresh = get_active_or_404(&state, &token).await?;
    Ok(Json(fresh.into()))
}

/// DELETE /api/qr/{token} — soft delete.
pub async fn delete_qr(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    // 404 if it doesn't exist or is already deleted.
    get_active_or_404(&state, &token).await?;
    repo::soft_delete(&state.pool, &token).await?;
    state.cache_invalidate(&token);
    Ok(Json(json!({ "detail": "Deleted" })))
}

/// GET /api/qr/{token}/image — PNG of the QR code encoding the short URL.
pub async fn get_qr_image(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Result<Response, AppError> {
    get_active_or_404(&state, &token).await?;

    let short_url = format!("{BASE_URL}/r/{token}");
    let png = render_qr_png(&short_url)?;

    Ok(([(header::CONTENT_TYPE, "image/png")], png).into_response())
}

/// GET /api/qr/{token}/analytics
pub async fn get_analytics(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Result<Json<AnalyticsResponse>, AppError> {
    get_active_or_404(&state, &token).await?;

    let total = repo::count_scans(&state.pool, &token).await?;
    let daily = repo::scans_by_day(&state.pool, &token).await?;

    Ok(Json(AnalyticsResponse {
        token,
        total_scans: total,
        scans_by_day: daily.into_iter().map(Into::into).collect(),
    }))
}

// ---- helpers --------------------------------------------------------------

/// Fetch a mapping that exists AND is not soft-deleted, else 404.
/// Mirrors the reference's `_get_mapping_or_404` (management endpoints treat a deleted
/// link as simply "not there"; only the redirect path distinguishes 410).
async fn get_active_or_404(
    state: &AppState,
    token: &str,
) -> Result<crate::models::UrlMapping, AppError> {
    let mapping = repo::find_by_token(&state.pool, token)
        .await?
        .ok_or(AppError::NotFound)?;
    if mapping.is_deleted {
        return Err(AppError::NotFound);
    }
    Ok(mapping)
}

/// Build a 302 Found redirect. (axum's `Redirect::temporary` emits 307; the spec wants
/// 302, so we set the status and Location header explicitly.)
fn found(location: &str) -> Response {
    (StatusCode::FOUND, [(header::LOCATION, location.to_string())]).into_response()
}

/// Best-effort scan logging — a logging failure must not break the redirect.
async fn record_scan(state: &AppState, token: &str, headers: &HeaderMap, addr: SocketAddr) {
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok());
    let ip = addr.ip().to_string();

    if let Err(e) = repo::insert_scan(&state.pool, token, user_agent, Some(&ip)).await {
        eprintln!("[scan log] failed to record scan for {token}: {e:?}");
    }
}

/// Render a QR code for `data` into PNG bytes.
///
/// Built from the raw module matrix (`to_colors`) so we don't depend on qrcode's
/// optional image integration — each dark module becomes an N×N black block with a
/// quiet-zone border, drawn onto a grayscale `image` buffer.
fn render_qr_png(data: &str) -> Result<Vec<u8>, AppError> {
    use image::{DynamicImage, GrayImage, ImageFormat, Luma};
    use qrcode::{Color, QrCode};

    let code = QrCode::new(data.as_bytes())
        .map_err(|e| AppError::Internal(format!("QR encode failed: {e}")))?;

    let width = code.width(); // modules per side
    let colors = code.to_colors();

    let scale: u32 = 8; // pixels per module
    let quiet: u32 = 4; // quiet-zone modules around the code
    let dim = (width as u32 + quiet * 2) * scale;

    // White background; we paint dark modules black.
    let mut img: GrayImage = GrayImage::from_pixel(dim, dim, Luma([255]));

    for y in 0..width {
        for x in 0..width {
            if colors[y * width + x] == Color::Dark {
                let x0 = (quiet + x as u32) * scale;
                let y0 = (quiet + y as u32) * scale;
                for dy in 0..scale {
                    for dx in 0..scale {
                        img.put_pixel(x0 + dx, y0 + dy, Luma([0]));
                    }
                }
            }
        }
    }

    let mut buf = Vec::new();
    DynamicImage::ImageLuma8(img)
        .write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
        .map_err(|e| AppError::Internal(format!("PNG encode failed: {e}")))?;
    Ok(buf)
}
