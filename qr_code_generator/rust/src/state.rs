//! SHARED-STATE PATTERN
//!
//! axum hands every handler a clone of `AppState` via the `State` extractor, so the
//! state must be cheap to clone and safe to share across threads/tasks.
//!
//!   * `SqlitePool` is already an `Arc` internally — cloning it just bumps a refcount.
//!   * The redirect cache is wrapped in `Arc<RwLock<..>>` so all clones point at the
//!     SAME map. `RwLock` (not `Mutex`) lets many redirects read concurrently while
//!     writes (create/update/delete) take the exclusive lock briefly.
//!
//! In production this in-memory map would be Redis; the trait-free `HashMap` keeps the
//! prototype dependency-light while modelling the cache → DB fallthrough.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

use crate::token::{RandomTokenGenerator, TokenGenerator};

/// Public base URL used to build short links and the QR image URL.
pub const BASE_URL: &str = "http://localhost:8000";

/// What we cache per token. We keep the expiry alongside the URL so a cache hit can
/// make the SAME decision as the DB path — otherwise an expired link that was cached
/// (e.g. warmed at create time) would be served as a 302 instead of a 410.
#[derive(Clone)]
pub struct CachedTarget {
    pub url: String,
    pub expires_at: Option<DateTime<Utc>>,
}

impl CachedTarget {
    pub fn is_expired(&self) -> bool {
        matches!(self.expires_at, Some(exp) if exp < Utc::now())
    }
}

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    /// token -> target. A hit lets us serve a 302 without touching SQLite.
    pub cache: Arc<RwLock<HashMap<String, CachedTarget>>>,
    /// Boxed behind a trait object so the token strategy is swappable (e.g. in tests).
    pub token_gen: Arc<dyn TokenGenerator>,
}

impl AppState {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            cache: Arc::new(RwLock::new(HashMap::new())),
            token_gen: Arc::new(RandomTokenGenerator),
        }
    }

    pub fn cache_put(&self, token: &str, url: &str, expires_at: Option<DateTime<Utc>>) {
        if let Ok(mut guard) = self.cache.write() {
            guard.insert(
                token.to_string(),
                CachedTarget { url: url.to_string(), expires_at },
            );
        }
    }

    pub fn cache_get(&self, token: &str) -> Option<CachedTarget> {
        self.cache.read().ok().and_then(|g| g.get(token).cloned())
    }

    pub fn cache_invalidate(&self, token: &str) {
        if let Ok(mut guard) = self.cache.write() {
            guard.remove(token);
        }
    }
}
