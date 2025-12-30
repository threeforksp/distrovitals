//! DistroVitals Web API
//!
//! Axum-based REST API and static file server.

mod handlers;
mod routes;

pub use routes::create_router;

use distrovitals_database::Database;
use std::sync::Arc;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub db: Database,
}

impl AppState {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

pub type SharedState = Arc<AppState>;
