# TODO

## In Progress

- Verify the final GitHub Actions artifact after initializing STA COM and checking
  the NVIDIA UXD service before opening the NvCpl session.

## Verified

- The repository, tray application, tests, and initial cloud build are implemented.
- On 2026-10-25 a direct reversible probe using `0x318` produced `enabled=1`,
  `active=1` during Chrome SDR video playback, then restored `enabled=0`,
  `active=0`. The same sequence omitted by the shipped app had left `active=0`.

## Pending

- Push the fix and obtain a green GitHub Actions run.
- Download that artifact and verify the final app changes NVIDIA App's HDR state
  and Chrome playback becomes active within one second.
- Verify restart persistence, VSR and six tuning fields, tray behavior, shortcut
  conflict, single instance, Explorer restart, startup entry, rollback, working
  set, and idle CPU.

## Blockers

- None.

## Next Action

Commit and push the COM/UXD fix, then watch the GitHub Actions run with
`gh run watch --exit-status`.
