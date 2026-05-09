mod config;
mod github;
mod prompt;
mod runner;
mod startup;
mod worktree;

use clap::Parser;
use std::path::PathBuf;

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

    if let Err(e) = startup::startup_sweep(&worktrees, &github).await {
        eprintln!("startup sweep failed: {e}");
        std::process::exit(1);
    }
}
