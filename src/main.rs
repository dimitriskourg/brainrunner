mod agent;
mod config;
mod github;
mod prompt;
mod runner;
mod startup;
mod worktree;

use clap::Parser;
use std::path::PathBuf;
use std::time::Duration;
use tracing::info;

#[derive(Parser)]
#[command(name = "brainrunner")]
struct Cli {
    #[arg(long, help = "Path to config.toml")]
    config: Option<PathBuf>,
}

fn default_config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    PathBuf::from(home).join(".config/brainrunner/config.toml")
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    let config_path = cli.config.unwrap_or_else(default_config_path);
    let cfg = match config::load_config(&config_path) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    };

    let worktrees = worktree::WorktreeManager::new(&cfg.repo_path, &cfg.worktree_base);
    let github = github::GithubClient::new(&cfg.repo_path);
    let runner = runner::RalphRunner::new();

    if let Err(e) = startup::startup_sweep(&worktrees, &github).await {
        eprintln!("startup sweep failed: {e}");
        std::process::exit(1);
    }

    info!("startup sweep complete, entering poll loop");

    loop {
        tokio::time::sleep(Duration::from_secs(cfg.poll_interval_secs)).await;

        let issues = match github.list_ready_issues().await {
            Ok(issues) => issues,
            Err(e) => {
                tracing::warn!("failed to list ready issues: {e}");
                continue;
            }
        };

        let Some(issue) = issues.into_iter().next() else {
            info!("no ready issues, sleeping");
            continue;
        };

        info!(issue = issue.number, title = %issue.title, "picked issue for agent run");

        if let Err(e) = agent::run_one(
            &issue,
            &worktrees,
            &github,
            &runner,
            cfg.max_iterations,
            cfg.max_iteration_secs,
        )
        .await
        {
            tracing::warn!(issue = issue.number, "agent run error: {e}");
        }
    }
}
