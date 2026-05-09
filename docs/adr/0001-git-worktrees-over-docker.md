# Git worktrees over Docker for Agent Run isolation

Docker Engine 26 is available on the Pi, but Docker Desktop (which provides `docker sandbox run claude`) is not. Building a custom aarch64 image with Claude Code, forwarding `~/.claude`, SSH keys, and git config adds 3–4× the complexity of worktrees for no meaningful safety gain — brainrunner runs trusted, human-triaged issues on a personal machine. Worktrees give branch isolation for free, are instant (shared object store, no network clone), and are removed after each Agent Run.

## Consequences

Claude Code runs on bare metal with no process-level isolation. Acceptable for a personal Pi running trusted issues. A crash during an Agent Run leaves the worktree on disk; the Startup Sweep wipes the worktree base directory on next launch to recover cleanly.
