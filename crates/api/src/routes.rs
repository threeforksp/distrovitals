//! API route definitions

use crate::handlers;
use crate::SharedState;
use axum::{
    routing::{get, post},
    Router,
};
use std::path::PathBuf;
use tower_http::{
    compression::CompressionLayer,
    cors::{Any, CorsLayer},
    services::ServeDir,
};

/// Create the main application router
pub fn create_router(state: SharedState, static_dir: Option<PathBuf>) -> Router {
    let api_routes = Router::new()
        .route("/health", get(handlers::health_check))
        .route("/distros", get(handlers::list_distros))
        .route("/distros/{slug}", get(handlers::get_distro))
        .route("/distros/{slug}/health", get(handlers::get_distro_health))
        .route("/distros/{slug}/history", get(handlers::get_distro_history))
        .route("/rankings", get(handlers::get_rankings))
        .route("/collect/{slug}", post(handlers::trigger_collection))
        .with_state(state);

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let mut app = Router::new()
        .nest("/api/v1", api_routes)
        .layer(cors)
        .layer(CompressionLayer::new());

    // Serve static files if directory provided
    if let Some(dir) = static_dir {
        app = app.fallback_service(ServeDir::new(dir).append_index_html_on_directories(true));
    }

    app
}
