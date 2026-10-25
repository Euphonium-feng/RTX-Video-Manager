# TODO

## In Progress

- Validate the GPUI console, host IPC, dual-EXE packaging, and failure-path recovery
  against a green GitHub Actions artifact.

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
- The runnable calibration-dashboard prototype is in `prototype/gpui-console.html`
  and has been rendered at 900 x 700 for visual review.

## Pending

- Wait for a green dual-EXE cloud run and inspect compile, Clippy, formatting, and
  dependency-lock results.
- Validate ADR-009 after the local acceptance run showed the host main thread stuck
  behind a synchronous named-pipe write while the IPC thread held a pending read.
- Validate the GPUI-side single-I/O-thread change after the console handshake was
  observed blocking on its own concurrent pipe read/write.
- Download the artifact and verify the GPUI console opens, toggles HDR, saves
  settings, records a shortcut, reconnects, and exits completely on close.
- Verify that tray, shortcut, native overlay, startup entry, single instance,
  Explorer restart, rollback injection, no-HDR-display, Windows-HDR-off, and
  Surround behavior remain unchanged.
- Record host idle memory/CPU, console startup time, console private memory, and both
  executable sizes.

## Blockers

- None.

## Next Action

Watch the first GPUI cloud run, fix the actual compile/lint cause, and download the
green dual-EXE artifact for Windows acceptance.
