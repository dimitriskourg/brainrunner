mod config;
mod prompt;

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

fn main() {
    let cli = Cli::parse();
    let config_path = cli.config.unwrap_or_else(default_config_path);
    match config::load_config(&config_path) {
        Ok(cfg) => {
            println!("repo_path: {}", cfg.repo_path);
            println!("poll_interval_secs: {}", cfg.poll_interval_secs);
        }
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}
