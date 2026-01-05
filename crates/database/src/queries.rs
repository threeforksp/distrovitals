//! Database query functions

use crate::models::*;
use crate::schema::Database;
use crate::{DatabaseError, Result};

impl Database {
    // ==================== Distributions ====================

    /// Get all distributions
    pub async fn get_distributions(&self) -> Result<Vec<Distribution>> {
        let rows = sqlx::query_as::<_, Distribution>(
            "SELECT id, name, slug, homepage, github_org, gitlab_group, subreddit, description,
                    datetime(created_at) as created_at, datetime(updated_at) as updated_at
             FROM distributions ORDER BY name",
        )
        .fetch_all(self.pool())
        .await?;

        Ok(rows)
    }

    /// Get a distribution by slug
    pub async fn get_distribution_by_slug(&self, slug: &str) -> Result<Distribution> {
        sqlx::query_as::<_, Distribution>(
            "SELECT id, name, slug, homepage, github_org, gitlab_group, subreddit, description,
                    datetime(created_at) as created_at, datetime(updated_at) as updated_at
             FROM distributions WHERE slug = ?",
        )
        .bind(slug)
        .fetch_optional(self.pool())
        .await?
        .ok_or_else(|| DatabaseError::NotFound(format!("Distribution: {}", slug)))
    }

