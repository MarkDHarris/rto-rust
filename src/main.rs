mod calc;
mod cmd;
mod data;
mod ui;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "rustrto", about = "return to office")]
struct Cli {
    /// Path to the data directory containing config and data files (default: ./config)
    #[arg(long, default_value = "./config")]
    data_dir: PathBuf,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize data files from config
    Init,
    /// Calculate and display RTO statistics for a specific quarter
    Stats {
        /// Quarter key (e.g. Q1_2025)
        quarter_key: String,
    },
    /// Backup data files to a git repository
    Backup {
        /// Remote Git URL to push to
        #[arg(short, long)]
        remote: Option<String>,
        /// The directory containing the data to backup
        #[arg(short = 'd', long, default_value = ".")]
        target_dir: String,
    },
    /// List all vacations
    Vacations,
    /// List all holidays
    Holidays,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Resolve data_dir to an absolute path so git -C <dir> and file I/O work
    // regardless of future directory changes within the process.
    let data_dir = if cli.data_dir.is_absolute() {
        cli.data_dir.clone()
    } else {
        std::env::current_dir()?.join(&cli.data_dir)
    };
    data::persistence::set_data_dir(data_dir.clone());

    // Auto-init when the data directory is missing or empty and the user did not
    // explicitly invoke the `init` subcommand.
    let is_init_command = matches!(cli.command, Some(Commands::Init));
    if !is_init_command && dir_needs_init(&data_dir) {
        eprintln!(
            "Data directory '{}' is missing or empty — running init...",
            data_dir.display()
        );
        cmd::init::run()?;
    }

    match cli.command {
        None => cmd::root::run(),
        Some(Commands::Init) => cmd::init::run(),
        Some(Commands::Stats { quarter_key }) => cmd::stats::run(&quarter_key),
        Some(Commands::Backup { remote, target_dir }) => {
            cmd::backup::run(remote.as_deref(), &target_dir)
        }
        Some(Commands::Vacations) => cmd::vacations::run(),
        Some(Commands::Holidays) => cmd::holidays::run(),
    }
}

/// Returns true when `dir` does not exist or exists but contains no files.
fn dir_needs_init(dir: &std::path::Path) -> bool {
    if !dir.exists() {
        return true;
    }
    dir.read_dir()
        .map(|mut entries| entries.next().is_none())
        .unwrap_or(false)
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
    fn test_dir_needs_init_nonempty_dir() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("file.txt"), "data").unwrap();
        assert!(!dir_needs_init(tmp.path()));
    }
}
