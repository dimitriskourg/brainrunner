# GitHub labels as the Agent Run state store

GitHub issue labels (`agent-running`, `agent-done`, `agent-failed`) are the sole state store — no local database or file. The daemon is stateless between runs; the only state needed is "which issue is in-progress." GitHub is already the authoritative source for issues, so using labels keeps a single source of truth, survives daemon restarts, and provides free observability (anyone can see the label on the issue).

## Consequences

A Startup Sweep is required on every launch to recover from crashes: any issue still labeled `agent-running` is flipped to `agent-failed`. A crash between removing `ready-for-agent` and applying `agent-running` leaves an issue with no active brainrunner label — a human must re-triage it manually. This gap is accepted; it requires two API calls to fail in sequence, which is rare enough that automation isn't worth it.
