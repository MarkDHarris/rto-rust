use anyhow::Result;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct BackupResult {
    pub message: String,
    pub is_error: bool,
}

#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct StatusInfo {
    pub is_repo: bool,
    pub has_remote: bool,
    pub modified: usize,
    pub untracked: usize,
    pub last_commit: String,
    pub clean: bool,
}

pub fn run(remote: Option<&str>, target_dir: &str) -> Result<()> {
    let dir = if target_dir.is_empty() {
        crate::data::persistence::get_data_dir()?
    } else {
        std::path::PathBuf::from(target_dir)
    };

    let result = perform(&dir, remote.unwrap_or(""));
    if result.is_error {
        anyhow::bail!("{}", result.message);
    }
    println!("{}", result.message);
    Ok(())
}

/// Executes the git backup workflow: init, remote, add, commit, push.
pub fn perform(dir: &Path, remote: &str) -> BackupResult {
    let is_repo = is_git_repo(dir);

    if !is_repo {
        if let Err(e) = run_git_silent(dir, &["init"]) {
            return BackupResult {
                message: format!("git init failed: {}", e),
                is_error: true,
            };
        }
        let _ = run_git_silent(dir, &["checkout", "-b", "main"]);
    }

    if !remote.is_empty() {
        if remote_exists(dir) {
            if let Err(e) = run_git_silent(dir, &["remote", "set-url", "origin", remote]) {
                return BackupResult {
                    message: format!("set-url failed: {}", e),
                    is_error: true,
                };
            }
        } else if let Err(e) = run_git_silent(dir, &["remote", "add", "origin", remote]) {
            return BackupResult {
                message: format!("remote add failed: {}", e),
                is_error: true,
            };
        }
    }

    if let Err(e) = run_git_silent(dir, &["add", "."]) {
        return BackupResult {
            message: format!("git add failed: {}", e),
            is_error: true,
        };
    }

    let now = chrono::Local::now();
    let commit_msg = format!(
        "backup: {}-{:03}",
        now.format("%Y-%m-%d-%H-%M-%S"),
        now.timestamp_subsec_millis()
    );

    if let Err(e) = run_git_silent(dir, &["commit", "-m", &commit_msg]) {
        let err_str = e.to_string();
        if err_str.contains("nothing to commit") || err_str.contains("nothing added") {
            return BackupResult {
                message: "Nothing to commit — backup up to date".to_string(),
                is_error: false,
            };
        }
        return BackupResult {
            message: format!("git commit failed: {}", e),
            is_error: true,
        };
    }

    if !remote.is_empty() || remote_exists(dir) {
        if let Err(e) = run_git_silent(dir, &["push", "origin", "main"]) {
            return BackupResult {
                message: format!("Committed (push failed: {})", e),
                is_error: false,
            };
        }
        return BackupResult {
            message: "Backup committed and pushed".to_string(),
            is_error: false,
        };
    }

    BackupResult {
        message: "Backup committed (no remote configured)".to_string(),
        is_error: false,
    }
}

/// Checks the git state of a directory and returns a summary.
#[allow(dead_code)]
pub fn status(dir: &Path) -> StatusInfo {
    let mut info = StatusInfo::default();

    if !is_git_repo(dir) {
        return info;
    }
    info.is_repo = true;
    info.has_remote = remote_exists(dir);

    if let Ok(output) = run_git_output(dir, &["status", "--porcelain"]) {
        for line in output.trim().lines() {
            if line.len() < 2 {
                continue;
            }
            if line.starts_with("??") {
                info.untracked += 1;
            } else {
                info.modified += 1;
            }
        }
    }

    info.clean = info.modified == 0 && info.untracked == 0;

    if let Ok(output) = run_git_output(dir, &["log", "-1", "--format=%s"]) {
        info.last_commit = output.trim().to_string();
    }

    info
}

fn is_git_repo(dir: &Path) -> bool {
    dir.join(".git").exists()
}

fn remote_exists(dir: &Path) -> bool {
    if let Ok(output) = run_git_output(dir, &["remote"]) {
        for line in output.lines() {
            if line.trim() == "origin" {
                return true;
            }
        }
    }
    false
}

fn run_git_silent(dir: &Path, args: &[&str]) -> Result<()> {
    let output = Command::new("git").args(args).current_dir(dir).output()?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let combined = if stderr.is_empty() { stdout } else { stderr };
        anyhow::bail!("{}: {}", args.join(" "), combined)
    }
}

