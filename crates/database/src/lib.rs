//! DistroVitals Database Layer
//!
//! SQLite-based storage for distribution health metrics.

mod models;
mod queries;
mod schema;

pub use models::*;
pub use schema::Database;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("Database connection failed: {0}")]
    Connection(#[from] sqlx::Error),

    #[error("Migration failed: {0}")]
    Migration(String),

    #[error("Record not found: {0}")]
    NotFound(String),
}

pub type Result<T> = std::result::Result<T, DatabaseError>;
