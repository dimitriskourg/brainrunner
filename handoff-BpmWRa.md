# Handoff: brainrunner — Design Session

## What this is

A design grilling session for a new standalone Rust daemon called **`brainrunner`**. All major architectural decisions have been made. The project does not exist yet as a repo. The next agent should create the repo, scaffold the Rust project, and implement the daemon.

## Context

The user is **dimitriskourg** (Raspberry Pi, aarch64, Debian, Docker Engine 26 available). They maintain a Nuxt/Supabase project called **Gymnasou** (`/home/kourgia/projects/gymnasou`). That repo uses GitHub issues as its issue tracker (`gh` CLI is configured) and has a triage label convention defined in `docs/agents/triage-labels.md`.

The user read three articles about the **Ralph** technique (AFK AI coding loops) and wants to build a daemon that automates it — triggered by GitHub issue labels rather than manual shell scripts.

The user also wants to learn Rust. `brainrunner` is their first Rust project.

---

## What brainrunner does

1. Polls a GitHub repo every 30 seconds for issues labeled `ready-for-agent`
2. Picks the first one it finds
3. Creates a git worktree on branch `agent/<issue-number>`
4. Runs Claude Code in a Ralph loop inside that worktree (max 20 iterations)
5. On success: pushes branch, opens PR referencing the issue, labels it `agent-done`
6. On failure (exhausted iterations): labels it `agent-failed`
7. Removes the worktree and goes back to polling

---

## All locked decisions

| Decision | Choice | Rationale |
|---|---|---|
| Project type | Standalone Rust daemon | Reusable across any repo; user wants to learn Rust |
| Project name | `brainrunner` | Fun, not tied to "Ralph", short enough to type |
| Trigger | Polling loop every 30s | Webhooks overkill — a 30–45 min agent run makes latency irrelevant |
| Concurrency | Serial queue — one issue at a time | Cost and resource control; simplicity |
| State store | GitHub labels | Durable across restarts, free observability, no local DB |
| Isolation | Git worktrees (`agent/<number>` branch) | Docker Desktop unavailable on Pi; worktrees give branch isolation for free |
| Completion signal | Open PR + label `agent-done` | Human review gate before merge |
| Failure signal | Label `agent-failed` | Consistent with exhausted-iterations policy |
| Iteration cap | 20 (configurable in TOML) | Matt Pocock's recommendation for medium tasks |
| Prompt input | Issue title + body + all comments | Comments carry clarification added post-triage |
| Configuration | TOML config file | Better than CLI flags for a systemd service |
| Startup sweep | Flip stale `agent-running` → `agent-failed` | Safe recovery from crash/reboot |
| Runtime target | Rust + tokio, 24/7 on Raspberry Pi (aarch64) | Low memory, no GC, stable long-running process |

---

## Issue label lifecycle

```
ready-for-agent  →  agent-running  →  agent-done    (PR opened)
                                   →  agent-failed   (exhausted or crash)
```

- On startup: any existing `agent-running` issue is immediately flipped to `agent-failed` (Startup Sweep)
- The daemon never touches `ready-for-human`, `needs-triage`, `needs-info`, or `wontfix`

---

## CONTEXT.md (to be placed at the root of the new repo)

```markdown
# brainrunner

A daemon that watches a GitHub repo for issues labeled `ready-for-agent`
and runs Claude Code autonomously on each one using the Ralph loop technique.

## Glossary

### Agent Run
One full execution of the Ralph loop against a single issue. An Agent Run
begins when an issue transitions from `ready-for-agent` to `agent-running`,
and ends when Claude emits the Completion Sigil, the iteration cap is reached,
or the daemon crashes.

### Ralph Loop
The technique of running `claude --permission-mode acceptEdits -p "..."`
repeatedly in a Worktree, with Claude choosing its own next action each
iteration, until it emits the Completion Sigil or the cap is exhausted.

### Completion Sigil
The literal string `<promise>COMPLETE</promise>` that Claude Code must emit
in its final output to signal a successful Agent Run.

### Worktree
A git worktree created at the start of each Agent Run on branch
`agent/<issue-number>`. Provides an isolated workspace without a full clone.
Deleted after the Agent Run completes.

### Poll Cycle
One execution of `gh issue list --label ready-for-agent`. Runs on a
configurable interval (default: 30s). Only one issue is picked per cycle;
subsequent issues wait for the next cycle.

### Startup Sweep
On daemon startup, any issue still labeled `agent-running` is immediately
flipped to `agent-failed`. These represent interrupted Agent Runs from a
previous daemon session.

### Issue States
The four labels that represent an issue's lifecycle in brainrunner:

| Label | Meaning |
|---|---|
| `ready-for-agent` | Fully specified, awaiting an Agent Run |
| `agent-running` | Agent Run in progress |
| `agent-done` | Agent Run completed — PR opened |
| `agent-failed` | Agent Run exhausted iterations or was interrupted |
```

