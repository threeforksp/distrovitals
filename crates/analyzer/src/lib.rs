//! DistroVitals Health Analyzer
//!
//! Calculates health scores based on collected metrics.

use chrono::Utc;
use distrovitals_database::{Database, GithubSnapshot, HealthScore, NewHealthScore};
use thiserror::Error;
use tracing::info;

#[derive(Error, Debug)]
pub enum AnalyzerError {
    #[error("Database error: {0}")]
    Database(#[from] distrovitals_database::DatabaseError),

    #[error("Insufficient data for analysis")]
    InsufficientData,
}

pub type Result<T> = std::result::Result<T, AnalyzerError>;

/// Health score analyzer
pub struct Analyzer;

impl Analyzer {
    /// Calculate health score for a distribution
    pub async fn calculate_health_score(db: &Database, distro_id: i64) -> Result<i64> {
        let github_snapshots = db.get_latest_github_snapshots(distro_id).await?;
        let previous_score = db.get_latest_health_score(distro_id).await?;

        let development_score = Self::calculate_development_score(&github_snapshots);
        let community_score = Self::calculate_community_score(&github_snapshots);
        let maintenance_score = Self::calculate_maintenance_score(&github_snapshots);

        let overall_score = (development_score * 0.4)
            + (community_score * 0.3)
            + (maintenance_score * 0.3);

        let trend = Self::determine_trend(overall_score, previous_score.as_ref());

        let score = NewHealthScore {
            distro_id,
            overall_score,
            development_score,
            community_score,
            maintenance_score,
            trend,
        };

        let id = db.insert_health_score(score).await?;
        info!(distro_id = distro_id, overall_score = overall_score, "Calculated health score");

        Ok(id)
    }

    /// Calculate development activity score (0-100)
    fn calculate_development_score(github: &[GithubSnapshot]) -> f64 {
        if github.is_empty() {
            return 50.0; // Neutral score when no data
        }

        let total_commits: i64 = github.iter().map(|s| s.commits_30d).sum();
        let total_contributors: i64 = github.iter().map(|s| s.contributors_30d).sum();

        // Score based on activity levels
        let commit_score: f64 = match total_commits {
            0..=10 => 20.0,
            11..=50 => 40.0,
            51..=200 => 60.0,
            201..=500 => 80.0,
            _ => 95.0,
        };

        let contributor_score: f64 = match total_contributors {
            0..=2 => 20.0,
            3..=10 => 40.0,
            11..=30 => 60.0,
            31..=100 => 80.0,
            _ => 95.0,
        };

        (commit_score * 0.6 + contributor_score * 0.4).min(100.0)
    }

    /// Calculate community engagement score (0-100)
    fn calculate_community_score(github: &[GithubSnapshot]) -> f64 {
        if github.is_empty() {
            return 50.0;
        }

        let total_stars: i64 = github.iter().map(|s| s.stars).sum();
        let total_forks: i64 = github.iter().map(|s| s.forks).sum();

        let star_score: f64 = match total_stars {
            0..=100 => 20.0,
            101..=1000 => 40.0,
            1001..=5000 => 60.0,
            5001..=20000 => 80.0,
            _ => 95.0,
        };

        let fork_score: f64 = match total_forks {
            0..=10 => 20.0,
            11..=100 => 40.0,
            101..=500 => 60.0,
            501..=2000 => 80.0,
            _ => 95.0,
        };

        (star_score * 0.5 + fork_score * 0.5).min(100.0)
    }

    /// Calculate maintenance health score (0-100)
    fn calculate_maintenance_score(github: &[GithubSnapshot]) -> f64 {
        if github.is_empty() {
            return 50.0;
        }

        let total_issues: i64 = github.iter().map(|s| s.open_issues).sum();
        let total_prs: i64 = github.iter().map(|s| s.open_prs).sum();

        // Lower open issues/PRs relative to activity is better
        // But some activity is expected for healthy projects
        let issue_score: f64 = match total_issues {
            0..=10 => 90.0,
            11..=50 => 80.0,
            51..=200 => 70.0,
            201..=500 => 50.0,
            501..=1000 => 30.0,
            _ => 20.0,
        };

        let pr_score: f64 = match total_prs {
            0..=5 => 90.0,
            6..=20 => 80.0,
            21..=50 => 70.0,
            51..=100 => 50.0,
            _ => 30.0,
        };

        // Check recency of last commit
        let recency_score: f64 = github
            .iter()
            .filter_map(|s| s.last_commit_at)
            .max()
            .map(|last| {
                let days_ago = (Utc::now() - last).num_days();
                match days_ago {
                    0..=7 => 100.0,
                    8..=30 => 80.0,
                    31..=90 => 60.0,
                    91..=180 => 40.0,
                    _ => 20.0,
                }
            })
            .unwrap_or(50.0);

        (issue_score * 0.3 + pr_score * 0.3 + recency_score * 0.4).min(100.0)
    }

    /// Determine trend based on previous score
    fn determine_trend(current: f64, previous: Option<&HealthScore>) -> String {
        match previous {
            Some(prev) => {
                let diff = current - prev.overall_score;
                if diff > 2.0 {
                    "up".to_string()
                } else if diff < -2.0 {
                    "down".to_string()
                } else {
                    "stable".to_string()
                }
            }
            None => "stable".to_string(),
        }
    }
}

/// Summary of a distribution's health for API responses
#[derive(Debug, Clone, serde::Serialize)]
pub struct DistroHealthSummary {
    pub slug: String,
    pub name: String,
    pub overall_score: f64,
    pub development_score: f64,
    pub community_score: f64,
    pub maintenance_score: f64,
    pub trend: String,
    pub rank: usize,
}
