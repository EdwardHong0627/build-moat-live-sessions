//! Bootstrap: open the SQLite pool, run the schema migration, build the Router, serve.
//! All wiring lives in the library (`lib.rs`) so it can be exercised by integration tests.

use std::net::SocketAddr;

use qr_code_generator::state::AppState;
use qr_code_generator::{build_router, init_pool, run_migrations};

const DB_URL: &str = "sqlite://qr_code.db";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = init_pool(DB_URL, 5).await?;
    run_migrations(&pool).await?;

    let app = build_router(AppState::new(pool));

    let addr = SocketAddr::from(([0, 0, 0, 0], 8000));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("QR Code Generator listening on http://{addr}");

    // `ConnectInfo` requires the connect-info make-service so handlers can read the
    // client socket address (used for scan-event IP logging).
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}
