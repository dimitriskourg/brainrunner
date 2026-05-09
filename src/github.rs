use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Issue {
    pub number: u64,
    pub title: String,
}

pub struct GithubClient {
    cwd: PathBuf,
}

#[derive(Debug)]
pub enum GithubError {
    Io(std::io::Error),
    Gh { code: Option<i32>, stderr: String },
    Parse(String),
}

impl std::fmt::Display for GithubError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GithubError::Io(e) => write!(f, "io error: {e}"),
            GithubError::Gh { code, stderr } => write!(f, "gh exited {:?}: {stderr}", code),
            GithubError::Parse(msg) => write!(f, "parse error: {msg}"),
        }
    }
}

impl std::error::Error for GithubError {}

impl GithubClient {
    pub fn new(cwd: impl Into<PathBuf>) -> Self {
        Self { cwd: cwd.into() }
    }

    pub async fn list_ready_issues(&self) -> Result<Vec<Issue>, GithubError> {
        let out = run_gh(
            &self.cwd,
            &["issue", "list", "--label", "ready-for-agent", "--json", "number,title"],
        )
        .await?;
        parse_issues(&out)
    }

    pub async fn apply_label(&self, issue_n: u64, label: &str) -> Result<(), GithubError> {
        run_gh(
            &self.cwd,
            &["issue", "edit", &issue_n.to_string(), "--add-label", label],
        )
        .await?;
        Ok(())
    }

    pub async fn remove_label(&self, issue_n: u64, label: &str) -> Result<(), GithubError> {
        run_gh(
            &self.cwd,
            &["issue", "edit", &issue_n.to_string(), "--remove-label", label],
        )
        .await?;
        Ok(())
    }

    pub async fn push_branch(&self, branch: &str) -> Result<(), GithubError> {
        let out = tokio::process::Command::new("git")
            .args(["push", "origin", branch])
            .current_dir(&self.cwd)
            .output()
            .await
            .map_err(GithubError::Io)?;
        if out.status.success() {
            Ok(())
        } else {
            Err(GithubError::Gh {
                code: out.status.code(),
                stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
            })
        }
    }

    pub async fn open_pr(&self, issue_n: u64, title: &str) -> Result<(), GithubError> {
        let body = format!("Closes #{issue_n}");
        run_gh(
            &self.cwd,
            &["pr", "create", "--title", title, "--body", &body],
        )
        .await?;
        Ok(())
    }
}

pub(crate) fn parse_issues(json: &str) -> Result<Vec<Issue>, GithubError> {
    #[derive(serde::Deserialize)]
    struct Raw {
        number: u64,
        title: String,
    }
    let mut raw: Vec<Raw> = serde_json::from_str(json)
        .map_err(|e| GithubError::Parse(e.to_string()))?;
    raw.sort_by_key(|r| r.number);
    Ok(raw.into_iter().map(|r| Issue { number: r.number, title: r.title }).collect())
}

async fn run_gh(cwd: &std::path::Path, args: &[&str]) -> Result<String, GithubError> {
    let out = tokio::process::Command::new("gh")
        .args(args)
        .current_dir(cwd)
        .output()
        .await
        .map_err(GithubError::Io)?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(GithubError::Gh {
            code: out.status.code(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn list_ready_issues_gh_failure_returns_error() {
        let tmp = tempfile::tempdir().unwrap();
        let client = GithubClient::new(tmp.path());
        // tmp dir has no git remote, so gh will fail
        let result = client.list_ready_issues().await;
        assert!(matches!(result, Err(GithubError::Gh { .. })));
    }

    #[test]
    fn parse_issues_invalid_json_returns_parse_error() {
        let result = parse_issues("not json at all");
        assert!(matches!(result, Err(GithubError::Parse(_))));
    }

    #[test]
    fn parse_issues_empty_list_returns_empty_vec() {
        let issues = parse_issues("[]").unwrap();
        assert!(issues.is_empty());
    }

    #[test]
    fn parse_issues_sorts_ascending() {
        let json = r#"[{"number":3,"title":"Third"},{"number":1,"title":"First"},{"number":2,"title":"Second"}]"#;
        let issues = parse_issues(json).unwrap();
        assert_eq!(issues.len(), 3);
        assert_eq!(issues[0].number, 1);
        assert_eq!(issues[1].number, 2);
        assert_eq!(issues[2].number, 3);
    }
}
