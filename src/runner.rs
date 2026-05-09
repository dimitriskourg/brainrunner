use std::path::Path;
use std::time::Duration;
use tracing::{debug, info};

const COMPLETION_SIGIL: &str = "<promise>COMPLETE</promise>";

pub struct RalphRunner {
    extra_path: Option<std::ffi::OsString>,
}

#[derive(Debug, PartialEq)]
pub enum RunOutcome {
    Success,
    ProcessError { code: Option<i32>, stderr: String },
    Timeout,
    Exhausted,
}

impl RalphRunner {
    pub fn new() -> Self {
        Self { extra_path: None }
    }

    pub fn with_extra_path(extra_path: impl Into<std::ffi::OsString>) -> Self {
        Self { extra_path: Some(extra_path.into()) }
    }

    pub async fn run(
        &self,
        prompt: &str,
        worktree_path: &Path,
        max_iterations: u32,
        max_iteration_secs: u64,
    ) -> RunOutcome {
        for iteration in 1..=max_iterations {
            info!(iteration, "starting claude iteration");

            let mut cmd = tokio::process::Command::new("claude");
            cmd.args(["--permission-mode", "acceptEdits", "-p", prompt])
                .current_dir(worktree_path);

            if let Some(extra) = &self.extra_path {
                let current_path = std::env::var_os("PATH").unwrap_or_default();
                let mut new_path = extra.clone();
                new_path.push(":");
                new_path.push(&current_path);
                cmd.env("PATH", new_path);
            }

            match tokio::time::timeout(
                Duration::from_secs(max_iteration_secs),
                cmd.output(),
            )
            .await
            {
                Err(_elapsed) => {
                    info!("claude timed out on iteration {iteration}");
                    return RunOutcome::Timeout;
                }
                Ok(Err(e)) => {
                    return RunOutcome::ProcessError {
                        code: None,
                        stderr: e.to_string(),
                    };
                }
                Ok(Ok(output)) => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    debug!(%stdout, "claude stdout");

                    if !output.status.success() {
                        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
                        info!(code = output.status.code(), "claude process error on iteration {iteration}");
                        return RunOutcome::ProcessError {
                            code: output.status.code(),
                            stderr,
                        };
                    }

                    if stdout.contains(COMPLETION_SIGIL) {
                        info!("completion sigil detected on iteration {iteration}");
                        return RunOutcome::Success;
                    }

                    info!(iteration, "iteration complete without sigil");
                }
            }
        }

        info!("exhausted {max_iterations} iterations without completion sigil");
        RunOutcome::Exhausted
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    fn write_fake_claude(dir: &tempfile::TempDir, body: &str) {
        let script_path = dir.path().join("claude");
        std::fs::write(&script_path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(
            &script_path,
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
    }

    #[tokio::test]
    async fn success_when_sigil_in_stdout() {
        let bin_dir = tempdir().unwrap();
        write_fake_claude(&bin_dir, "printf '<promise>COMPLETE</promise>'");
        let runner = RalphRunner::with_extra_path(bin_dir.path());
        let outcome = runner.run("test prompt", bin_dir.path(), 3, 30).await;
        assert_eq!(outcome, RunOutcome::Success);
    }

    #[tokio::test]
    async fn success_with_surrounding_text_and_trailing_newlines() {
        let bin_dir = tempdir().unwrap();
        write_fake_claude(
            &bin_dir,
            "echo 'some output'\necho '<promise>COMPLETE</promise>'\necho 'trailing'",
        );
        let runner = RalphRunner::with_extra_path(bin_dir.path());
        let outcome = runner.run("test prompt", bin_dir.path(), 1, 30).await;
        assert_eq!(outcome, RunOutcome::Success);
    }

    #[tokio::test]
    async fn process_error_on_non_zero_exit() {
        let bin_dir = tempdir().unwrap();
        write_fake_claude(&bin_dir, "echo 'error output' >&2\nexit 2");
        let runner = RalphRunner::with_extra_path(bin_dir.path());
        let outcome = runner.run("test prompt", bin_dir.path(), 3, 30).await;
        assert!(
            matches!(outcome, RunOutcome::ProcessError { code: Some(2), .. }),
            "expected ProcessError with code 2, got {outcome:?}"
        );
    }

    #[tokio::test]
    async fn process_error_does_not_consume_more_iterations() {
        let bin_dir = tempdir().unwrap();
        let counter_file = bin_dir.path().join("call_count");
        write_fake_claude(
            &bin_dir,
            &format!(
                r#"COUNT=$(cat "{path}" 2>/dev/null || echo 0)
echo $((COUNT + 1)) > "{path}"
exit 1"#,
                path = counter_file.display()
            ),
        );
        let runner = RalphRunner::with_extra_path(bin_dir.path());
        runner.run("test prompt", bin_dir.path(), 5, 30).await;
        let count: u32 = std::fs::read_to_string(&counter_file)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert_eq!(count, 1, "should have called claude only once, got {count}");
    }

    #[tokio::test]
    async fn timeout_when_iteration_exceeds_limit() {
        let bin_dir = tempdir().unwrap();
        write_fake_claude(&bin_dir, "sleep 60");
        let runner = RalphRunner::with_extra_path(bin_dir.path());
        let outcome = runner.run("test prompt", bin_dir.path(), 1, 1).await;
        assert_eq!(outcome, RunOutcome::Timeout);
    }

    #[tokio::test]
    async fn exhausted_when_cap_reached_without_sigil() {
        let bin_dir = tempdir().unwrap();
        write_fake_claude(&bin_dir, "echo 'no sigil here'");
        let runner = RalphRunner::with_extra_path(bin_dir.path());
        let outcome = runner.run("test prompt", bin_dir.path(), 3, 30).await;
        assert_eq!(outcome, RunOutcome::Exhausted);
    }
}
