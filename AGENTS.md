## Project

`brainrunner` is a Rust daemon that polls a GitHub repo for issues labeled `ready-for-agent` and runs Claude Code autonomously on each one using the Ralph loop technique. It manages the full lifecycle — worktree creation, iteration loop, PR opening, and label state transitions — on a Raspberry Pi (aarch64).

## Common commands

```bash
cargo run                                    # build + run
cargo build --release                        # optimized build
cargo fmt --check && cargo clippy -- -D warnings  # full lint pass
cargo fmt                                    # auto-format
```

Before marking any task complete, run the full lint pass and confirm the project builds successfully:

```bash
cargo fmt --check && cargo clippy -- -D warnings && cargo build
```

Fix all errors and warnings before finishing.

## Agent skills

### Issue tracker

Issues live in GitHub Issues. See `docs/agents/issue-tracker.md`.

### Triage labels

Default canonical label vocabulary (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`). See `docs/agents/triage-labels.md`.

### Domain docs

Single-context repo: one `CONTEXT.md` + `docs/adr/` at the repo root. See `docs/agents/domain.md`.
