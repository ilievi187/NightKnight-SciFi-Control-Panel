mod metrics;

use axum::{routing::get, Json, Router};
use serde::Serialize;

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

async fn host_metrics() -> Json<metrics::HostMetrics> {
    Json(metrics::collect_host_metrics().await)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/metrics", get(host_metrics));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080").await?;

    println!("NightKnight local API listening on http://127.0.0.1:8080");
    axum::serve(listener, app).await?;
    Ok(())
}
