//! Database models for DistroVitals

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A Linux distribution being tracked
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Distribution {
    pub id: i64,
    pub name: String,
    pub slug: String,
    pub homepage: Option<String>,
    pub github_org: Option<String>,
    pub gitlab_group: Option<String>,
    pub subreddit: Option<String>,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// GitHub repository metrics snapshot
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct GithubSnapshot {
    pub id: i64,
    pub distro_id: i64,
    pub repo_name: String,
    pub stars: i64,
    pub forks: i64,
    pub open_issues: i64,
    pub open_prs: i64,
    pub commits_30d: i64,
    pub commits_365d: i64,
    pub contributors_30d: i64,
    pub last_commit_at: Option<DateTime<Utc>>,
    pub collected_at: DateTime<Utc>,
}

/// Package repository metrics snapshot
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PackageSnapshot {
    pub id: i64,
    pub distro_id: i64,
    pub total_packages: i64,
    pub outdated_packages: i64,
    pub security_updates: i64,
    pub collected_at: DateTime<Utc>,
}

/// Community metrics snapshot (forums, mailing lists, etc.)
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CommunitySnapshot {
    pub id: i64,
    pub distro_id: i64,
    pub source: String,
    pub active_users_30d: Option<i64>,
    pub posts_30d: Option<i64>,
    pub response_time_avg_hours: Option<f64>,
    pub collected_at: DateTime<Utc>,
}

/// Calculated health score for a distribution
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct HealthScore {
    pub id: i64,
    pub distro_id: i64,
    pub overall_score: f64,
    pub development_score: f64,
    pub community_score: f64,
    pub maintenance_score: f64,
    pub trend: String, // "up", "down", "stable"
    pub calculated_at: DateTime<Utc>,
}

/// Input for creating a new distribution
#[derive(Debug, Clone, Deserialize)]
pub struct NewDistribution {
    pub name: String,
    pub slug: String,
    pub homepage: Option<String>,
    pub github_org: Option<String>,
    pub gitlab_group: Option<String>,
    pub subreddit: Option<String>,
    pub description: Option<String>,
}

/// Input for creating a community snapshot
#[derive(Debug, Clone)]
pub struct NewCommunitySnapshot {
    pub distro_id: i64,
    pub source: String,
    pub active_users_30d: Option<i64>,
    pub posts_30d: Option<i64>,
    pub response_time_avg_hours: Option<f64>,
}

/// Input for creating a GitHub snapshot
#[derive(Debug, Clone)]
pub struct NewGithubSnapshot {
    pub distro_id: i64,
    pub repo_name: String,
    pub stars: i64,
    pub forks: i64,
    pub open_issues: i64,
    pub open_prs: i64,
    pub commits_30d: i64,
    pub commits_365d: i64,
    pub contributors_30d: i64,
    pub last_commit_at: Option<DateTime<Utc>>,
}

/// Input for creating a health score
#[derive(Debug, Clone)]
pub struct NewHealthScore {
    pub distro_id: i64,
    pub overall_score: f64,
    pub development_score: f64,
    pub community_score: f64,
    pub maintenance_score: f64,
    pub trend: String,
}

/// Release snapshot from GitHub
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ReleaseSnapshot {
    pub id: i64,
    pub distro_id: i64,
    pub repo_name: String,
    pub tag_name: String,
    pub release_name: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
    pub is_prerelease: bool,
    pub collected_at: DateTime<Utc>,
}

/// Input for creating a release snapshot
#[derive(Debug, Clone)]
pub struct NewReleaseSnapshot {
    pub distro_id: i64,
    pub repo_name: String,
    pub tag_name: String,
    pub release_name: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
    pub is_prerelease: bool,
}
