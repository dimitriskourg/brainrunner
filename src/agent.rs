use tracing::{info, warn};

use crate::github::{GithubClient, GithubError, Issue};
use crate::prompt::build_prompt;
use crate::runner::{RalphRunner, RunOutcome};
use crate::worktree::{WorktreeError, WorktreeManager};

#[derive(Debug)]
pub enum AgentRunError {
    Worktree(WorktreeError),
    Github(GithubError),
}

impl std::fmt::Display for AgentRunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentRunError::Worktree(e) => write!(f, "worktree error: {e}"),
            AgentRunError::Github(e) => write!(f, "github error: {e}"),
        }
    }
}

impl std::error::Error for AgentRunError {}

/// Executes one full Agent Run for `issue`. Returns Ok(()) whether the run
/// succeeded or failed — both outcomes result in correct label transitions.
/// Returns Err only when an infrastructure operation (gh/git) cannot complete.
pub async fn run_one(
    issue: &Issue,
    worktrees: &WorktreeManager,
    github: &GithubClient,
    runner: &RalphRunner,
    max_iterations: u32,
    max_iteration_secs: u64,
) -> Result<(), AgentRunError> {
    info!(issue = issue.number, "starting agent run");

    github.apply_label(issue.number, "agent-running").await.map_err(AgentRunError::Github)?;
    github.remove_label(issue.number, "ready-for-agent").await.map_err(AgentRunError::Github)?;

    let worktree_path = worktrees
        .create_worktree(issue.number)
        .await
        .map_err(AgentRunError::Worktree)?;

    let (body, comments) = github
        .get_issue_details(issue.number)
        .await
        .map_err(AgentRunError::Github)?;
    let comment_refs: Vec<&str> = comments.iter().map(|s| s.as_str()).collect();
    let prompt = build_prompt(issue.number, &issue.title, &body, &comment_refs);

    let outcome = runner
        .run(&prompt, &worktree_path, max_iterations, max_iteration_secs)
        .await;

    let result = match outcome {
        RunOutcome::Success => {
            let branch = format!("agent/{}", issue.number);
            let push_result = github
                .push_branch(&branch)
                .await
                .map_err(AgentRunError::Github);
            if let Err(e) = push_result {
                warn!(issue = issue.number, "push failed: {e}");
                transition_to_failed(issue.number, github).await?;
                Err(e)
            } else {
                github
                    .open_pr(issue.number, &issue.title)
                    .await
                    .map_err(AgentRunError::Github)?;
                github
                    .remove_label(issue.number, "agent-running")
                    .await
                    .map_err(AgentRunError::Github)?;
                github
                    .apply_label(issue.number, "agent-done")
                    .await
                    .map_err(AgentRunError::Github)?;
                info!(issue = issue.number, "agent run succeeded");
                Ok(())
            }
        }
        other => {
            info!(issue = issue.number, outcome = ?other, "agent run failed");
            transition_to_failed(issue.number, github).await?;
            Ok(())
        }
    };

    if let Err(e) = worktrees.remove_worktree(issue.number).await {
        warn!(issue = issue.number, "failed to remove worktree: {e}");
    }

    result
}

