use std::path::Path;

#[derive(Debug, serde::Deserialize)]
pub struct Config {
    pub repo_path: String,
    #[serde(default = "default_poll_interval_secs")]
    pub poll_interval_secs: u64,
    #[serde(default = "default_max_iterations")]
    pub max_iterations: u32,
    #[serde(default = "default_max_iteration_secs")]
    pub max_iteration_secs: u64,
    #[serde(default = "default_worktree_base")]
    pub worktree_base: String,
}

fn default_poll_interval_secs() -> u64 {
    30
}
fn default_max_iterations() -> u32 {
    20
}
fn default_max_iteration_secs() -> u64 {
    1800
}
fn default_worktree_base() -> String {
    "/tmp/brainrunner-worktrees".to_string()
}

pub fn load_config(path: &Path) -> Result<Config, String> {
    let contents = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read config file {}: {}", path.display(), e))?;
    toml::from_str(&contents).map_err(|e| format!("invalid config file {}: {}", path.display(), e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_toml(contents: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "{}", contents).unwrap();
        f
    }

    #[test]
    fn malformed_toml_returns_error() {
        let f = write_toml("this is not valid toml ][[[");
        let result = load_config(f.path());
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(msg.contains("invalid config file"), "got: {msg}");
    }

    #[test]
    fn missing_file_returns_error() {
        let result = load_config(Path::new("/nonexistent/path/config.toml"));
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(msg.contains("cannot read config file"), "got: {msg}");
    }

    #[test]
    fn defaults_apply_when_fields_omitted() {
        let f = write_toml(r#"repo_path = "/some/repo""#);
        let cfg = load_config(f.path()).unwrap();
        assert_eq!(cfg.poll_interval_secs, 30);
        assert_eq!(cfg.max_iterations, 20);
        assert_eq!(cfg.max_iteration_secs, 1800);
        assert_eq!(cfg.worktree_base, "/tmp/brainrunner-worktrees");
    }

    #[test]
    fn loads_config_from_explicit_path() {
        let f = write_toml(
            r#"
            repo_path = "/home/kourgia/projects/gymnasou"
            poll_interval_secs = 60
            max_iterations = 10
            max_iteration_secs = 900
            worktree_base = "/tmp/custom-worktrees"
        "#,
        );
        let cfg = load_config(f.path()).unwrap();
        assert_eq!(cfg.repo_path, "/home/kourgia/projects/gymnasou");
        assert_eq!(cfg.poll_interval_secs, 60);
        assert_eq!(cfg.max_iterations, 10);
        assert_eq!(cfg.max_iteration_secs, 900);
        assert_eq!(cfg.worktree_base, "/tmp/custom-worktrees");
    }
}