---

## ADRs (drafted, not yet written to files)

### ADR-0001: Git worktrees over Docker for Agent Run isolation

**Decision**: Use `git worktree` for isolation instead of Docker containers.

**Context**: Docker Engine 26 is available on the Pi, but Docker Desktop (which provides `docker sandbox run claude`) is not. Building a custom aarch64 Docker image with Claude Code, forwarding auth (`~/.claude`), SSH keys, and git config adds 3–4× complexity over worktrees.

**Consequences**: Claude Code runs on bare metal (no process-level isolation). Acceptable for a personal Pi running trusted issues. Worktrees provide branch isolation for free and are instant (shared object store, no network clone).

---

### ADR-0002: GitHub labels as the Agent Run state store

**Decision**: Use GitHub issue labels (`agent-running`, `agent-done`, `agent-failed`) as the sole state store — no local database or file.

**Context**: The daemon is stateless between runs. The only state needed is "which issue is in-progress." GitHub is already the authoritative source for issues; using labels keeps a single source of truth, survives daemon restarts, and gives free observability (anyone can see the label on the issue).

**Consequences**: A startup sweep is required to recover from crashes (flip stale `agent-running` → `agent-failed`). Label API calls must be atomic-enough — a crash between removing `ready-for-agent` and applying `agent-running` leaves an issue with no label, which a human must re-triage manually.

---

## Ralph loop prompt shape

```
You are working on issue #<N> from GitHub repo <owner>/<repo>.

Title: <title>
Body:
<body>

Comments:
<comments>

Instructions:
1. Read the codebase and understand what needs to be done.
2. Implement the solution in small, focused commits.
3. Run all feedback loops (pnpm typecheck, pnpm lint) and fix any errors before committing.
4. When the implementation is complete, push branch agent/<N> and open a PR referencing #<N>.
5. Output <promise>COMPLETE</promise> as your final message.

ONLY work on this issue. One logical change per commit. Quality over speed.
If all work is done before reaching the iteration limit, output <promise>COMPLETE</promise> immediately.
```

---

## Config file shape (`config.toml`)

```toml
repo_path = "/home/kourgia/projects/gymnasou"
poll_interval_secs = 30
max_iterations = 20
worktree_base = "/tmp/brainrunner-worktrees"
```

---

## Suggested next steps for the implementing agent

1. `cargo new brainrunner` — init the Rust project
2. Add dependencies to `Cargo.toml`: `tokio`, `serde`, `toml`, `reqwest` (or just shell out to `gh` CLI)
3. Write `CONTEXT.md` at repo root (content above)
4. Write ADR-0001 and ADR-0002 to `docs/adr/`
5. Implement the startup sweep (flip stale `agent-running` → `agent-failed`)
6. Implement the poll loop (`gh issue list --label ready-for-agent --json number,title,body,comments`)
7. Implement worktree creation (`git worktree add`), Claude Code invocation, worktree cleanup
8. Implement PR creation and label mutation via `gh` CLI
9. Wire config loading from `config.toml`
10. Test against the Gymnasou repo

## Important note on `gh` usage

The `gh` CLI automatically infers the GitHub repo from `git remote -v` when run inside a git working tree. All `gh` commands should be run with the worktree path as the working directory (or the repo root for label/issue operations). No need to hardcode `owner/repo` in the binary — derive it from `gh repo view --json nameWithOwner` at startup.
