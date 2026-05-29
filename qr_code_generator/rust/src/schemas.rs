//! Request/response DTOs (the API boundary), separate from the DB `models`.
//!
//! Keeping these distinct from `UrlMapping` means the wire format can evolve
//! independently of the table layout (e.g. we never expose the internal `id`).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::models::{DailyScan, UrlMapping};

#[derive(Debug, Deserialize)]
pub struct CreateRequest {
    pub url: String,
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct CreateResponse {
    pub token: String,
    pub short_url: String,
    pub qr_code_url: String,
    pub original_url: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateRequest {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct QRInfoResponse {
    pub token: String,
    pub original_url: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub is_deleted: bool,
}

// Ergonomic conversion: `mapping.into()` builds the public response.
impl From<UrlMapping> for QRInfoResponse {
    fn from(m: UrlMapping) -> Self {
        Self {
            token: m.token,
            original_url: m.original_url,
            created_at: m.created_at,
            updated_at: m.updated_at,
            expires_at: m.expires_at,
            is_deleted: m.is_deleted,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct DailyScanResponse {
    pub date: String,
    pub count: i64,
}

impl From<DailyScan> for DailyScanResponse {
    fn from(d: DailyScan) -> Self {
        Self { date: d.date, count: d.count }
    }
}

#[derive(Debug, Serialize)]
pub struct AnalyticsResponse {
    pub token: String,
    pub total_scans: i64,
    pub scans_by_day: Vec<DailyScanResponse>,
}
