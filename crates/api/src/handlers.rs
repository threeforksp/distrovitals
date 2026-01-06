//! API request handlers

use crate::SharedState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use distrovitals_analyzer::{Analyzer, DistroHealthSummary, RawMetrics};
use distrovitals_collector::{github::GithubCollector, CollectorConfig};
use serde::{Deserialize, Serialize};
use tracing::error;

#[derive(Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Json<Self> {
        Json(Self {
            success: true,
            data: Some(data),
            error: None,
        })
    }

    pub fn err(message: impl Into<String>) -> (StatusCode, Json<Self>) {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(Self {
                success: false,
                data: None,
                error: Some(message.into()),
            }),
        )
    }
}

/// Health check endpoint
pub async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

/// List all tracked distributions
pub async fn list_distros(State(state): State<SharedState>) -> impl IntoResponse {
    match state.db.get_distributions().await {
        Ok(distros) => ApiResponse::ok(distros).into_response(),
        Err(e) => {
            error!("Failed to list distros: {}", e);
            ApiResponse::<()>::err(e.to_string()).into_response()
        }
    }
}

/// Get a specific distribution by slug
pub async fn get_distro(
    State(state): State<SharedState>,
    Path(slug): Path<String>,
) -> impl IntoResponse {
    match state.db.get_distribution_by_slug(&slug).await {
        Ok(distro) => ApiResponse::ok(distro).into_response(),
        Err(e) => {
            error!("Failed to get distro {}: {}", slug, e);
            (
                StatusCode::NOT_FOUND,
                Json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    error: Some(format!("Distribution not found: {}", slug)),
                }),
            )
                .into_response()
        }
    }
}

/// Get health score for a distribution
pub async fn get_distro_health(
    State(state): State<SharedState>,
    Path(slug): Path<String>,
) -> impl IntoResponse {
    let distro = match state.db.get_distribution_by_slug(&slug).await {
        Ok(d) => d,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    error: Some(format!("Distribution not found: {}", slug)),
                }),
            )
                .into_response()
        }
    };

    match state.db.get_latest_health_score(distro.id).await {
        Ok(Some(score)) => ApiResponse::ok(score).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<()> {
                success: false,
                data: None,
                error: Some("No health score available yet".to_string()),
            }),
        )
            .into_response(),
        Err(e) => {
            error!("Failed to get health score for {}: {}", slug, e);
            ApiResponse::<()>::err(e.to_string()).into_response()
        }
    }
}

#[derive(Deserialize)]
pub struct HistoryQuery {
    #[serde(default = "default_days")]
    days: i32,
}

fn default_days() -> i32 {
    30
}

/// Get health score history for a distribution
pub async fn get_distro_history(
    State(state): State<SharedState>,
    Path(slug): Path<String>,
    Query(query): Query<HistoryQuery>,
) -> impl IntoResponse {
    let distro = match state.db.get_distribution_by_slug(&slug).await {
        Ok(d) => d,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    error: Some(format!("Distribution not found: {}", slug)),
                }),
            )
                .into_response()
        }
    };

    match state.db.get_health_score_history(distro.id, query.days).await {
        Ok(history) => ApiResponse::ok(history).into_response(),
        Err(e) => {
            error!("Failed to get history for {}: {}", slug, e);
            ApiResponse::<()>::err(e.to_string()).into_response()
        }
    }
}

/// Get rankings of all distributions
pub async fn get_rankings(State(state): State<SharedState>) -> impl IntoResponse {
    let distros = match state.db.get_distributions().await {
        Ok(d) => d,
        Err(e) => return ApiResponse::<()>::err(e.to_string()).into_response(),
    };

    let scores = match state.db.get_all_latest_health_scores().await {
        Ok(s) => s,
        Err(e) => return ApiResponse::<()>::err(e.to_string()).into_response(),
    };

    let mut rankings: Vec<DistroHealthSummary> = Vec::new();

    for (idx, score) in scores.into_iter().enumerate() {
        if let Some(d) = distros.iter().find(|d| d.id == score.distro_id) {
            let snapshots = state.db.get_latest_github_snapshots(d.id).await.unwrap_or_default();
            let releases = state.db.get_latest_release_snapshots(d.id).await.unwrap_or_default();
            let community = state.db.get_latest_community_snapshots(d.id).await.unwrap_or_default();
            let metrics = RawMetrics::from_github_snapshots(&snapshots)
                .with_releases(&releases)
                .with_community(&community);

            rankings.push(DistroHealthSummary {
                slug: d.slug.clone(),
                name: d.name.clone(),
                overall_score: score.overall_score,
                development_score: score.development_score,
                community_score: score.community_score,
                maintenance_score: score.maintenance_score,
                trend: score.trend,
                rank: idx + 1,
                metrics,
                github_org: d.github_org.clone(),
                subreddit: d.subreddit.clone(),
                description: d.description.clone(),
            });
        }
    }

    // Add distros without scores
    for distro in &distros {
        if !rankings.iter().any(|r| r.slug == distro.slug) {
            rankings.push(DistroHealthSummary {
                slug: distro.slug.clone(),
                name: distro.name.clone(),
                overall_score: 0.0,
                development_score: 0.0,
                community_score: 0.0,
                maintenance_score: 0.0,
                trend: "unknown".to_string(),
                rank: rankings.len() + 1,
                metrics: RawMetrics::default(),
                github_org: distro.github_org.clone(),
                subreddit: distro.subreddit.clone(),
                description: distro.description.clone(),
            });
        }
    }

    ApiResponse::ok(rankings).into_response()
}

/// Trigger data collection for a distribution (admin endpoint)
pub async fn trigger_collection(
    State(state): State<SharedState>,
    Path(slug): Path<String>,
) -> impl IntoResponse {
    let distro = match state.db.get_distribution_by_slug(&slug).await {
        Ok(d) => d,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    error: Some(format!("Distribution not found: {}", slug)),
                }),
            )
                .into_response()
        }
    };

    // Collect GitHub data if org is configured
    if let Some(ref org) = distro.github_org {
        let config = CollectorConfig::default();
        let collector = match GithubCollector::new(config) {
            Ok(c) => c,
            Err(e) => return ApiResponse::<()>::err(e.to_string()).into_response(),
        };

        if let Err(e) = collector.collect_org_repos(&state.db, distro.id, org).await {
            error!("GitHub collection failed for {}: {}", slug, e);
            return ApiResponse::<()>::err(e.to_string()).into_response();
        }

        // Collect releases
        if let Err(e) = collector.collect_org_releases(&state.db, distro.id, org).await {
            error!("GitHub release collection failed for {}: {}", slug, e);
            // Don't fail the whole request for release errors
        }
    }

    // Calculate new health score
    if let Err(e) = Analyzer::calculate_health_score(&state.db, distro.id).await {
        error!("Health score calculation failed for {}: {}", slug, e);
        return ApiResponse::<()>::err(e.to_string()).into_response();
    }

    #[derive(Serialize)]
    struct CollectionResult {
        message: String,
    }

    ApiResponse::ok(CollectionResult {
        message: format!("Collection completed for {}", slug),
    })
    .into_response()
}
