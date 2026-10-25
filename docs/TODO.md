# TODO

## In Progress

- Implement the approved Rust/Win32 application and cloud build workflow.

## Pending

- Build the Release artifact in GitHub Actions and obtain a green run.
- Download the artifact and verify the reversible hardware transaction.
- Manually verify tray behavior, shortcut recording, startup, Explorer restart,
  supported Chrome playback, rollback paths, working set, and idle CPU.

## Blockers

- GitHub CLI authentication and private repository creation are still required.

## Next Action

Write the Rust package, unit tests, Windows resource icon, and GitHub Actions
workflow, then push and watch the first build.
