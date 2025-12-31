//! DistroVitals CLI
//!
//! Admin tool and web server runner.

use anyhow::Result;
use clap::{Parser, Subcommand};
use distrovitals_analyzer::Analyzer;
use distrovitals_api::{create_router, AppState};
use distrovitals_collector::{github::GithubCollector, reddit::RedditCollector, CollectorConfig};
use distrovitals_database::Database;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser)]
#[command(name = "dv")]
#[command(about = "DistroVitals - Linux Distribution Health Tracker")]
#[command(version)]
struct Cli {
    /// Database file path
    #[arg(short, long, default_value = "distrovitals.db")]
    database: PathBuf,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the web server
    Serve {
        /// Address to bind to
        #[arg(short, long, default_value = "127.0.0.1:3000")]
        bind: SocketAddr,

        /// Static files directory
        #[arg(short, long)]
        static_dir: Option<PathBuf>,
    },

    /// Collect GitHub data for distributions
    Collect {
        /// Distribution slug (or "all" for all distributions)
        #[arg(default_value = "all")]
        distro: String,
    },

    /// Collect Reddit community data for distributions
    CollectReddit {
        /// Distribution slug (or "all" for all distributions)
        #[arg(default_value = "all")]
        distro: String,
    },

    /// Calculate health scores
    Analyze {
        /// Distribution slug (or "all" for all distributions)
        #[arg(default_value = "all")]
        distro: String,
    },

    /// List tracked distributions
    List,

    /// Show health rankings
    Rankings,

