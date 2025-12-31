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
        // Run base schema (tables without subreddit for backwards compat)
        sqlx::query(BASE_SCHEMA)
            .execute(&self.pool)
            .await
            .map_err(|e| DatabaseError::Migration(e.to_string()))?;

        // Run incremental migrations (adds subreddit column if needed)
        self.run_incremental_migrations().await?;

        // Seed distributions (with subreddit now available)
        sqlx::query(SEED_DATA)
            .execute(&self.pool)
            .await
            .map_err(|e| DatabaseError::Migration(e.to_string()))?;

        Ok(())
    }

    /// Run incremental migrations for schema changes
    async fn run_incremental_migrations(&self) -> Result<()> {
        // Add subreddit column if it doesn't exist
        let has_subreddit: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('distributions') WHERE name = 'subreddit'"
        )
        .fetch_one(&self.pool)
        .await
        .unwrap_or(false);

        if !has_subreddit {
            sqlx::query("ALTER TABLE distributions ADD COLUMN subreddit TEXT")
                .execute(&self.pool)
                .await
                .map_err(|e| DatabaseError::Migration(format!("Failed to add subreddit column: {}", e)))?;

            // Populate subreddits for existing distros
            let updates = [
                ("arch", "archlinux"),
                ("debian", "debian"),
                ("fedora", "Fedora"),
                ("nixos", "NixOS"),
                ("ubuntu", "Ubuntu"),
                ("popos", "pop_os"),
                ("manjaro", "ManjaroLinux"),
                ("endeavouros", "EndeavourOS"),
                ("mint", "linuxmint"),
                ("gentoo", "Gentoo"),
                ("void", "voidlinux"),
                ("opensuse", "openSUSE"),
                ("elementary", "elementaryos"),
                ("garuda", "GarudaLinux"),
                ("kali", "Kalilinux"),
                ("alpine", "alpinelinux"),
                ("rocky", "RockyLinux"),
                ("almalinux", "AlmaLinux"),
                ("qubes", "Qubes"),
                ("cachyos", "cachyos"),
                ("bazzite", "bazzite"),
                ("solus", "SolusProject"),
            ];

            for (slug, subreddit) in updates {
                sqlx::query("UPDATE distributions SET subreddit = ? WHERE slug = ?")
                    .bind(subreddit)
                    .bind(slug)
                    .execute(&self.pool)
                    .await
                    .ok(); // Ignore errors for missing slugs
            }

            info!("Added subreddit column and populated data");
        }

        Ok(())
    }
}

const BASE_SCHEMA: &str = r#"
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

-- Release snapshots
CREATE TABLE IF NOT EXISTS release_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    distro_id INTEGER NOT NULL REFERENCES distributions(id),
    repo_name TEXT NOT NULL,
    tag_name TEXT NOT NULL,
    release_name TEXT,
    published_at TEXT,
    is_prerelease INTEGER NOT NULL DEFAULT 0,
    collected_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_release_snapshots_distro
    ON release_snapshots(distro_id, collected_at DESC);

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
"#;

const SEED_DATA: &str = r#"
-- Seed distributions
-- Major independent distributions
INSERT OR IGNORE INTO distributions (name, slug, homepage, github_org, subreddit) VALUES
    ('Arch Linux', 'arch', 'https://archlinux.org', 'archlinux', 'archlinux'),
    ('Debian', 'debian', 'https://debian.org', NULL, 'debian'),
    ('Fedora', 'fedora', 'https://fedoraproject.org', 'fedora-infra', 'Fedora'),
    ('openSUSE', 'opensuse', 'https://opensuse.org', 'openSUSE', 'openSUSE'),
    ('Gentoo', 'gentoo', 'https://gentoo.org', 'gentoo', 'Gentoo'),
    ('Slackware', 'slackware', 'http://www.slackware.com', NULL, 'slackware'),
    ('Void Linux', 'void', 'https://voidlinux.org', 'void-linux', 'voidlinux'),
    ('Alpine Linux', 'alpine', 'https://alpinelinux.org', 'alpinelinux', 'alpinelinux'),
    ('NixOS', 'nixos', 'https://nixos.org', 'NixOS', 'NixOS'),
    ('Clear Linux', 'clearlinux', 'https://clearlinux.org', 'clearlinux', NULL),
    ('Solus', 'solus', 'https://getsol.us', 'getsolus', 'SolusProject'),
    ('Mageia', 'mageia', 'https://www.mageia.org', NULL, NULL);

-- Debian-based
INSERT OR IGNORE INTO distributions (name, slug, homepage, github_org, subreddit) VALUES
    ('Ubuntu', 'ubuntu', 'https://ubuntu.com', 'ubuntu', 'Ubuntu'),
    ('Linux Mint', 'mint', 'https://linuxmint.com', 'linuxmint', 'linuxmint'),
    ('Pop!_OS', 'popos', 'https://pop.system76.com', 'pop-os', 'pop_os'),
    ('elementary OS', 'elementary', 'https://elementary.io', 'elementary', 'elementaryos'),
    ('Zorin OS', 'zorin', 'https://zorin.com/os', NULL, 'zorinos'),
    ('MX Linux', 'mxlinux', 'https://mxlinux.org', 'MX-Linux', 'MXLinux'),
    ('antiX', 'antix', 'https://antixlinux.com', NULL, NULL),
    ('KDE neon', 'kdeneon', 'https://neon.kde.org', NULL, 'kdeneon'),
    ('Kali Linux', 'kali', 'https://www.kali.org', 'kalilinux', 'Kalilinux'),
    ('Parrot OS', 'parrot', 'https://www.parrotsec.org', 'ParrotSec', 'ParrotOS'),
    ('Tails', 'tails', 'https://tails.net', NULL, 'tails'),
    ('Raspberry Pi OS', 'raspios', 'https://www.raspberrypi.com/software', 'RPi-Distro', 'raspberry_pi'),
    ('Deepin', 'deepin', 'https://www.deepin.org', 'linuxdeepin', 'deepin'),
    ('PureOS', 'pureos', 'https://pureos.net', NULL, NULL),
    ('Devuan', 'devuan', 'https://www.devuan.org', NULL, 'Devuan');

