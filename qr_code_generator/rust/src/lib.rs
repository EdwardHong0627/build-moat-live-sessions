//! Library surface of the QR Code Generator.
//!
//! `main.rs` is a thin wrapper around this crate, and integration tests in `tests/`
//! build the same `Router` against an in-memory database. Keeping the wiring here (not
//! in `main`) is what makes the app testable without binding a TCP port.

pub mod error;
pub mod handlers;
pub mod models;
pub mod repo;
pub mod schemas;
pub mod state;
pub mod token;
pub mod url_validator;

use std::str::FromStr;

use axum::routing::{get, post};
use axum::Router;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{ConnectOptions, SqlitePool};

use crate::state::AppState;

/// Open a SQLite connection pool, creating the database file if it does not exist.
///
/// NOTE on `:memory:` — an in-memory database lives inside a single connection, so
/// callers that want a shared in-memory DB (e.g. tests) must cap `max_connections` at 1.
/// `max_connections` is therefore a parameter rather than hard-coded.
pub async fn init_pool(db_url: &str, max_connections: u32) -> Result<SqlitePool, sqlx::Error> {
    let opts = SqliteConnectOptions::from_str(db_url)?
        .create_if_missing(true)
        .disable_statement_logging();

    SqlitePoolOptions::new()
        .max_connections(max_connections)
        .connect_with(opts)
        .await
}

/// Apply the schema. `raw_sql` runs the multiple `;`-separated statements in one shot.
pub async fn run_migrations(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::raw_sql(include_str!("../migrations/0001_init.sql"))
        .execute(pool)
        .await?;
    Ok(())
}

/// Build the application router (all routes + shared state). The returned `Router` still
/// expects `ConnectInfo<SocketAddr>` to be supplied — `main` does that via
/// `into_make_service_with_connect_info`, tests via a `MockConnectInfo` layer.
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/api/qr/create", post(handlers::create_qr))
        .route("/r/{token}", get(handlers::redirect))
        .route(
            "/api/qr/{token}",
            get(handlers::get_qr_info)
                .patch(handlers::update_qr)
                .delete(handlers::delete_qr),
        )
        .route("/api/qr/{token}/image", get(handlers::get_qr_image))
        .route("/api/qr/{token}/analytics", get(handlers::get_analytics))
        .with_state(state)
}