    /// Show status of a distribution
    Status {
        /// Distribution slug
        distro: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let level = if cli.verbose { Level::DEBUG } else { Level::INFO };
    FmtSubscriber::builder()
        .with_max_level(level)
        .with_target(false)
        .compact()
        .init();

    // Connect to database
    let db = Database::connect(&cli.database).await?;

    match cli.command {
        Commands::Serve { bind, static_dir } => {
            serve(db, bind, static_dir).await?;
        }
        Commands::Collect { distro } => {
            collect(&db, &distro).await?;
        }
        Commands::CollectReddit { distro } => {
            collect_reddit(&db, &distro).await?;
        }
        Commands::Analyze { distro } => {
            analyze(&db, &distro).await?;
        }
        Commands::List => {
            list(&db).await?;
        }
        Commands::Rankings => {
            rankings(&db).await?;
        }
        Commands::Status { distro } => {
            status(&db, &distro).await?;
        }
    }

    Ok(())
}

async fn serve(db: Database, bind: SocketAddr, static_dir: Option<PathBuf>) -> Result<()> {
    let state = Arc::new(AppState::new(db));
    let router = create_router(state, static_dir.clone());

    info!("Starting DistroVitals server on {}", bind);
    if let Some(ref dir) = static_dir {
        info!("Serving static files from {}", dir.display());
    }
    info!("API available at http://{}/api/v1", bind);

    let listener = tokio::net::TcpListener::bind(bind).await?;
    axum::serve(listener, router).await?;

    Ok(())
}

async fn collect_reddit(db: &Database, distro_slug: &str) -> Result<()> {
    let config = CollectorConfig::default();
    let collector = RedditCollector::new(config)?;

    if distro_slug == "all" {
        println!("Collecting Reddit data for all distributions...");
        match collector.collect_all(db).await {
            Ok(ids) => println!("Reddit: {} snapshots collected", ids.len()),
            Err(e) => eprintln!("Reddit: Error - {}", e),
        }
    } else {
        let distro = db.get_distribution_by_slug(distro_slug).await?;
        println!("Collecting Reddit data for {}...", distro.name);

        if let Some(ref subreddit) = distro.subreddit {
            match collector.collect_subreddit(db, distro.id, subreddit).await {
                Ok(_) => println!("  Reddit: r/{} collected", subreddit),
                Err(e) => eprintln!("  Reddit: Error - {}", e),
            }
        } else {
            println!("  Reddit: No subreddit configured, skipping");
        }
    }

    println!("\nReddit collection complete!");
    Ok(())
}

async fn collect(db: &Database, distro_slug: &str) -> Result<()> {
    let config = CollectorConfig::default();

    if config.github_token.is_none() {
        eprintln!("Warning: GITHUB_TOKEN not set. API rate limits will be restricted.");
    }

    let collector = GithubCollector::new(config)?;

    let distros = if distro_slug == "all" {
        db.get_distributions().await?
    } else {
        vec![db.get_distribution_by_slug(distro_slug).await?]
    };

    for distro in distros {
        println!("Collecting data for {}...", distro.name);

        if let Some(ref org) = distro.github_org {
            match collector.collect_org_repos(db, distro.id, org).await {
                Ok(ids) => println!("  GitHub: {} snapshots collected", ids.len()),
                Err(e) => eprintln!("  GitHub: Error - {}", e),
            }

            match collector.collect_org_releases(db, distro.id, org).await {
                Ok(ids) => println!("  Releases: {} collected", ids.len()),
                Err(e) => eprintln!("  Releases: Error - {}", e),
            }
        } else {
            println!("  GitHub: No org configured, skipping");
        }
    }

    println!("\nCollection complete!");
    Ok(())
}

async fn analyze(db: &Database, distro_slug: &str) -> Result<()> {
    let distros = if distro_slug == "all" {
        db.get_distributions().await?
    } else {
        vec![db.get_distribution_by_slug(distro_slug).await?]
    };

    for distro in distros {
        print!("Analyzing {}... ", distro.name);

        match Analyzer::calculate_health_score(db, distro.id).await {
            Ok(_) => {
                if let Ok(Some(score)) = db.get_latest_health_score(distro.id).await {
                    println!(
                        "Score: {:.1} (Dev: {:.1}, Community: {:.1}, Maint: {:.1}) [{}]",
                        score.overall_score,
                        score.development_score,
                        score.community_score,
                        score.maintenance_score,
                        score.trend
                    );
                }
            }
            Err(e) => eprintln!("Error: {}", e),
        }
    }

    Ok(())
}

async fn list(db: &Database) -> Result<()> {
    let distros = db.get_distributions().await?;

    println!("{:<15} {:<20} {:<15}", "SLUG", "NAME", "GITHUB ORG");
    println!("{}", "-".repeat(50));

    for distro in distros {
        println!(
            "{:<15} {:<20} {:<15}",
            distro.slug,
            distro.name,
            distro.github_org.as_deref().unwrap_or("-")
        );
    }

    Ok(())
}

async fn rankings(db: &Database) -> Result<()> {
    let distros = db.get_distributions().await?;
    let scores = db.get_all_latest_health_scores().await?;

    println!("{:<5} {:<15} {:<10} {:<8}", "RANK", "DISTRO", "SCORE", "TREND");
    println!("{}", "-".repeat(40));

    for (idx, score) in scores.iter().enumerate() {
        if let Some(distro) = distros.iter().find(|d| d.id == score.distro_id) {
            let trend_icon = match score.trend.as_str() {
                "up" => "↑",
                "down" => "↓",
                _ => "→",
            };
            println!(
                "{:<5} {:<15} {:<10.1} {}",
                idx + 1,
                distro.slug,
                score.overall_score,
                trend_icon
            );
        }
    }

    if scores.is_empty() {
        println!("No scores yet. Run 'dv collect' and 'dv analyze' first.");
    }

    Ok(())
}

async fn status(db: &Database, distro_slug: &str) -> Result<()> {
    let distro = db.get_distribution_by_slug(distro_slug).await?;

    println!("Distribution: {} ({})", distro.name, distro.slug);
    println!("Homepage: {}", distro.homepage.as_deref().unwrap_or("-"));
    println!("GitHub Org: {}", distro.github_org.as_deref().unwrap_or("-"));
    println!();

    if let Ok(Some(score)) = db.get_latest_health_score(distro.id).await {
        let trend_icon = match score.trend.as_str() {
            "up" => "↑",
            "down" => "↓",
            _ => "→",
        };

        println!("Health Score: {:.1} {}", score.overall_score, trend_icon);
        println!("  Development:  {:.1}", score.development_score);
        println!("  Community:    {:.1}", score.community_score);
        println!("  Maintenance:  {:.1}", score.maintenance_score);
        println!("  Last Updated: {}", score.calculated_at);
    } else {
        println!("No health score available yet.");
    }

    let github_snapshots = db.get_latest_github_snapshots(distro.id).await?;
    if !github_snapshots.is_empty() {
        println!("\nGitHub Metrics:");
        for snap in github_snapshots.iter().take(5) {
            println!(
                "  {} - ⭐{} 🍴{} 📝{} PRs:{}",
                snap.repo_name, snap.stars, snap.forks, snap.open_issues, snap.open_prs
            );
        }
        if github_snapshots.len() > 5 {
            println!("  ... and {} more repos", github_snapshots.len() - 5);
        }
    }

    Ok(())
}
