# TODO

## In Progress

- Complete the remaining manual tray and failure-path checks.

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

## Pending

- Visually confirm the Chrome brightness/color difference and measure activation
  latency from the physical shortcut.
- Verify tray icons and menus, shortcut recording, restart persistence, single
  instance opening settings, Explorer restart, startup entry, rollback injection,
  no-HDR-display/Windows-HDR-off/Surround states, and idle CPU.

## Blockers

- None.

## Next Action

Run the physical shortcut in Chrome, then complete the remaining manual checks.