-- Arch-based
INSERT OR IGNORE INTO distributions (name, slug, homepage, github_org, subreddit) VALUES
    ('Manjaro', 'manjaro', 'https://manjaro.org', 'manjaro', 'ManjaroLinux'),
    ('EndeavourOS', 'endeavouros', 'https://endeavouros.com', 'endeavouros-team', 'EndeavourOS'),
    ('Garuda Linux', 'garuda', 'https://garudalinux.org', 'garuda-linux', 'GarudaLinux'),
    ('ArcoLinux', 'arcolinux', 'https://arcolinux.com', 'arcolinux', 'arcolinux'),
    ('Artix Linux', 'artix', 'https://artixlinux.org', 'artix-linux', 'artixlinux'),
    ('CachyOS', 'cachyos', 'https://cachyos.org', 'CachyOS', 'cachyos');

-- Fedora-based / RPM
INSERT OR IGNORE INTO distributions (name, slug, homepage, github_org, subreddit) VALUES
    ('Rocky Linux', 'rocky', 'https://rockylinux.org', 'rocky-linux', 'RockyLinux'),
    ('AlmaLinux', 'almalinux', 'https://almalinux.org', 'AlmaLinux', 'AlmaLinux'),
    ('CentOS Stream', 'centosstream', 'https://www.centos.org', NULL, 'CentOS'),
    ('Nobara', 'nobara', 'https://nobaraproject.org', 'Nobara-Project', 'NobaraProject'),
    ('Ultramarine', 'ultramarine', 'https://ultramarine-linux.org', 'Ultramarine-Linux', NULL),
    ('Bazzite', 'bazzite', 'https://bazzite.gg', 'ublue-os', 'bazzite');

-- Immutable / Container-focused
INSERT OR IGNORE INTO distributions (name, slug, homepage, github_org, subreddit) VALUES
    ('Fedora Silverblue', 'silverblue', 'https://fedoraproject.org/silverblue', NULL, 'Fedora'),
    ('Fedora Kinoite', 'kinoite', 'https://fedoraproject.org/kinoite', NULL, 'Fedora'),
    ('openSUSE MicroOS', 'microos', 'https://microos.opensuse.org', NULL, 'openSUSE'),
    ('Vanilla OS', 'vanillaos', 'https://vanillaos.org', 'Vanilla-OS', 'vanillaos'),
    ('blendOS', 'blendos', 'https://blendos.co', 'blend-os', 'blendos');

-- Specialized / Niche
INSERT OR IGNORE INTO distributions (name, slug, homepage, github_org, subreddit) VALUES
    ('Qubes OS', 'qubes', 'https://www.qubes-os.org', 'QubesOS', 'Qubes'),
    ('Whonix', 'whonix', 'https://www.whonix.org', 'Whonix', 'Whonix'),
    ('Bedrock Linux', 'bedrock', 'https://bedrocklinux.org', 'bedrocklinux', 'bedrocklinux'),
    ('GoboLinux', 'gobolinux', 'https://gobolinux.org', 'gobolinux', NULL),
    ('Guix System', 'guix', 'https://guix.gnu.org', NULL, 'GUIX'),
    ('KISS Linux', 'kiss', 'https://kisslinux.org', 'kiss-community', 'kisslinux'),
    ('Chimera Linux', 'chimera', 'https://chimera-linux.org', 'chimera-linux', NULL),
    ('Serpent OS', 'serpent', 'https://serpentos.com', 'serpent-os', NULL);

-- Update existing distributions with subreddits (migration for existing data)
UPDATE distributions SET subreddit = 'archlinux' WHERE slug = 'arch' AND subreddit IS NULL;
UPDATE distributions SET subreddit = 'debian' WHERE slug = 'debian' AND subreddit IS NULL;
UPDATE distributions SET subreddit = 'Fedora' WHERE slug = 'fedora' AND subreddit IS NULL;
UPDATE distributions SET subreddit = 'NixOS' WHERE slug = 'nixos' AND subreddit IS NULL;
UPDATE distributions SET subreddit = 'Ubuntu' WHERE slug = 'ubuntu' AND subreddit IS NULL;
UPDATE distributions SET subreddit = 'pop_os' WHERE slug = 'popos' AND subreddit IS NULL;
UPDATE distributions SET subreddit = 'ManjaroLinux' WHERE slug = 'manjaro' AND subreddit IS NULL;
UPDATE distributions SET subreddit = 'EndeavourOS' WHERE slug = 'endeavouros' AND subreddit IS NULL;
UPDATE distributions SET subreddit = 'linuxmint' WHERE slug = 'mint' AND subreddit IS NULL;
UPDATE distributions SET subreddit = 'Gentoo' WHERE slug = 'gentoo' AND subreddit IS NULL;
UPDATE distributions SET subreddit = 'voidlinux' WHERE slug = 'void' AND subreddit IS NULL;
UPDATE distributions SET subreddit = 'openSUSE' WHERE slug = 'opensuse' AND subreddit IS NULL;
"#;
