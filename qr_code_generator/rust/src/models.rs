//! Domain rows as Rust structs.
//!
//! `#[derive(FromRow)]` lets sqlx map a query result straight into these structs by
//! column name. Timestamps use `chrono::DateTime<Utc>` (sqlx's `chrono` feature
//! handles TEXT <-> DateTime), and the SQLite `0/1` INTEGER decodes into `bool`.

use chrono::{DateTime, Utc};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct UrlMapping {
    // Primary key — read by `FromRow` (SELECT *) but not surfaced to clients.
    #[allow(dead_code)]
    pub id: i64,
    pub token: String,
    pub original_url: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub is_deleted: bool,
}

impl UrlMapping {
    /// True when an expiry is set and already in the past.
    pub fn is_expired(&self) -> bool {
        matches!(self.expires_at, Some(exp) if exp < Utc::now())
    }
}

/// One row in the per-day analytics rollup (`substr(scanned_at, 1, 10)` => "YYYY-MM-DD").
#[derive(Debug, Clone, FromRow)]
pub struct DailyScan {
    pub date: String,
    pub count: i64,
}
