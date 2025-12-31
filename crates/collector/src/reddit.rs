//! Reddit API collector for community metrics

use crate::{CollectorConfig, CollectorError, Result};
use distrovitals_database::{Database, NewCommunitySnapshot};
use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, info, warn};

/// Reddit API client
pub struct RedditCollector {
    client: Client,
}

#[derive(Debug, Deserialize)]
struct SubredditResponse {
    data: SubredditData,
}

#[derive(Debug, Deserialize)]
struct SubredditData {
    display_name: String,
    subscribers: i64,
    accounts_active: Option<i64>,
    #[serde(default)]
    active_user_count: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct ListingResponse {
    data: ListingData,
}

#[derive(Debug, Deserialize)]
struct ListingData {
    children: Vec<PostWrapper>,
}

#[derive(Debug, Deserialize)]
struct PostWrapper {
    data: PostData,
}

#[derive(Debug, Deserialize)]
struct PostData {
    created_utc: f64,
    num_comments: i64,
}

impl RedditCollector {
    /// Create a new Reddit collector
    pub fn new(_config: CollectorConfig) -> Result<Self> {
        let client = Client::builder()
            .user_agent("DistroVitals/0.1 (Linux distribution health tracker)")
            .build()?;

        Ok(Self { client })
    }

    /// Collect metrics for a subreddit
    pub async fn collect_subreddit(
        &self,
        db: &Database,
        distro_id: i64,
        subreddit: &str,
    ) -> Result<i64> {
        info!(subreddit = subreddit, "Collecting Reddit metrics");

        // Get subreddit info
        let about_url = format!("https://www.reddit.com/r/{}/about.json", subreddit);
        let response = self.client.get(&about_url).send().await?;

        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(CollectorError::RateLimited(60));
        }

        if !response.status().is_success() {
            return Err(CollectorError::Api(format!(
                "Reddit API error: {} for r/{}",
                response.status(),
                subreddit
            )));
        }

        let about: SubredditResponse = response.json().await?;
        let subscribers = about.data.subscribers;
        let active_users = about.data.accounts_active.or(about.data.active_user_count);

        // Get recent posts to count activity
        let posts_30d = self.count_recent_posts(subreddit, 30).await.unwrap_or(0);

        debug!(
            subreddit = subreddit,
            subscribers = subscribers,
            active_users = ?active_users,
            posts_30d = posts_30d,
            "Collected Reddit metrics"
        );

        let snapshot = NewCommunitySnapshot {
            distro_id,
            source: format!("reddit:r/{}", subreddit),
            active_users_30d: Some(subscribers), // Using subscribers as proxy
            posts_30d: Some(posts_30d),
            response_time_avg_hours: None, // Could calculate from comment times
        };

        let id = db.insert_community_snapshot(snapshot).await?;
        info!(subreddit = subreddit, subscribers = subscribers, "Collected Reddit snapshot");

        Ok(id)
    }

    /// Count posts in the last N days
    async fn count_recent_posts(&self, subreddit: &str, days: i64) -> Result<i64> {
        let url = format!(
            "https://www.reddit.com/r/{}/new.json?limit=100",
            subreddit
        );

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Ok(0);
        }

        let listing: ListingResponse = response.json().await?;

        let now = chrono::Utc::now().timestamp() as f64;
        let cutoff = now - (days as f64 * 86400.0);

        let count = listing
            .data
            .children
            .iter()
            .filter(|p| p.data.created_utc >= cutoff)
            .count() as i64;

        Ok(count)
    }

    /// Collect metrics for all distributions with subreddits
    pub async fn collect_all(&self, db: &Database) -> Result<Vec<i64>> {
        let distros = db.get_distributions().await?;
        let mut snapshot_ids = Vec::new();

        for distro in distros {
            if let Some(ref subreddit) = distro.subreddit {
                match self.collect_subreddit(db, distro.id, subreddit).await {
                    Ok(id) => snapshot_ids.push(id),
                    Err(e) => {
                        warn!(
                            distro = distro.slug,
                            subreddit = subreddit,
                            error = %e,
                            "Failed to collect Reddit metrics"
                        );
                        // Rate limit - wait before continuing
                        if matches!(e, CollectorError::RateLimited(_)) {
                            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                        }
                    }
                }
                // Reddit rate limiting - be gentle
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            }
        }

        info!(count = snapshot_ids.len(), "Collected Reddit snapshots");
        Ok(snapshot_ids)
    }
}
