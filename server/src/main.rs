mod agents;
mod hosts;
mod metrics;

use axum::{
    extract::State,
    http::{header, Request, StatusCode},
    middleware::{self, Next},
    response::Response,
    routing::get,
    Json, Router,
};
use serde::Serialize;
use tower_http::services::ServeDir;

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

#[derive(Clone)]
struct AppState {
    api_token: Option<String>,
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

async fn host_metrics() -> Json<metrics::HostMetrics> {
    Json(metrics::collect_host_metrics().await)
}

async fn hosts(State(_state): State<AppState>) -> Json<Vec<hosts::HostEntry>> {
    Json(hosts::list_hosts().await)
}

async fn agents() -> Json<Vec<agents::AgentInfo>> {
    Json(agents::list_agents().await)
}

/// Auth middleware: if NIGHTKNIGHT_API_TOKEN is set, require Bearer token.
/// Health endpoint is always accessible (no auth required).
async fn auth_middleware(
    State(state): State<AppState>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    // Health endpoint and static files are always accessible
    if req.uri().path() == "/api/health" || !req.uri().path().starts_with("/api/") {
        return Ok(next.run(req).await);
    }

    if let Some(expected_token) = &state.api_token {
        let auth_header = req
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "));

        match auth_header {
            Some(token) if token == expected_token => Ok(next.run(req).await),
            _ => Err(StatusCode::UNAUTHORIZED),
        }
    } else {
        // No token configured - allow all (local-only mode)
        Ok(next.run(req).await)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_token = std::env::var("NIGHTKNIGHT_API_TOKEN").ok();

    if api_token.is_some() {
        println!("API authentication enabled (NIGHTKNIGHT_API_TOKEN set)");
    } else {
        println!("API authentication disabled (local-only mode)");
    }

    let state = AppState { api_token };

    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/metrics", get(host_metrics))
        .route("/api/hosts", get(hosts))
        .route("/api/agents", get(agents))
        .fallback_service(ServeDir::new("dist"))
        .with_state(state.clone())
        .layer(middleware::from_fn_with_state(state, auth_middleware));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080").await?;

    println!("NightKnight local API listening on http://127.0.0.1:8080");
    axum::serve(listener, app).await?;
    Ok(())
}
