# TODO

## In Progress

- Validate the icon-embedded GPUI console against a green GitHub Actions artifact,
  then inspect the taskbar/window icon and recheck the 500 ms overlay.

## Verified

- The repository, tray application, tests, and initial cloud build are implemented.
- On 2026-10-25 a direct reversible probe using `0x318` produced `enabled=1`,
  `active=1` during Chrome SDR video playback, then restored `enabled=0`,
  `active=0`. The same sequence omitted by the shipped app had left `active=0`.
- GitHub Actions run `40176950707` is green. Its artifact (`SHA-256`
  `086D6F7C3093978A4B376734207590BEA0DFCB65BA866712C837EC23899F17E8`) was
  downloaded and exercised through the tray window's `WM_HOTKEY` path: HDR changed
  `0 -> 1 -> 0`, Chrome `hdrStatus` changed `0 -> 1 -> 0`, and peak, range, middle
  gray, contrast, and saturation remained unchanged.
- `Ctrl+Alt+H` was registered by the final process (a competing registration
  returned Windows error 1409). Initial private memory was 5.37 MiB; after opening
  the UXD session it was 12.32 MiB.
- Pixel comparison on a static SDR video showed the `0x318` path changed 32.5% of
  pixels by more than 10 RGB levels; the unrelated NvBackend `TrueHDR` path changed
  zero pixels.
- GitHub Actions run `40189900273` is green for the committed native layout and
  overlay baseline.
- GitHub Actions run `40207361394` is green for commit `97a8a07`; it produced host
  hash `56CA4809C9523511DE2FC426906E086289A05F186D1B78565D838CF522093AB6`
  and console hash `657A6376E9EC6946A0B2012F75FFC0234C47DC01797E4636B76914916C7CBC58`.
- The host and console named-pipe deadlocks were fixed by assigning each connection's
  complete read/write path to one thread. User testing confirmed that the tray,
  shortcut, and GPUI console handshake now work.
- The console icon master, PNG previews, multi-resolution ICO, resource script, and
  cloud build wiring are present locally; final cloud and shell-surface validation
  remain pending.
- The runnable calibration-dashboard prototype is in `prototype/gpui-console.html`
  and has been rendered at 900 x 700 for visual review.

## Pending

- Push the icon and README changes and wait for a green cloud run that validates the
  `embed-resource` dependency and `resources.rc` path resolution.
- Download the green artifact and verify the custom icon in the taskbar, title bar,
  executable properties, and notification-size shell surfaces.
- Download the artifact and verify the GPUI console opens, toggles HDR, saves
  settings, records a shortcut, reconnects, and exits completely on close.
- Visually verify the 500 ms overlay duration and 50% opacity in the final artifact.
- Verify that tray, shortcut, native overlay, startup entry, single instance,
  Explorer restart, rollback injection, no-HDR-display, Windows-HDR-off, and
  Surround behavior remain unchanged.
- Record host idle memory/CPU, console startup time, console private memory, and both
  executable sizes.

## Blockers

- None.

## Next Action

Commit the icon, README, lockfile, and documentation updates; push to `main`; then run
`gh run watch <id> --exit-status`. If green, download the artifact and inspect the
console icon plus the 500 ms overlay on the reference machine.