    /// Create a new distribution
    pub async fn create_distribution(&self, distro: NewDistribution) -> Result<Distribution> {
        let id = sqlx::query(
            "INSERT INTO distributions (name, slug, homepage, github_org, gitlab_group, subreddit)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&distro.name)
        .bind(&distro.slug)
        .bind(&distro.homepage)
        .bind(&distro.github_org)
        .bind(&distro.gitlab_group)
        .bind(&distro.subreddit)
        .execute(self.pool())
        .await?
        .last_insert_rowid();

        self.get_distribution_by_id(id).await
    }

    /// Get a distribution by ID
    pub async fn get_distribution_by_id(&self, id: i64) -> Result<Distribution> {
        sqlx::query_as::<_, Distribution>(
            "SELECT id, name, slug, homepage, github_org, gitlab_group, subreddit, description,
                    datetime(created_at) as created_at, datetime(updated_at) as updated_at
             FROM distributions WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(self.pool())
        .await?
        .ok_or_else(|| DatabaseError::NotFound(format!("Distribution ID: {}", id)))
    }

    /// Update a distribution's subreddit
    pub async fn update_distribution_subreddit(&self, id: i64, subreddit: &str) -> Result<()> {
        sqlx::query("UPDATE distributions SET subreddit = ?, updated_at = datetime('now') WHERE id = ?")
            .bind(subreddit)
            .bind(id)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    // ==================== GitHub Snapshots ====================

    /// Insert a new GitHub snapshot
    pub async fn insert_github_snapshot(&self, snapshot: NewGithubSnapshot) -> Result<i64> {
        let id = sqlx::query(
            "INSERT INTO github_snapshots
             (distro_id, repo_name, stars, forks, open_issues, open_prs,
              commits_30d, commits_365d, contributors_30d, last_commit_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(snapshot.distro_id)
        .bind(&snapshot.repo_name)
        .bind(snapshot.stars)
        .bind(snapshot.forks)
        .bind(snapshot.open_issues)
        .bind(snapshot.open_prs)
        .bind(snapshot.commits_30d)
        .bind(snapshot.commits_365d)
        .bind(snapshot.contributors_30d)
        .bind(snapshot.last_commit_at)
        .execute(self.pool())
        .await?
        .last_insert_rowid();

        Ok(id)
    }

    /// Get latest GitHub snapshots for a distribution (most recent per repo)
    pub async fn get_latest_github_snapshots(&self, distro_id: i64) -> Result<Vec<GithubSnapshot>> {
        let rows = sqlx::query_as::<_, GithubSnapshot>(
            "SELECT g.id, g.distro_id, g.repo_name, g.stars, g.forks, g.open_issues, g.open_prs,
                    g.commits_30d, g.commits_365d, g.contributors_30d,
                    datetime(g.last_commit_at) as last_commit_at,
                    datetime(g.collected_at) as collected_at
             FROM github_snapshots g
             INNER JOIN (
                 SELECT repo_name, MAX(collected_at) as max_collected
                 FROM github_snapshots
                 WHERE distro_id = ?
                 GROUP BY repo_name
             ) latest ON g.repo_name = latest.repo_name AND g.collected_at = latest.max_collected
             WHERE g.distro_id = ?
             ORDER BY g.repo_name",
        )
        .bind(distro_id)
        .bind(distro_id)
        .fetch_all(self.pool())
        .await?;

        Ok(rows)
    }

    // ==================== Health Scores ====================

    /// Insert a new health score
    pub async fn insert_health_score(&self, score: NewHealthScore) -> Result<i64> {
        let id = sqlx::query(
            "INSERT INTO health_scores
             (distro_id, overall_score, development_score, community_score, maintenance_score, trend)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(score.distro_id)
        .bind(score.overall_score)
        .bind(score.development_score)
        .bind(score.community_score)
        .bind(score.maintenance_score)
        .bind(&score.trend)
        .execute(self.pool())
        .await?
        .last_insert_rowid();

        Ok(id)
    }

    /// Get latest health score for a distribution
    pub async fn get_latest_health_score(&self, distro_id: i64) -> Result<Option<HealthScore>> {
        let row = sqlx::query_as::<_, HealthScore>(
            "SELECT id, distro_id, overall_score, development_score, community_score,
                    maintenance_score, trend, datetime(calculated_at) as calculated_at
             FROM health_scores
             WHERE distro_id = ?
             ORDER BY calculated_at DESC
             LIMIT 1",
        )
        .bind(distro_id)
        .fetch_optional(self.pool())
        .await?;

        Ok(row)
    }

    /// Get all latest health scores
    pub async fn get_all_latest_health_scores(&self) -> Result<Vec<HealthScore>> {
        let rows = sqlx::query_as::<_, HealthScore>(
            "SELECT h.id, h.distro_id, h.overall_score, h.development_score, h.community_score,
                    h.maintenance_score, h.trend, datetime(h.calculated_at) as calculated_at
             FROM health_scores h
             INNER JOIN (
                 SELECT distro_id, MAX(calculated_at) as max_calc
                 FROM health_scores
                 GROUP BY distro_id
             ) latest ON h.distro_id = latest.distro_id AND h.calculated_at = latest.max_calc
             ORDER BY h.overall_score DESC",
        )
        .fetch_all(self.pool())
        .await?;

        Ok(rows)
    }

    /// Get health score history for a distribution
    pub async fn get_health_score_history(
        &self,
        distro_id: i64,
        days: i32,
    ) -> Result<Vec<HealthScore>> {
        let rows = sqlx::query_as::<_, HealthScore>(
            "SELECT id, distro_id, overall_score, development_score, community_score,
                    maintenance_score, trend, datetime(calculated_at) as calculated_at
             FROM health_scores
             WHERE distro_id = ?
             AND calculated_at >= datetime('now', ?)
             ORDER BY calculated_at ASC",
        )
        .bind(distro_id)
        .bind(format!("-{} days", days))
        .fetch_all(self.pool())
        .await?;

        Ok(rows)
    }

    // ==================== Release Snapshots ====================

    /// Insert a new release snapshot
    pub async fn insert_release_snapshot(&self, snapshot: NewReleaseSnapshot) -> Result<i64> {
        let id = sqlx::query(
            "INSERT INTO release_snapshots
             (distro_id, repo_name, tag_name, release_name, published_at, is_prerelease)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(snapshot.distro_id)
        .bind(&snapshot.repo_name)
        .bind(&snapshot.tag_name)
        .bind(&snapshot.release_name)
        .bind(snapshot.published_at)
        .bind(snapshot.is_prerelease)
        .execute(self.pool())
        .await?
        .last_insert_rowid();

        Ok(id)
    }

    /// Get latest release snapshots for a distribution (most recent per tag)
    pub async fn get_latest_release_snapshots(&self, distro_id: i64) -> Result<Vec<ReleaseSnapshot>> {
        let rows = sqlx::query_as::<_, ReleaseSnapshot>(
            "SELECT r.id, r.distro_id, r.repo_name, r.tag_name, r.release_name,
                    datetime(r.published_at) as published_at, r.is_prerelease,
                    datetime(r.collected_at) as collected_at
             FROM release_snapshots r
             INNER JOIN (
                 SELECT repo_name, tag_name, MAX(collected_at) as max_collected
                 FROM release_snapshots
                 WHERE distro_id = ?
                 GROUP BY repo_name, tag_name
             ) latest ON r.repo_name = latest.repo_name
                     AND r.tag_name = latest.tag_name
                     AND r.collected_at = latest.max_collected
             WHERE r.distro_id = ?
             ORDER BY r.published_at DESC",
        )
        .bind(distro_id)
        .bind(distro_id)
        .fetch_all(self.pool())
        .await?;

        Ok(rows)
    }

    /// Get releases from the last N days for a distribution
    pub async fn get_recent_releases(&self, distro_id: i64, days: i32) -> Result<Vec<ReleaseSnapshot>> {
        let rows = sqlx::query_as::<_, ReleaseSnapshot>(
            "SELECT r.id, r.distro_id, r.repo_name, r.tag_name, r.release_name,
                    datetime(r.published_at) as published_at, r.is_prerelease,
                    datetime(r.collected_at) as collected_at
             FROM release_snapshots r
             INNER JOIN (
                 SELECT repo_name, tag_name, MAX(collected_at) as max_collected
                 FROM release_snapshots
                 WHERE distro_id = ?
                 GROUP BY repo_name, tag_name
             ) latest ON r.repo_name = latest.repo_name
                     AND r.tag_name = latest.tag_name
                     AND r.collected_at = latest.max_collected
             WHERE r.distro_id = ?
             AND r.published_at >= datetime('now', ?)
             ORDER BY r.published_at DESC",
        )
        .bind(distro_id)
        .bind(distro_id)
        .bind(format!("-{} days", days))
        .fetch_all(self.pool())
        .await?;

        Ok(rows)
    }

    // ==================== Community Snapshots ====================

    /// Insert a new community snapshot
    pub async fn insert_community_snapshot(&self, snapshot: NewCommunitySnapshot) -> Result<i64> {
        let id = sqlx::query(
            "INSERT INTO community_snapshots
             (distro_id, source, active_users_30d, posts_30d, response_time_avg_hours)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(snapshot.distro_id)
        .bind(&snapshot.source)
        .bind(snapshot.active_users_30d)
        .bind(snapshot.posts_30d)
        .bind(snapshot.response_time_avg_hours)
        .execute(self.pool())
        .await?
        .last_insert_rowid();

        Ok(id)
    }

    /// Get latest community snapshots for a distribution (most recent per source)
    pub async fn get_latest_community_snapshots(&self, distro_id: i64) -> Result<Vec<CommunitySnapshot>> {
        let rows = sqlx::query_as::<_, CommunitySnapshot>(
            "SELECT c.id, c.distro_id, c.source, c.active_users_30d, c.posts_30d,
                    c.response_time_avg_hours, datetime(c.collected_at) as collected_at
             FROM community_snapshots c
             INNER JOIN (
                 SELECT source, MAX(collected_at) as max_collected
                 FROM community_snapshots
                 WHERE distro_id = ?
                 GROUP BY source
             ) latest ON c.source = latest.source AND c.collected_at = latest.max_collected
             WHERE c.distro_id = ?
             ORDER BY c.source",
        )
        .bind(distro_id)
        .bind(distro_id)
        .fetch_all(self.pool())
        .await?;

        Ok(rows)
    }
}
