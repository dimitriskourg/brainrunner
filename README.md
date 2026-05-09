# brainrunner

A daemon that watches a GitHub repo for issues labeled `ready-for-agent` and runs Claude Code autonomously on each one using the Ralph loop technique.

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/dimitriskourg/brainrunner/main/install.sh | bash
```

Requires `gh` (GitHub CLI) and `claude` (Claude Code CLI) to be installed and authenticated before running.

## Dev workflow

```bash
cargo run                                    # build + run
cargo build --release                        # optimized build
cargo fmt --check && cargo clippy -- -D warnings  # full lint pass
cargo fmt                                    # auto-format
```

## Configuration

Config lives at `~/.config/brainrunner/config.toml`:

```toml
repo_path = "/path/to/your/repo"
poll_interval_secs = 30
max_iterations = 20
max_iteration_secs = 1800
worktree_base = "/tmp/brainrunner-worktrees"
```

## Service management

```bash
sudo systemctl start brainrunner     # start
sudo systemctl stop brainrunner      # stop
sudo systemctl restart brainrunner   # restart
sudo systemctl disable brainrunner   # prevent start on boot
```

## Logs

```bash
journalctl -fu brainrunner
```
