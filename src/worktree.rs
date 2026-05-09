use std::path::{Path, PathBuf};
use tokio::process::Command;

#[derive(Debug)]
pub struct WorktreeManager {
    repo_path: PathBuf,
    worktree_base: PathBuf,
}

#[derive(Debug)]
pub enum WorktreeError {
    Io(std::io::Error),
    GitCommandFailed {
        operation: &'static str,
        stderr: String,
    },
}

impl std::fmt::Display for WorktreeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "io error: {}", err),
            Self::GitCommandFailed { operation, stderr } => {
                write!(f, "git command failed during {}: {}", operation, stderr)
            }
        }
    }
}

impl std::error::Error for WorktreeError {}

impl From<std::io::Error> for WorktreeError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl WorktreeManager {
    pub fn new(repo_path: impl Into<PathBuf>, worktree_base: impl Into<PathBuf>) -> Self {
        Self {
            repo_path: repo_path.into(),
            worktree_base: worktree_base.into(),
        }
    }

    pub fn worktree_path(&self, issue_n: u64) -> PathBuf {
        self.worktree_base.join(format!("agent-{}", issue_n))
    }

    pub async fn wipe_base(&self) -> Result<(), WorktreeError> {
        if !tokio::fs::try_exists(&self.worktree_base).await? {
            return Ok(());
        }
        tokio::fs::remove_dir_all(&self.worktree_base).await?;
        Ok(())
    }

    pub async fn create_worktree(&self, issue_n: u64) -> Result<PathBuf, WorktreeError> {
        let worktree_path = self.worktree_path(issue_n);
        tokio::fs::create_dir_all(&self.worktree_base).await?;

        let branch = format!("agent/{}", issue_n);
        run_git_worktree(
            &self.repo_path,
            &["add", "-b", &branch, &path_arg(&worktree_path)],
            "create_worktree",
        )
        .await?;

        Ok(worktree_path)
    }

    pub async fn remove_worktree(&self, issue_n: u64) -> Result<(), WorktreeError> {
        let worktree_path = self.worktree_path(issue_n);
        run_git_worktree(
            &self.repo_path,
            &["remove", "--force", &path_arg(&worktree_path)],
            "remove_worktree",
        )
        .await?;
        Ok(())
    }
}

async fn run_git_worktree(
    repo_path: &Path,
    args: &[&str],
    operation: &'static str,
) -> Result<(), WorktreeError> {
    let output = Command::new("git")
        .current_dir(repo_path)
        .arg("worktree")
        .args(args)
        .output()
        .await?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(WorktreeError::GitCommandFailed { operation, stderr })
}

fn path_arg(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command as StdCommand;
    use tempfile::TempDir;

    fn run_git(repo_path: &Path, args: &[&str]) {
        let output = StdCommand::new("git")
            .current_dir(repo_path)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn setup_repo() -> TempDir {
        let repo = TempDir::new().unwrap();
        run_git(repo.path(), &["init"]);
        run_git(repo.path(), &["config", "user.email", "test@example.com"]);
        run_git(repo.path(), &["config", "user.name", "Test User"]);
        std::fs::write(repo.path().join("README.md"), "seed").unwrap();
        run_git(repo.path(), &["add", "README.md"]);
        run_git(repo.path(), &["commit", "-m", "seed"]);
        repo
    }

    #[tokio::test]
    async fn create_worktree_creates_expected_path_and_branch() {
        let repo = setup_repo();
        let worktree_base = repo.path().join("worktrees");
        let manager = WorktreeManager::new(repo.path(), &worktree_base);

        let worktree_path = manager.create_worktree(7).await.unwrap();

        assert_eq!(worktree_path, worktree_base.join("agent-7"));
        assert!(worktree_path.exists());

        let branch_output = StdCommand::new("git")
            .current_dir(&worktree_path)
            .args(["branch", "--show-current"])
            .output()
            .unwrap();
        assert!(branch_output.status.success());
        assert_eq!(
            String::from_utf8_lossy(&branch_output.stdout).trim(),
            "agent/7"
        );
    }

    #[tokio::test]
    async fn remove_worktree_removes_directory_and_deregisters() {
        let repo = setup_repo();
        let worktree_base = repo.path().join("worktrees");
        let manager = WorktreeManager::new(repo.path(), &worktree_base);

        let worktree_path = manager.create_worktree(8).await.unwrap();
        assert!(worktree_path.exists());

        manager.remove_worktree(8).await.unwrap();

        assert!(!worktree_path.exists());

        let list_output = StdCommand::new("git")
            .current_dir(repo.path())
            .args(["worktree", "list"])
            .output()
            .unwrap();
        assert!(list_output.status.success());
        let listed = String::from_utf8_lossy(&list_output.stdout);
        assert!(!listed.contains(&worktree_path.to_string_lossy().to_string()));
    }

    #[tokio::test]
    async fn wipe_base_removes_directory_and_noops_if_missing() {
        let repo = setup_repo();
        let worktree_base = repo.path().join("worktrees");
        std::fs::create_dir_all(worktree_base.join("nested")).unwrap();
        std::fs::write(worktree_base.join("nested/file.txt"), "x").unwrap();
        let manager = WorktreeManager::new(repo.path(), &worktree_base);

        manager.wipe_base().await.unwrap();
        assert!(!worktree_base.exists());

        manager.wipe_base().await.unwrap();
    }
}
