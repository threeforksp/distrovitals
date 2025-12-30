//! GitHub API collector

use crate::{CollectorConfig, CollectorError, Result};
use chrono::{DateTime, Utc};
use distrovitals_database::{Database, NewGithubSnapshot};
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
        let (commits_30d, contributors_30d) = self
            .get_recent_activity(owner, repo)
            .await
            .unwrap_or((0, 0));

        let snapshot = NewGithubSnapshot {
            distro_id,
            repo_name: format!("{}/{}", owner, repo),
            stars: repo_info.stargazers_count,
            forks: repo_info.forks_count,
            open_issues: repo_info.open_issues_count,
            open_prs,
            commits_30d,
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

    async fn get_recent_activity(&self, owner: &str, repo: &str) -> Result<(i64, i64)> {
        let since = (Utc::now() - chrono::TimeDelta::days(30))
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string();

        let url = format!(
            "https://api.github.com/repos/{}/{}/commits?since={}&per_page=100",
            owner, repo, since
        );

        let response = self.client.get(&url).send().await?;
        self.check_rate_limit(&response)?;

        let commits: Vec<CommitResponse> = response.json().await?;
        let commits_count = commits.len() as i64;

        // Get unique contributors (simplified - would need pagination for accuracy)
        let contributors_url = format!(
            "https://api.github.com/repos/{}/{}/stats/contributors",
            owner, repo
        );

        let contrib_response = self.client.get(&contributors_url).send().await?;

        #[derive(Deserialize)]
        struct ContributorStats {
            #[allow(dead_code)]
            total: i64,
        }

        let contributors: Vec<ContributorStats> =
            contrib_response.json().await.unwrap_or_default();
        let contributors_count = contributors.len() as i64;

        Ok((commits_count, contributors_count))
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
