//! DistroVitals Health Analyzer
//!
//! Calculates health scores based on collected metrics.

use chrono::Utc;
use distrovitals_database::{
    CommunitySnapshot, Database, GithubSnapshot, HealthScore, NewHealthScore, ReleaseSnapshot,
};
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
        let community_snapshots = db.get_latest_community_snapshots(distro_id).await?;
        let previous_score = db.get_latest_health_score(distro_id).await?;

        let development_score = Self::calculate_development_score(&github_snapshots);
        let community_score = Self::calculate_community_score(&github_snapshots, &community_snapshots);
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
    /// Combines GitHub metrics (stars, forks) with Reddit community data
    fn calculate_community_score(github: &[GithubSnapshot], community: &[CommunitySnapshot]) -> f64 {
        // GitHub component (stars + forks)
        let github_score = if github.is_empty() {
            50.0
        } else {
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

            star_score * 0.5 + fork_score * 0.5
        };

        // Reddit component (subscribers + activity)
        let reddit_score = Self::calculate_reddit_score(community);

        // Weight: 40% GitHub, 60% Reddit (Reddit is better indicator of user community)
        // If no Reddit data, use 100% GitHub
        if reddit_score > 0.0 {
            (github_score * 0.4 + reddit_score * 0.6).min(100.0)
        } else {
            github_score.min(100.0)
        }
    }

    /// Calculate Reddit community score based on subscribers and activity
    fn calculate_reddit_score(community: &[CommunitySnapshot]) -> f64 {
        // Find Reddit snapshots
        let reddit_snapshots: Vec<_> = community
            .iter()
            .filter(|c| c.source.starts_with("reddit:"))
            .collect();

        if reddit_snapshots.is_empty() {
            return 0.0; // No Reddit data
        }

        // Sum subscribers across all Reddit sources (usually just one subreddit)
        let total_subscribers: i64 = reddit_snapshots
            .iter()
            .filter_map(|s| s.active_users_30d)
            .sum();

        // Sum recent posts
        let total_posts: i64 = reddit_snapshots
            .iter()
            .filter_map(|s| s.posts_30d)
            .sum();

        // Score based on subscriber count
        // Linux distro subreddits range from ~1k to ~350k
        let subscriber_score: f64 = match total_subscribers {
            0..=1000 => 20.0,
            1001..=5000 => 30.0,
            5001..=15000 => 45.0,
            15001..=50000 => 60.0,
            50001..=100000 => 75.0,
            100001..=200000 => 85.0,
            _ => 95.0, // 200k+ (Arch, Ubuntu territory)
        };

        // Score based on recent activity (posts in last 30 days)
        let activity_score: f64 = match total_posts {
            0..=10 => 20.0,
            11..=30 => 40.0,
            31..=60 => 60.0,
            61..=100 => 80.0,
            _ => 95.0,
        };

        // Weight: 70% subscribers, 30% activity
        subscriber_score * 0.7 + activity_score * 0.3
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

/// Raw metrics aggregated from snapshots
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct RawMetrics {
    pub repos_tracked: i64,
    pub total_stars: i64,
    pub total_forks: i64,
    pub total_contributors: i64,
    pub commits_30d: i64,
    pub open_issues: i64,
    pub open_prs: i64,
    pub total_releases: i64,
    pub releases_30d: i64,
    pub latest_release: Option<String>,
    pub days_since_release: Option<i64>,
    // Reddit metrics
    pub reddit_subscribers: i64,
    pub reddit_posts_30d: i64,
    pub subreddit: Option<String>,
}

impl RawMetrics {
    /// Aggregate metrics from GitHub snapshots
    pub fn from_github_snapshots(snapshots: &[GithubSnapshot]) -> Self {
        Self {
            repos_tracked: snapshots.len() as i64,
            total_stars: snapshots.iter().map(|s| s.stars).sum(),
            total_forks: snapshots.iter().map(|s| s.forks).sum(),
            total_contributors: snapshots.iter().map(|s| s.contributors_30d).sum(),
            commits_30d: snapshots.iter().map(|s| s.commits_30d).sum(),
            open_issues: snapshots.iter().map(|s| s.open_issues).sum(),
            open_prs: snapshots.iter().map(|s| s.open_prs).sum(),
            total_releases: 0,
            releases_30d: 0,
            latest_release: None,
            days_since_release: None,
            reddit_subscribers: 0,
            reddit_posts_30d: 0,
            subreddit: None,
        }
    }

    /// Add Reddit community metrics
    pub fn with_community(mut self, community: &[CommunitySnapshot]) -> Self {
        // Find Reddit snapshots
        for snap in community.iter().filter(|c| c.source.starts_with("reddit:")) {
            if let Some(subs) = snap.active_users_30d {
                self.reddit_subscribers += subs;
            }
            if let Some(posts) = snap.posts_30d {
                self.reddit_posts_30d += posts;
            }
            // Extract subreddit name from source (e.g., "reddit:r/archlinux" -> "archlinux")
            if self.subreddit.is_none() {
                self.subreddit = snap.source.strip_prefix("reddit:r/").map(String::from);
            }
        }
        self
    }

    /// Add release metrics
    pub fn with_releases(mut self, releases: &[ReleaseSnapshot]) -> Self {
        self.total_releases = releases.len() as i64;

        // Count releases in last 30 days
        let thirty_days_ago = Utc::now() - chrono::TimeDelta::days(30);
        self.releases_30d = releases
            .iter()
            .filter(|r| !r.is_prerelease)
            .filter(|r| r.published_at.map(|d| d > thirty_days_ago).unwrap_or(false))
            .count() as i64;

        // Find latest non-prerelease
        if let Some(latest) = releases
            .iter()
            .filter(|r| !r.is_prerelease)
            .max_by_key(|r| r.published_at)
        {
            self.latest_release = Some(latest.tag_name.clone());
            if let Some(published) = latest.published_at {
                self.days_since_release = Some((Utc::now() - published).num_days());
            }
        }

        self
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
    pub metrics: RawMetrics,
    pub github_org: Option<String>,
    pub subreddit: Option<String>,
}
