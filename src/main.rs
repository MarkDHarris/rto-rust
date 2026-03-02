mod calc;
mod cmd;
mod data;
mod ui;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "rto", about = "Return-to-Office tracker")]
struct Cli {
    /// Path to the data directory (default: ./config)
    #[arg(short = 'd', long, default_value = "./config")]
    data_dir: PathBuf,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize data files with defaults
    Init,
    /// Print statistics for a time period
    Stats {
        /// Period key (e.g. Q1_2025). Uses the current period if not specified.
        period_key: Option<String>,
    },
    /// Backup data directory to git
    Backup {
        /// Remote Git URL to push to
        #[arg(short, long)]
        remote: Option<String>,
        /// The directory to backup (default: data-dir)
        #[arg(long)]
        dir: Option<String>,
    },
    /// List all vacations
    Vacations,
    /// List all holidays
    Holidays,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let data_dir = if cli.data_dir.is_absolute() {
        cli.data_dir.clone()
    } else {
        std::env::current_dir()?.join(&cli.data_dir)
    };
    data::persistence::set_data_dir(data_dir.clone());

    let is_init_command = matches!(cli.command, Some(Commands::Init));
    if !is_init_command && dir_needs_init(&data_dir) {
        eprintln!("Data directory not initialized. Running 'rto init'...",);
        cmd::init::run()?;
    }

    match cli.command {
        None => cmd::root::run(),
        Some(Commands::Init) => cmd::init::run(),
        Some(Commands::Stats { period_key }) => cmd::stats::run(period_key.as_deref()),
        Some(Commands::Backup { remote, dir }) => {
            let target = dir.unwrap_or_else(|| data_dir.to_string_lossy().to_string());
            cmd::backup::run(remote.as_deref(), &target)
        }
        Some(Commands::Vacations) => cmd::vacations::run(),
        Some(Commands::Holidays) => cmd::holidays::run(),
    }
}

/// Returns true when the data directory has never been initialized.
/// Checks for settings.yaml as the canonical marker of initialization.
fn dir_needs_init(dir: &std::path::Path) -> bool {
    if !dir.exists() {
        return true;
    }
    if !dir.is_dir() {
        return true;
    }
    let settings_path = dir.join("settings.yaml");
    !settings_path.exists()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_dir_needs_init_nonexistent() {
        let tmp = TempDir::new().unwrap();
        let missing = tmp.path().join("does_not_exist");
        assert!(dir_needs_init(&missing));
    }

    #[test]
    fn test_dir_needs_init_empty_dir() {
        let tmp = TempDir::new().unwrap();
        assert!(dir_needs_init(tmp.path()));
    }

    #[test]
    fn test_dir_needs_init_nonempty_dir_without_settings() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("file.txt"), "data").unwrap();
        assert!(dir_needs_init(tmp.path()));
    }

    #[test]
    fn test_dir_needs_init_with_settings() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("settings.yaml"), "goal: 50").unwrap();
        assert!(!dir_needs_init(tmp.path()));
    }
}
