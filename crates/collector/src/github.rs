//! GitHub API collector

use crate::{CollectorConfig, CollectorError, Result};
use chrono::{DateTime, Utc};
use distrovitals_database::{Database, NewGithubSnapshot, NewReleaseSnapshot};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};
use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, info, warn};

/// GitHub API client
pub struct GithubCollector {
    client: Client,
    #[allow(dead_code)]
    config: CollectorConfig,
}

#[derive(Debug, Deserialize)]
struct RepoResponse {
    name: String,
    stargazers_count: i64,
    forks_count: i64,
    open_issues_count: i64,
    pushed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
struct CommitResponse {
    #[allow(dead_code)]
    sha: String,
}

#[derive(Debug, Deserialize)]
struct ReleaseResponse {
    tag_name: String,
    name: Option<String>,
    published_at: Option<DateTime<Utc>>,
    prerelease: bool,
}

impl GithubCollector {
    /// Create a new GitHub collector
    pub fn new(config: CollectorConfig) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static("application/vnd.github.v3+json"));
        headers.insert(USER_AGENT, HeaderValue::from_str(&config.user_agent).unwrap());

        if let Some(ref token) = config.github_token {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
            );
        }

        let client = Client::builder().default_headers(headers).build()?;

        Ok(Self { client, config })
    }

    /// Collect metrics for a GitHub organization's repositories
    pub async fn collect_org_repos(
        &self,
        db: &Database,
        distro_id: i64,
        org: &str,
    ) -> Result<Vec<i64>> {
        info!(org = org, "Collecting GitHub metrics");

        let repos = self.get_org_repos(org).await?;
        let mut snapshot_ids = Vec::new();

        for repo in repos {
            match self.collect_repo(db, distro_id, org, &repo.name).await {
                Ok(id) => snapshot_ids.push(id),
                Err(e) => warn!(repo = repo.name, error = %e, "Failed to collect repo metrics"),
            }
        }

        info!(org = org, count = snapshot_ids.len(), "Collected GitHub snapshots");
        Ok(snapshot_ids)
    }

    /// Collect releases for a GitHub organization's repositories
    pub async fn collect_org_releases(
        &self,
        db: &Database,
        distro_id: i64,
        org: &str,
    ) -> Result<Vec<i64>> {
        info!(org = org, "Collecting GitHub releases");

        let repos = self.get_org_repos(org).await?;
        let mut release_ids = Vec::new();

        for repo in repos {
            match self.collect_repo_releases(db, distro_id, org, &repo.name).await {
                Ok(ids) => release_ids.extend(ids),
                Err(e) => warn!(repo = repo.name, error = %e, "Failed to collect releases"),
            }
        }

        info!(org = org, count = release_ids.len(), "Collected releases");
        Ok(release_ids)
    }

    /// Collect releases for a single repository
    pub async fn collect_repo_releases(
        &self,
        db: &Database,
        distro_id: i64,
        owner: &str,
        repo: &str,
    ) -> Result<Vec<i64>> {
        let releases = self.get_releases(owner, repo).await?;
        let mut ids = Vec::new();

        let repo_name = format!("{}/{}", owner, repo);
        for release in releases {
            let snapshot = NewReleaseSnapshot {
                distro_id,
                repo_name: repo_name.clone(),
                tag_name: release.tag_name,
                release_name: release.name,
                published_at: release.published_at,
                is_prerelease: release.prerelease,
            };

            let id = db.insert_release_snapshot(snapshot).await?;
            ids.push(id);
        }

        debug!(owner = owner, repo = repo, count = ids.len(), "Collected releases");
        Ok(ids)
    }

    async fn get_releases(&self, owner: &str, repo: &str) -> Result<Vec<ReleaseResponse>> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/releases?per_page=30",
            owner, repo
        );

        let response = self.client.get(&url).send().await?;
        self.check_rate_limit(&response)?;

        if !response.status().is_success() {
            return Ok(Vec::new());
        }

        let releases: Vec<ReleaseResponse> = response.json().await.unwrap_or_default();
        Ok(releases)
    }

    /// Collect metrics for a single repository
    pub async fn collect_repo(
        &self,
        db: &Database,
        distro_id: i64,
        owner: &str,
        repo: &str,
    ) -> Result<i64> {
        debug!(owner = owner, repo = repo, "Collecting repo metrics");

        let repo_info = self.get_repo(owner, repo).await?;
        let open_prs = self.count_open_prs(owner, repo).await.unwrap_or(0);
        let (commits_30d, commits_365d, contributors_30d) = self
            .get_recent_activity(owner, repo)
            .await
            .unwrap_or((0, 0, 0));

        let snapshot = NewGithubSnapshot {
            distro_id,
            repo_name: format!("{}/{}", owner, repo),
            stars: repo_info.stargazers_count,
            forks: repo_info.forks_count,
            open_issues: repo_info.open_issues_count,
            open_prs,
            commits_30d,
            commits_365d,
            contributors_30d,
            last_commit_at: repo_info.pushed_at,
        };

        let id = db.insert_github_snapshot(snapshot).await?;
        Ok(id)
    }

    async fn get_org_repos(&self, org: &str) -> Result<Vec<RepoResponse>> {
        let url = format!(
            "https://api.github.com/orgs/{}/repos?type=sources&sort=pushed&per_page=30",
            org
        );

        let response = self.client.get(&url).send().await?;
        self.check_rate_limit(&response)?;

        let repos: Vec<RepoResponse> = response.json().await?;
        Ok(repos)
    }

    async fn get_repo(&self, owner: &str, repo: &str) -> Result<RepoResponse> {
        let url = format!("https://api.github.com/repos/{}/{}", owner, repo);

        let response = self.client.get(&url).send().await?;
        self.check_rate_limit(&response)?;

        if !response.status().is_success() {
            return Err(CollectorError::Api(format!(
                "GitHub API error: {}",
                response.status()
            )));
        }

        let repo: RepoResponse = response.json().await?;
        Ok(repo)
    }

    async fn count_open_prs(&self, owner: &str, repo: &str) -> Result<i64> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/pulls?state=open&per_page=1",
            owner, repo
        );

        let response = self.client.get(&url).send().await?;
        self.check_rate_limit(&response)?;

        // GitHub returns the total count in the Link header for pagination
        // For simplicity, we'll make a search query instead
        let search_url = format!(
            "https://api.github.com/search/issues?q=repo:{}/{}+type:pr+state:open",
            owner, repo
        );

        let search_response = self.client.get(&search_url).send().await?;
        self.check_rate_limit(&search_response)?;

        #[derive(Deserialize)]
        struct SearchResult {
            total_count: i64,
        }

        let result: SearchResult = search_response.json().await?;
        Ok(result.total_count)
    }

    async fn get_recent_activity(&self, owner: &str, repo: &str) -> Result<(i64, i64, i64)> {
        // Try stats API first, fall back to commits API if it's not ready
        let stats_url = format!(
            "https://api.github.com/repos/{}/{}/stats/commit_activity",
            owner, repo
        );

        #[derive(Deserialize)]
        struct WeeklyCommits {
            total: i64,
            #[allow(dead_code)]
            week: i64,
        }

        let mut commits_30d_count: i64 = 0;
        let mut commits_365d_count: i64 = 0;

        // Try stats API (returns 202 if computing - need to use fallback)
        let stats_response = self.client.get(&stats_url).send().await?;
        if stats_response.status() == reqwest::StatusCode::OK {
            let weekly_stats: Vec<WeeklyCommits> = stats_response.json().await.unwrap_or_default();
            if !weekly_stats.is_empty() {
                commits_365d_count = weekly_stats.iter().map(|w| w.total).sum();
                commits_30d_count = weekly_stats.iter().rev().take(4).map(|w| w.total).sum();
            }
        }

        // If stats API didn't return data, fall back to commits API
        if commits_365d_count == 0 {
            // Get 30-day commits
            let since_30d = (Utc::now() - chrono::TimeDelta::days(30))
                .format("%Y-%m-%dT%H:%M:%SZ")
                .to_string();
            let url_30d = format!(
                "https://api.github.com/repos/{}/{}/commits?since={}&per_page=100",
                owner, repo, since_30d
            );
            let response_30d = self.client.get(&url_30d).send().await?;
            if response_30d.status().is_success() {
                let commits: Vec<CommitResponse> = response_30d.json().await.unwrap_or_default();
                commits_30d_count = commits.len() as i64;
            }

            // Get 365-day commits (limited to 100, but better than 0)
            let since_365d = (Utc::now() - chrono::TimeDelta::days(365))
                .format("%Y-%m-%dT%H:%M:%SZ")
                .to_string();
            let url_365d = format!(
                "https://api.github.com/repos/{}/{}/commits?since={}&per_page=100",
                owner, repo, since_365d
            );
            let response_365d = self.client.get(&url_365d).send().await?;
            if response_365d.status().is_success() {
                let commits: Vec<CommitResponse> = response_365d.json().await.unwrap_or_default();
                commits_365d_count = commits.len() as i64;
            }
        }

        // Get unique contributors
        let contributors_url = format!(
            "https://api.github.com/repos/{}/{}/stats/contributors",
            owner, repo
        );
        let contrib_response = self.client.get(&contributors_url).send().await?;
        let contributors: Vec<serde_json::Value> = contrib_response.json().await.unwrap_or_default();
        let contributors_count = contributors.len() as i64;

        Ok((commits_30d_count, commits_365d_count, contributors_count))
    }

    fn check_rate_limit(&self, response: &reqwest::Response) -> Result<()> {
        if response.status() == reqwest::StatusCode::FORBIDDEN {
            if let Some(remaining) = response.headers().get("x-ratelimit-remaining") {
                if remaining == "0" {
                    let reset = response
                        .headers()
                        .get("x-ratelimit-reset")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|v| v.parse::<u64>().ok())
                        .unwrap_or(60);

                    let now = Utc::now().timestamp() as u64;
                    let wait = reset.saturating_sub(now);

                    return Err(CollectorError::RateLimited(wait));
                }
            }
        }
        Ok(())
    }
}