async fn transition_to_failed(
    issue_n: u64,
    github: &GithubClient,
) -> Result<(), AgentRunError> {
    github
        .remove_label(issue_n, "agent-running")
        .await
        .map_err(AgentRunError::Github)?;
    github
        .apply_label(issue_n, "agent-failed")
        .await
        .map_err(AgentRunError::Github)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    async fn init_repo(path: &std::path::Path) {
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

    fn write_fake_claude(dir: &TempDir, body: &str) {
        let p = dir.path().join("claude");
        std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    /// Writes a fake `gh` script. All invocations are appended to `call_log`.
    /// Returns sensible JSON for `issue view` and `issue list`, exits 0 for
    /// `issue edit` and `pr create`.
    fn write_fake_gh(dir: &TempDir, call_log: &std::path::Path) {
        let log = call_log.to_str().unwrap();
        let p = dir.path().join("gh");
        std::fs::write(&p, format!(
            r#"#!/bin/sh
echo "$*" >> "{log}"
case "$1 $2" in
    "issue view")   echo '{{"body":"test body","comments":[]}}' ;;
    "issue list")   echo '[]' ;;
    "issue edit")   exit 0 ;;
    "pr create")    exit 0 ;;
    *)              exit 1 ;;
esac
"#
        ))
        .unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn read_calls(call_log: &std::path::Path) -> String {
        std::fs::read_to_string(call_log).unwrap_or_default()
    }

    struct TestEnv {
        repo_dir: TempDir,
        bin_dir: TempDir,
        call_log: std::path::PathBuf,
    }

    impl TestEnv {
        async fn new() -> Self {
            let repo_dir = tempfile::tempdir().unwrap();
            init_repo(repo_dir.path()).await;
            let worktree_base = repo_dir.path().join("worktrees");
            std::fs::create_dir(&worktree_base).unwrap();
            let bin_dir = tempfile::tempdir().unwrap();
            let call_log = bin_dir.path().join("call_log");
            TestEnv { repo_dir, bin_dir, call_log }
        }

        fn worktrees(&self) -> WorktreeManager {
            let base = self.repo_dir.path().join("worktrees");
            WorktreeManager::new(self.repo_dir.path(), base)
        }

        fn github(&self) -> GithubClient {
            GithubClient::with_extra_path(self.repo_dir.path(), self.bin_dir.path())
        }

        fn runner(&self) -> RalphRunner {
            RalphRunner::with_extra_path(self.bin_dir.path())
        }
    }

    #[tokio::test]
    async fn run_one_applies_agent_running_label() {
        let env = TestEnv::new().await;
        write_fake_gh(&env.bin_dir, &env.call_log);
        write_fake_claude(&env.bin_dir, "echo 'no sigil'");

        let issue = Issue { number: 42, title: "Test issue".to_string() };
        run_one(&issue, &env.worktrees(), &env.github(), &env.runner(), 1, 30).await.ok();

        let calls = read_calls(&env.call_log);
        assert!(
            calls.contains("issue edit 42 --add-label agent-running"),
            "expected agent-running applied, got:\n{calls}"
        );
    }

    #[tokio::test]
    async fn run_one_removes_ready_for_agent_label() {
        let env = TestEnv::new().await;
        write_fake_gh(&env.bin_dir, &env.call_log);
        write_fake_claude(&env.bin_dir, "echo 'no sigil'");

        let issue = Issue { number: 42, title: "Test issue".to_string() };
        run_one(&issue, &env.worktrees(), &env.github(), &env.runner(), 1, 30).await.ok();

        let calls = read_calls(&env.call_log);
        assert!(
            calls.contains("issue edit 42 --remove-label ready-for-agent"),
            "expected ready-for-agent removed, got:\n{calls}"
        );
    }

    #[tokio::test]
    async fn run_one_creates_worktree_on_agent_branch() {
        let env = TestEnv::new().await;
        write_fake_gh(&env.bin_dir, &env.call_log);
        write_fake_claude(&env.bin_dir, "echo 'no sigil'");

        let issue = Issue { number: 7, title: "Test".to_string() };
        run_one(&issue, &env.worktrees(), &env.github(), &env.runner(), 1, 30).await.ok();

        let out = tokio::process::Command::new("git")
            .args(["branch", "--list", "agent/7"])
            .current_dir(env.repo_dir.path())
            .output()
            .await
            .unwrap();
        let branches = String::from_utf8_lossy(&out.stdout);
        assert!(branches.contains("agent/7"), "expected branch agent/7, got: {branches}");
    }

    #[tokio::test]
    async fn run_one_ralph_failure_applies_agent_failed_and_removes_worktree() {
        let env = TestEnv::new().await;
        write_fake_gh(&env.bin_dir, &env.call_log);
        write_fake_claude(&env.bin_dir, "exit 1");

        let issue = Issue { number: 5, title: "Failing issue".to_string() };
        run_one(&issue, &env.worktrees(), &env.github(), &env.runner(), 1, 30).await.ok();

        let calls = read_calls(&env.call_log);
        assert!(
            calls.contains("issue edit 5 --add-label agent-failed"),
            "expected agent-failed applied, got:\n{calls}"
        );
        let worktree_path = env.repo_dir.path().join("worktrees").join("agent-5");
        assert!(!worktree_path.exists(), "worktree should be removed after failure");
    }

    #[tokio::test]
    async fn run_one_success_applies_agent_done_opens_pr_removes_worktree() {
        let env = TestEnv::new().await;
        write_fake_gh(&env.bin_dir, &env.call_log);
        // fake git that succeeds for push (records call and exits 0)
        let fake_git = env.bin_dir.path().join("git");
        std::fs::write(&fake_git, "#!/bin/sh\necho \"$*\" >> \"$(dirname $0)/git_log\"\nexit 0\n").unwrap();
        std::fs::set_permissions(&fake_git, std::fs::Permissions::from_mode(0o755)).unwrap();
        write_fake_claude(&env.bin_dir, "printf '<promise>COMPLETE</promise>'");

        let issue = Issue { number: 9, title: "Success issue".to_string() };
        run_one(&issue, &env.worktrees(), &env.github(), &env.runner(), 1, 30).await.ok();

        let calls = read_calls(&env.call_log);
        assert!(
            calls.contains("issue edit 9 --add-label agent-done"),
            "expected agent-done applied, got:\n{calls}"
        );
        assert!(
            calls.contains("pr create --title Success issue --body Closes #9"),
            "expected PR opened, got:\n{calls}"
        );
        let worktree_path = env.repo_dir.path().join("worktrees").join("agent-9");
        assert!(!worktree_path.exists(), "worktree should be removed after success");
    }
}
