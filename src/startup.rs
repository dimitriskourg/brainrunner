use crate::github::{GithubClient, GithubError};
use crate::worktree::{WorktreeError, WorktreeManager};

#[derive(Debug)]
pub enum StartupError {
    Worktree(WorktreeError),
    Github(GithubError),
}

impl std::fmt::Display for StartupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StartupError::Worktree(e) => write!(f, "worktree error: {e}"),
            StartupError::Github(e) => write!(f, "github error: {e}"),
        }
    }
}

impl std::error::Error for StartupError {}

pub async fn startup_sweep(
    worktrees: &WorktreeManager,
    github: &GithubClient,
) -> Result<(), StartupError> {
    worktrees
        .wipe_base()
        .await
        .map_err(StartupError::Worktree)?;
    let running = github
        .list_issues_by_label("agent-running")
        .await
        .map_err(StartupError::Github)?;
    for issue in running {
        github
            .remove_label(issue.number, "agent-running")
            .await
            .map_err(StartupError::Github)?;
        github
            .apply_label(issue.number, "agent-failed")
            .await
            .map_err(StartupError::Github)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn startup_sweep_wipes_base_before_querying_github() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path().join("worktrees");
        std::fs::create_dir(&base).unwrap();
        std::fs::write(base.join("sentinel"), b"data").unwrap();

        let worktrees = WorktreeManager::new(tmp.path(), &base);
        let github = GithubClient::new(tmp.path()); // no real remote

        let _ = startup_sweep(&worktrees, &github).await;

        assert!(
            !base.exists(),
            "worktree base should be wiped even if gh fails"
        );
    }

    #[tokio::test]
    async fn startup_sweep_returns_github_error_when_gh_unavailable() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path().join("worktrees");
        let worktrees = WorktreeManager::new(tmp.path(), &base);
        let github = GithubClient::new(tmp.path()); // no real remote

        let result = startup_sweep(&worktrees, &github).await;

        assert!(matches!(result, Err(StartupError::Github(_))));
    }
}