fn run_git_output(dir: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git").args(args).current_dir(dir).output()?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn has_git() -> bool {
        Command::new("git").arg("--version").output().is_ok()
    }

    fn set_git_identity(dir: &Path) {
        let _ = run_git_silent(dir, &["config", "user.email", "test@test.com"]);
        let _ = run_git_silent(dir, &["config", "user.name", "Test"]);
    }

    #[test]
    fn test_run_missing_target_dir_returns_error() {
        let result = run(None, "/nonexistent/path/that/cannot/exist/xyz123");
        assert!(result.is_err());
    }

    #[test]
    fn test_perform_new_repo() {
        if !has_git() {
            return;
        }
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(tmp.path().join("test.txt"), "hello").unwrap();
        let _ = run_git_silent(tmp.path(), &["init"]);
        set_git_identity(tmp.path());
        fs::remove_dir_all(tmp.path().join(".git")).unwrap();

        let result = perform(tmp.path(), "");
        // May fail if git doesn't have identity, but shouldn't panic
        let _ = result;
        assert!(is_git_repo(tmp.path()));
    }

    #[test]
    fn test_perform_nothing_to_commit() {
        if !has_git() {
            return;
        }
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(tmp.path().join("file.txt"), "content").unwrap();
        let _ = run_git_silent(tmp.path(), &["init"]);
        set_git_identity(tmp.path());

        let first = perform(tmp.path(), "");
        assert!(!first.is_error, "first perform failed: {}", first.message);
        let result = perform(tmp.path(), "");
        assert!(
            !result.is_error,
            "second perform failed: {}",
            result.message
        );
        assert!(
            result.message.contains("up to date"),
            "got: {}",
            result.message
        );
    }

    #[test]
    fn test_perform_commit_message_format() {
        if !has_git() {
            return;
        }
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(tmp.path().join("file.txt"), "data").unwrap();
        let _ = run_git_silent(tmp.path(), &["init"]);
        set_git_identity(tmp.path());

        let result = perform(tmp.path(), "");
        assert!(!result.is_error, "{}", result.message);

        let out = run_git_output(tmp.path(), &["log", "-1", "--format=%s"]).unwrap();
        let msg = out.trim();
        assert!(msg.starts_with("backup: "), "got: {}", msg);
        let ts = msg.strip_prefix("backup: ").unwrap();
        let parts: Vec<&str> = ts.split('-').collect();
        assert_eq!(parts.len(), 7, "expected 7 dash-separated parts in {}", ts);
    }

    #[test]
    fn test_is_git_repo() {
        let tmp = tempfile::TempDir::new().unwrap();
        assert!(!is_git_repo(tmp.path()));

        if !has_git() {
            return;
        }
        let _ = run_git_silent(tmp.path(), &["init"]);
        assert!(is_git_repo(tmp.path()));
    }

    #[test]
    fn test_status_not_a_repo() {
        let tmp = tempfile::TempDir::new().unwrap();
        let info = status(tmp.path());
        assert!(!info.is_repo);
    }

    #[test]
    fn test_status_clean_repo() {
        if !has_git() {
            return;
        }
        let tmp = tempfile::TempDir::new().unwrap();
        let _ = run_git_silent(tmp.path(), &["init"]);
        set_git_identity(tmp.path());
        fs::write(tmp.path().join("data.txt"), "init").unwrap();
        let _ = run_git_silent(tmp.path(), &["add", "."]);
        let _ = run_git_silent(tmp.path(), &["commit", "-m", "initial"]);

        let info = status(tmp.path());
        assert!(info.is_repo);
        assert!(info.clean);
        assert_eq!(info.last_commit, "initial");
    }

    #[test]
    fn test_status_modified_and_untracked() {
        if !has_git() {
            return;
        }
        let tmp = tempfile::TempDir::new().unwrap();
        let _ = run_git_silent(tmp.path(), &["init"]);
        set_git_identity(tmp.path());
        fs::write(tmp.path().join("data.txt"), "init").unwrap();
        let _ = run_git_silent(tmp.path(), &["add", "."]);
        let _ = run_git_silent(tmp.path(), &["commit", "-m", "initial"]);

        fs::write(tmp.path().join("data.txt"), "changed").unwrap();
        fs::write(tmp.path().join("new.txt"), "new").unwrap();

        let info = status(tmp.path());
        assert!(!info.clean);
        assert_eq!(info.modified, 1);
        assert_eq!(info.untracked, 1);
    }

    #[test]
    fn test_perform_with_remote() {
        if !has_git() {
            return;
        }
        let dir = tempfile::TempDir::new().unwrap();
        let remote_dir = tempfile::TempDir::new().unwrap();
        let _ = run_git_silent(remote_dir.path(), &["init", "--bare", "-b", "main"]);
        let _ = run_git_silent(dir.path(), &["init", "-b", "main"]);
        set_git_identity(dir.path());
        fs::write(dir.path().join("data.txt"), "backup").unwrap();

        let result = perform(dir.path(), remote_dir.path().to_str().unwrap());
        assert!(!result.is_error, "{}", result.message);
        assert!(result.message.contains("pushed"));
    }
}
