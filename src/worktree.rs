use std::path::PathBuf;

pub struct WorktreeManager {
    repo_path: PathBuf,
    worktree_base: PathBuf,
}

#[derive(Debug)]
pub enum WorktreeError {
    Io(std::io::Error),
    Git { code: Option<i32>, stderr: String },
}

impl std::fmt::Display for WorktreeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorktreeError::Io(e) => write!(f, "io error: {e}"),
            WorktreeError::Git { code, stderr } => {
                write!(f, "git exited {:?}: {stderr}", code)
            }
        }
    }
}

impl std::error::Error for WorktreeError {}

impl WorktreeManager {
    pub fn new(repo_path: impl Into<PathBuf>, worktree_base: impl Into<PathBuf>) -> Self {
        Self {
            repo_path: repo_path.into(),
            worktree_base: worktree_base.into(),
        }
    }

    pub async fn wipe_base(&self) -> Result<(), WorktreeError> {
        match tokio::fs::remove_dir_all(&self.worktree_base).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(WorktreeError::Io(e)),
        }
    }

    pub async fn create_worktree(&self, issue_n: u64) -> Result<PathBuf, WorktreeError> {
        let branch = format!("agent/{issue_n}");
        let worktree_path = self.worktree_base.join(format!("agent-{issue_n}"));
        run_git(
            &self.repo_path,
            &[
                "worktree",
                "add",
                "-b",
                &branch,
                worktree_path.to_str().unwrap(),
            ],
        )
        .await?;
        Ok(worktree_path)
    }

    pub async fn remove_worktree(&self, issue_n: u64) -> Result<(), WorktreeError> {
        let worktree_path = self.worktree_base.join(format!("agent-{issue_n}"));
        run_git(
            &self.repo_path,
            &[
                "worktree",
                "remove",
                "--force",
                worktree_path.to_str().unwrap(),
            ],
        )
        .await
    }
}

async fn run_git(cwd: &std::path::Path, args: &[&str]) -> Result<(), WorktreeError> {
    let out = tokio::process::Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .await
        .map_err(WorktreeError::Io)?;
    if out.status.success() {
        Ok(())
    } else {
        Err(WorktreeError::Git {
            code: out.status.code(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    async fn init_repo(path: &Path) {
        for args in [
            vec!["init"],
            vec!["config", "user.email", "test@test.com"],
            vec!["config", "user.name", "Test"],
        ] {
            tokio::process::Command::new("git")
                .args(&args)
                .current_dir(path)
                .status()
                .await
                .unwrap();
        }
        std::fs::write(path.join("readme"), b"init").unwrap();
        for args in [vec!["add", "."], vec!["commit", "-m", "init"]] {
            tokio::process::Command::new("git")
                .args(&args)
                .current_dir(path)
                .status()
                .await
                .unwrap();
        }
    }

    #[tokio::test]
    async fn wipe_base_no_ops_when_dir_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path().join("nonexistent");
        let mgr = WorktreeManager::new(tmp.path(), &base);
        mgr.wipe_base().await.unwrap();
    }

    #[tokio::test]
    async fn wipe_base_removes_existing_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path().join("worktrees");
        std::fs::create_dir(&base).unwrap();
        std::fs::write(base.join("sentinel"), b"data").unwrap();
        let mgr = WorktreeManager::new(tmp.path(), &base);
        mgr.wipe_base().await.unwrap();
        assert!(!base.exists(), "base dir should be gone");
    }

    #[tokio::test]
    async fn remove_worktree_removes_dir_and_deregisters() {
        let tmp = tempfile::tempdir().unwrap();
        init_repo(tmp.path()).await;
        let base = tmp.path().join("worktrees");
        std::fs::create_dir(&base).unwrap();
        let mgr = WorktreeManager::new(tmp.path(), &base);
        mgr.create_worktree(7).await.unwrap();
        let worktree_path = base.join("agent-7");
        assert!(worktree_path.is_dir());

        mgr.remove_worktree(7).await.unwrap();

        assert!(!worktree_path.exists(), "worktree dir should be gone");
        let out = tokio::process::Command::new("git")
            .args(["worktree", "list", "--porcelain"])
            .current_dir(tmp.path())
            .output()
            .await
            .unwrap();
        let list = String::from_utf8_lossy(&out.stdout);
        assert!(
            !list.contains("agent-7"),
            "worktree should be deregistered, got: {list}"
        );
    }

    #[tokio::test]
    async fn create_worktree_creates_dir_on_correct_branch() {
        let tmp = tempfile::tempdir().unwrap();
        init_repo(tmp.path()).await;
        let base = tmp.path().join("worktrees");
        std::fs::create_dir(&base).unwrap();
        let mgr = WorktreeManager::new(tmp.path(), &base);

        let path = mgr.create_worktree(5).await.unwrap();

        assert_eq!(path, base.join("agent-5"));
        assert!(path.is_dir(), "worktree dir should exist");
        let out = tokio::process::Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(&path)
            .output()
            .await
            .unwrap();
        let branch = String::from_utf8_lossy(&out.stdout).trim().to_string();
        assert_eq!(branch, "agent/5");
    }
}
