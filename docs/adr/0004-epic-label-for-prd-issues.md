# PRD issues use `epic`, not `ready-for-agent`

Issues created by `to-prd` carry the `epic` label instead of `ready-for-agent`. The PRD is a planning artifact (an Epic) meant to stay open as a progress tracker while `to-issues` breaks it into agent-executable sub-issues. Applying `ready-for-agent` to the PRD causes brainrunner to pick it up as a single Agent Run, which is wrong. The alternative — teaching brainrunner to filter out epics even when labeled `ready-for-agent` — was rejected because the fix belongs at the source: a PRD was never meant to be agent-executable.
