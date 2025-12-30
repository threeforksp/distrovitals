//! Database schema and connection management

use crate::{DatabaseError, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::Path;
use std::str::FromStr;
use tracing::info;

/// Database connection wrapper
#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    /// Connect to an existing database or create a new one
    pub async fn connect(path: &Path) -> Result<Self> {
        let url = format!("sqlite:{}?mode=rwc", path.display());

        let options = SqliteConnectOptions::from_str(&url)?
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .synchronous(sqlx::sqlite::SqliteSynchronous::Normal);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;

        let db = Self { pool };
        db.run_migrations().await?;

        info!("Database connected: {}", path.display());
        Ok(db)
    }

    /// Connect to an in-memory database (for testing)
    pub async fn in_memory() -> Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;

        let db = Self { pool };
        db.run_migrations().await?;

        info!("In-memory database initialized");
        Ok(db)
    }

    /// Get a reference to the connection pool
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Run database migrations
    async fn run_migrations(&self) -> Result<()> {
        sqlx::query(SCHEMA)
            .execute(&self.pool)
            .await
            .map_err(|e| DatabaseError::Migration(e.to_string()))?;

        Ok(())
    }
}

const SCHEMA: &str = r#"
-- Distributions table
CREATE TABLE IF NOT EXISTS distributions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    homepage TEXT,
    github_org TEXT,
    gitlab_group TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- GitHub snapshots
CREATE TABLE IF NOT EXISTS github_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    distro_id INTEGER NOT NULL REFERENCES distributions(id),
    repo_name TEXT NOT NULL,
    stars INTEGER NOT NULL DEFAULT 0,
    forks INTEGER NOT NULL DEFAULT 0,
    open_issues INTEGER NOT NULL DEFAULT 0,
    open_prs INTEGER NOT NULL DEFAULT 0,
    commits_30d INTEGER NOT NULL DEFAULT 0,
    contributors_30d INTEGER NOT NULL DEFAULT 0,
    last_commit_at TEXT,
    collected_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_github_snapshots_distro
    ON github_snapshots(distro_id, collected_at DESC);

-- Package repository snapshots
CREATE TABLE IF NOT EXISTS package_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    distro_id INTEGER NOT NULL REFERENCES distributions(id),
    total_packages INTEGER NOT NULL DEFAULT 0,
    outdated_packages INTEGER NOT NULL DEFAULT 0,
    security_updates INTEGER NOT NULL DEFAULT 0,
    collected_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_package_snapshots_distro
    ON package_snapshots(distro_id, collected_at DESC);

-- Community metrics snapshots
CREATE TABLE IF NOT EXISTS community_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    distro_id INTEGER NOT NULL REFERENCES distributions(id),
    source TEXT NOT NULL,
    active_users_30d INTEGER,
    posts_30d INTEGER,
    response_time_avg_hours REAL,
    collected_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_community_snapshots_distro
    ON community_snapshots(distro_id, collected_at DESC);

-- Health scores
CREATE TABLE IF NOT EXISTS health_scores (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    distro_id INTEGER NOT NULL REFERENCES distributions(id),
    overall_score REAL NOT NULL,
    development_score REAL NOT NULL,
    community_score REAL NOT NULL,
    maintenance_score REAL NOT NULL,
    trend TEXT NOT NULL DEFAULT 'stable',
    calculated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_health_scores_distro
    ON health_scores(distro_id, calculated_at DESC);

-- Seed some initial distributions
INSERT OR IGNORE INTO distributions (name, slug, homepage, github_org) VALUES
    ('Arch Linux', 'arch', 'https://archlinux.org', 'archlinux'),
    ('Debian', 'debian', 'https://debian.org', NULL),
    ('Fedora', 'fedora', 'https://fedoraproject.org', 'fedora-infra'),
    ('Ubuntu', 'ubuntu', 'https://ubuntu.com', 'ubuntu'),
    ('openSUSE', 'opensuse', 'https://opensuse.org', 'openSUSE'),
    ('Linux Mint', 'mint', 'https://linuxmint.com', 'linuxmint'),
    ('NixOS', 'nixos', 'https://nixos.org', 'NixOS'),
    ('Void Linux', 'void', 'https://voidlinux.org', 'void-linux'),
    ('Gentoo', 'gentoo', 'https://gentoo.org', 'gentoo'),
    ('Alpine Linux', 'alpine', 'https://alpinelinux.org', 'alpinelinux');
"#;
