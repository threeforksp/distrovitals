//! Database query functions

use crate::models::*;
use crate::schema::Database;
use crate::{DatabaseError, Result};

impl Database {
    // ==================== Distributions ====================

    /// Get all distributions
    pub async fn get_distributions(&self) -> Result<Vec<Distribution>> {
        let rows = sqlx::query_as::<_, Distribution>(
            "SELECT id, name, slug, homepage, github_org, gitlab_group,
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
            "SELECT id, name, slug, homepage, github_org, gitlab_group,
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
            "INSERT INTO distributions (name, slug, homepage, github_org, gitlab_group)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&distro.name)
        .bind(&distro.slug)
        .bind(&distro.homepage)
        .bind(&distro.github_org)
        .bind(&distro.gitlab_group)
        .execute(self.pool())
        .await?
        .last_insert_rowid();

        self.get_distribution_by_id(id).await
    }

    /// Get a distribution by ID
    pub async fn get_distribution_by_id(&self, id: i64) -> Result<Distribution> {
        sqlx::query_as::<_, Distribution>(
            "SELECT id, name, slug, homepage, github_org, gitlab_group,
                    datetime(created_at) as created_at, datetime(updated_at) as updated_at
             FROM distributions WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(self.pool())
        .await?
        .ok_or_else(|| DatabaseError::NotFound(format!("Distribution ID: {}", id)))
    }

    // ==================== GitHub Snapshots ====================

    /// Insert a new GitHub snapshot
    pub async fn insert_github_snapshot(&self, snapshot: NewGithubSnapshot) -> Result<i64> {
        let id = sqlx::query(
            "INSERT INTO github_snapshots
             (distro_id, repo_name, stars, forks, open_issues, open_prs,
              commits_30d, contributors_30d, last_commit_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(snapshot.distro_id)
        .bind(&snapshot.repo_name)
        .bind(snapshot.stars)
        .bind(snapshot.forks)
        .bind(snapshot.open_issues)
        .bind(snapshot.open_prs)
        .bind(snapshot.commits_30d)
        .bind(snapshot.contributors_30d)
        .bind(snapshot.last_commit_at)
        .execute(self.pool())
        .await?
        .last_insert_rowid();

        Ok(id)
    }

    /// Get latest GitHub snapshots for a distribution
    pub async fn get_latest_github_snapshots(&self, distro_id: i64) -> Result<Vec<GithubSnapshot>> {
        let rows = sqlx::query_as::<_, GithubSnapshot>(
            "SELECT id, distro_id, repo_name, stars, forks, open_issues, open_prs,
                    commits_30d, contributors_30d,
                    datetime(last_commit_at) as last_commit_at,
                    datetime(collected_at) as collected_at
             FROM github_snapshots
             WHERE distro_id = ?
             AND collected_at = (SELECT MAX(collected_at) FROM github_snapshots WHERE distro_id = ?)
             ORDER BY repo_name",
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
}
