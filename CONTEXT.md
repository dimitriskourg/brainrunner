# brainrunner

A daemon that watches a GitHub repo for issues labeled `ready-for-agent` and runs Claude Code autonomously on each one using the Ralph loop technique.

## Language

**Agent Run**:
One full execution of the Ralph loop against a single issue, from label transition to completion or failure.
_Avoid_: job, task, execution

**Ralph Loop**:
The technique of invoking `claude --permission-mode acceptEdits -p "..."` repeatedly in a Worktree until the Completion Sigil appears in stdout, a non-zero exit occurs, the per-iteration timeout elapses, or the iteration cap is exhausted.
_Avoid_: agent loop, claude loop

**Completion Sigil**:
The literal string `<promise>COMPLETE</promise>` that Claude Code must emit anywhere in its stdout to signal a successful Agent Run. Detected by substring scan of the full stdout.
_Avoid_: completion signal, done marker

**Worktree**:
A git worktree created at the start of each Agent Run on branch `agent/<issue-number>`. All `claude` invocations and `gh` commands run from inside the Worktree. Deleted after the Agent Run completes.
_Avoid_: workspace, clone, sandbox

**Poll Cycle**:
One execution of `gh issue list --label ready-for-agent`, sorted by issue number ascending. Runs on a configurable interval (default: 30s). Picks at most one issue per cycle; subsequent issues wait for the next cycle.
_Avoid_: polling loop, tick

**Startup Sweep**:
On daemon startup: wipe the worktree base directory, then flip any issue still labeled `agent-running` to `agent-failed`. Recovers from interrupted Agent Runs before the first Poll Cycle begins.
_Avoid_: recovery, cleanup

**Issue State**:
One of four labels that represent an issue's position in the brainrunner lifecycle: `ready-for-agent`, `agent-running`, `agent-done`, `agent-failed`.
_Avoid_: status, phase

## Relationships

- A **Poll Cycle** picks at most one **Issue State** `ready-for-agent`, choosing the lowest issue number when multiple exist
- An **Agent Run** owns exactly one **Worktree** for its duration
- A **Completion Sigil** in stdout ends the **Ralph Loop** successfully; brainrunner then pushes the branch, opens a PR (`Closes #<N>`), and transitions the issue to `agent-done`
- A **Ralph Loop** fails immediately on a non-zero `claude` exit or per-iteration timeout; the issue transitions to `agent-failed`
- A **Startup Sweep** runs exactly once before the first **Poll Cycle**

## Issue State lifecycle

```
ready-for-agent  →  agent-running  →  agent-done    (Completion Sigil detected, PR opened)
                                   →  agent-failed   (non-zero exit, timeout, or iterations exhausted)
```

On startup, the Startup Sweep forces any stale `agent-running` → `agent-failed`.

## Example dialogue

> **Dev:** "What happens if Claude doesn't emit the Completion Sigil after 20 iterations?"
> **Domain expert:** "The Agent Run ends — brainrunner labels the issue `agent-failed` and removes the Worktree. No PR is opened."

> **Dev:** "If the daemon crashes mid-run, does it pick up where it left off?"
> **Domain expert:** "No. The Startup Sweep wipes the Worktree base and flips the stale `agent-running` issue to `agent-failed`. A human re-triages the issue when ready."

> **Dev:** "Does Claude open the PR itself?"
> **Domain expert:** "No — Claude only implements the change and emits the Completion Sigil. Brainrunner owns all lifecycle transitions: label mutations, branch push, and PR creation."
