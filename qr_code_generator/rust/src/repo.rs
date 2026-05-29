//! REPOSITORY PATTERN
//!
//! Every SQL statement in the app lives here. Handlers call these functions and stay
//! declarative ("get the mapping or 404"), never touching query strings. That keeps
//! the storage choice (SQLite, today) swappable and the SQL in one auditable place.
//!
//! Queries are runtime-checked (`sqlx::query` / `query_as`) so no live DB is needed at
//! compile time.

use chrono::Utc;
use sqlx::SqlitePool;

use crate::error::AppError;
use crate::models::{DailyScan, UrlMapping};
use crate::token::TokenGenerator;

const MAX_TOKEN_RETRIES: usize = 10;

/// Insert a new mapping, generating tokens until one is free.
///
/// The UNIQUE constraint on `token` is the source of truth: we attempt the INSERT and,
/// if SQLite reports a unique violation, ask the generator for another token. This is
/// race-free even under concurrency — the DB, not a prior SELECT, decides.
pub async fn insert_mapping(
    pool: &SqlitePool,
    generator: &dyn TokenGenerator,
    original_url: &str,
    expires_at: Option<chrono::DateTime<Utc>>,
) -> Result<UrlMapping, AppError> {
    for _ in 0..MAX_TOKEN_RETRIES {
        let token = generator.generate();
        let now = Utc::now();

        let result = sqlx::query(
            "INSERT INTO url_mappings (token, original_url, created_at, updated_at, expires_at, is_deleted)
             VALUES (?, ?, ?, ?, ?, 0)",
        )
        .bind(token.as_str())
        .bind(original_url)
        .bind(now)
        .bind(now)
        .bind(expires_at)
        .execute(pool)
        .await;

        match result {
            Ok(_) => {
                return Ok(UrlMapping {
                    id: 0, // not surfaced to clients; avoid an extra round-trip
                    token: token.into_inner(),
                    original_url: original_url.to_string(),
                    created_at: now,
                    updated_at: now,
                    expires_at,
                    is_deleted: false,
                });
            }
            Err(sqlx::Error::Database(db_err)) if db_err.is_unique_violation() => {
                continue; // token clash — try a fresh one
            }
            Err(e) => return Err(AppError::Db(e)),
        }
    }

    Err(AppError::Internal(format!(
        "failed to generate a unique token after {MAX_TOKEN_RETRIES} retries"
    )))
}

/// Fetch a mapping by token, including soft-deleted ones (callers decide what to do).
pub async fn find_by_token(
    pool: &SqlitePool,
    token: &str,
) -> Result<Option<UrlMapping>, AppError> {
    let row = sqlx::query_as::<_, UrlMapping>("SELECT * FROM url_mappings WHERE token = ?")
        .bind(token)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

/// Persist the mutable fields of an updated mapping.
pub async fn update_mapping(pool: &SqlitePool, m: &UrlMapping) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE url_mappings SET original_url = ?, expires_at = ?, updated_at = ? WHERE token = ?",
    )
    .bind(&m.original_url)
    .bind(m.expires_at)
    .bind(Utc::now())
    .bind(&m.token)
    .execute(pool)
    .await?;
    Ok(())
}

/// Soft delete: flip the flag, keep the row (so scans can still distinguish 410 vs 404).
pub async fn soft_delete(pool: &SqlitePool, token: &str) -> Result<(), AppError> {
    sqlx::query("UPDATE url_mappings SET is_deleted = 1, updated_at = ? WHERE token = ?")
        .bind(Utc::now())
        .bind(token)
        .execute(pool)
        .await?;
    Ok(())
}

/// Append a scan event. Best-effort fields (user agent / IP) are nullable.
pub async fn insert_scan(
    pool: &SqlitePool,
    token: &str,
    user_agent: Option<&str>,
    ip_address: Option<&str>,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO scan_events (token, scanned_at, user_agent, ip_address) VALUES (?, ?, ?, ?)",
    )
    .bind(token)
    .bind(Utc::now())
    .bind(user_agent)
    .bind(ip_address)
    .execute(pool)
    .await?;
    Ok(())
}

/// Total scans for a token.
pub async fn count_scans(pool: &SqlitePool, token: &str) -> Result<i64, AppError> {
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM scan_events WHERE token = ?")
        .bind(token)
        .fetch_one(pool)
        .await?;
    Ok(total)
}

/// Scans grouped by calendar day. Uses `substr(..,1,10)` to take the date portion of
/// the RFC3339 timestamp — robust regardless of how SQLite parses the `Z`/offset.
pub async fn scans_by_day(pool: &SqlitePool, token: &str) -> Result<Vec<DailyScan>, AppError> {
    let rows = sqlx::query_as::<_, DailyScan>(
        "SELECT substr(scanned_at, 1, 10) AS date, COUNT(*) AS count
         FROM scan_events WHERE token = ?
         GROUP BY date ORDER BY date",
    )
    .bind(token)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
