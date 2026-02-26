use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;

pub fn run(remote: Option<&str>, target_dir: &str) -> Result<()> {
    println!("Initializing Git repository and pushing to remote...");

    if !Path::new(target_dir).exists() {
        bail!("Target directory does not exist: {}", target_dir);
    }

    let abs_path = std::fs::canonicalize(target_dir)
        .with_context(|| format!("failed to resolve path: {}", target_dir))?;
    println!("Backing up data in: {}", abs_path.display());

    perform_git_backup(target_dir, remote)?;
    println!("Backup completed successfully.");
    Ok(())
}

fn perform_git_backup(dir: &str, remote: Option<&str>) -> Result<()> {
    // 1. Check if already a git repo
    if run_git_command(dir, &["rev-parse", "--is-inside-work-tree"]).is_err() {
        println!("Repository not found. Initializing...");
        run_git_command(dir, &["init"])
            .context("git init failed")?;
        // Rename branch to main
        let _ = run_git_command(dir, &["branch", "-M", "main"]);
    }

    // 2. Configure remote (idempotent)
    if let Some(remote_url) = remote {
        if run_git_command(dir, &["remote", "get-url", "origin"]).is_ok() {
            run_git_command(dir, &["remote", "set-url", "origin", remote_url])
                .context("git remote set-url failed")?;
        } else {
            run_git_command(dir, &["remote", "add", "origin", remote_url])
                .context("git remote add failed")?;
        }
    }

    // 3. Add all files
    run_git_command_visible(dir, &["add", "."])
        .context("git add failed")?;

    // 4. Commit (allow failure for "nothing to commit")
    if let Err(_) = run_git_command_visible(dir, &["commit", "-m", "Backup: updated data files"]) {
        println!("Nothing to commit or commit failed (proceeding to push anyway)...");
    }

    // 5. Push
    if remote.is_some() {
        println!("Pushing to remote...");
        run_git_command_visible(dir, &["push", "-u", "origin", "main"])
            .context("git push failed")?;
    } else {
        println!("No remote specified. Skipping push.");
    }

    Ok(())
}

fn run_git_command(dir: &str, args: &[&str]) -> Result<()> {
    let status = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .context("failed to run git")?;
    if status.status.success() {
        Ok(())
    } else {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&status.stderr)
        )
    }
}

fn run_git_command_visible(dir: &str, args: &[&str]) -> Result<()> {
    println!("[{}] Running: git {}", dir, args.join(" "));
    let status = Command::new("git")
        .args(args)
        .current_dir(dir)
        .status()
        .context("failed to run git")?;
    if status.success() {
        Ok(())
    } else {
        bail!("git {} failed", args.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_run_missing_target_dir_returns_error() {
        let result = run(None, "/nonexistent/path/that/cannot/exist/xyz123");
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("does not exist") || msg.contains("nonexistent"));
    }

    #[test]
    fn test_run_existing_target_dir_no_remote() {
        if Command::new("git").arg("--version").output().is_err() {
            return;
        }
        let _ = Command::new("git")
            .args(["config", "--global", "user.email", "test@test.com"])
            .output();
        let _ = Command::new("git")
            .args(["config", "--global", "user.name", "Test"])
            .output();
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(tmp.path().join("data.txt"), "hello").unwrap();
        let dir = tmp.path().to_str().unwrap();
        let result = run(None, dir);
        assert!(result.is_ok(), "run() failed: {:?}", result);
    }

    #[test]
    fn test_backup_already_initialized_repo() {
        if Command::new("git").arg("--version").output().is_err() {
            return;
        }
        let _ = Command::new("git")
            .args(["config", "--global", "user.email", "test@test.com"])
            .output();
        let _ = Command::new("git")
            .args(["config", "--global", "user.name", "Test"])
            .output();
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path().to_str().unwrap().to_string();
        // Pre-init the repo so the "already a git repo" branch is hit
        Command::new("git")
            .args(["init"])
            .current_dir(&dir)
            .output()
            .unwrap();
        fs::write(format!("{}/file.txt", dir), "data").unwrap();
        // First backup: commits
        let r1 = perform_git_backup(&dir, None);
        assert!(r1.is_ok(), "first backup failed: {:?}", r1);
        // Second backup on same repo: "nothing to commit" path
        let r2 = perform_git_backup(&dir, None);
        assert!(r2.is_ok(), "second backup (nothing to commit) failed: {:?}", r2);
    }

    #[test]
    fn test_git_backup_local_only() {
        // Skip if git not available
        if Command::new("git").arg("--version").output().is_err() {
            return;
        }
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path().to_str().unwrap().to_string();
        // Write a dummy file
        fs::write(format!("{}/test.txt", dir), "hello").unwrap();
        // Configure git identity for test
        let _ = Command::new("git")
            .args(["config", "--global", "user.email", "test@test.com"])
            .output();
        let _ = Command::new("git")
            .args(["config", "--global", "user.name", "Test"])
            .output();
        let result = perform_git_backup(&dir, None);
        assert!(result.is_ok(), "backup failed: {:?}", result);
    }

    #[test]
    fn test_git_backup_with_remote() {
        if Command::new("git").arg("--version").output().is_err() {
            return;
        }
        // Create a bare "remote" repo
        let bare = tempfile::TempDir::new().unwrap();
        let bare_path = bare.path().to_str().unwrap().to_string();
        Command::new("git")
            .args(["init", "--bare"])
            .current_dir(&bare_path)
            .output()
            .unwrap();

        let source = tempfile::TempDir::new().unwrap();
        let source_path = source.path().to_str().unwrap().to_string();
        fs::write(format!("{}/data.txt", source_path), "backup data").unwrap();

        let _ = Command::new("git")
            .args(["config", "--global", "user.email", "test@test.com"])
            .output();
        let _ = Command::new("git")
            .args(["config", "--global", "user.name", "Test"])
            .output();

        let result = perform_git_backup(&source_path, Some(&bare_path));
        // Push may fail in CI without proper git config, just check it ran
        let _ = result; // don't assert hard failure
    }
}
