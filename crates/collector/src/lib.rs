//! DistroVitals Data Collectors
//!
//! Fetches metrics from various sources (GitHub, Reddit, package repos, etc.)

pub mod github;
pub mod reddit;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum CollectorError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Rate limited, retry after {0} seconds")]
    RateLimited(u64),

    #[error("API error: {0}")]
    Api(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Database error: {0}")]
    Database(#[from] distrovitals_database::DatabaseError),
}

pub type Result<T> = std::result::Result<T, CollectorError>;

/// Configuration for collectors
#[derive(Debug, Clone)]
pub struct CollectorConfig {
    pub github_token: Option<String>,
    pub user_agent: String,
}

impl Default for CollectorConfig {
    fn default() -> Self {
        Self {
            github_token: std::env::var("GITHUB_TOKEN").ok(),
            user_agent: "DistroVitals/0.1 (https://distrovitals.org)".to_string(),
        }
    }
}
