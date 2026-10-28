# TODO

## In Progress

- Review and merge `feature/readonly-compatibility` after user approval; the branch
  is intentionally not auto-merged.

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
- GitHub Actions run `40210660195` is green for commit `ebdf9cd`. The artifact's host
  SHA-256 is `2A822BE7BF695A96A231E31B8A1DAF16D459ACAB52C755714AA0D2F18927C411`;
  the console SHA-256 is
  `58ADFE84F3AF612D89CD4330A2A7FCD5019E0AEF882881BCBC75E101B2819F37`.
- The green artifact's associated executable icon was extracted and visually checked
  as the custom calibration-ring icon. Launching the artifact produced a responsive
  GPUI console connected to the host, with the current HDR-off state, 400–1440 nits
  range, shortcut, and 500 ms overlay setting rendered correctly.
- GitHub Actions run `40224894598` is green for the HDR tuning implementation.
  Clippy, unit tests, host release, and GPUI release build all passed.
- A reversible IPC probe changed all four tuning fields
  (`1440 / 50 / 0 / 0` to `1400 / 45 / 5 / -5`), verified the readback, and restored
  the original values. Both `Ctrl+Alt+H` and `Ctrl+Alt+G` were registered; a competing
  registration returned error 1409 for each.
- Pressing a synthetic `Ctrl+Alt+G` reopened the console after its window process was
  closed.
- `Ctrl+Alt+G` now queues a visibility toggle to the connected console: active
  windows close, background windows activate. Final artifact verification remains.
- The runnable calibration-dashboard prototype is in `prototype/gpui-console.html`
  and has been rendered at 800 x 760 for visual review.
- `feature/readonly-compatibility` implements unknown-build read-only mode, protocol
  version `3`, compatibility diagnostics, raw `0x318` reporting, and disabled write
  controls. The host rejects tray, hotkey, IPC, and legacy-window writes before the
  worker queue when the mode is read-only.
- GitHub Actions run `40300042727` is green for commit `7aa7e9f`; it validated the
  first read-only compatibility implementation. It is an intermediate build, not the
  final artifact for this branch.
- GitHub Actions run `40318079147` is green for final commit `c57feef`; strict
  `fmt --check`, Clippy, tests, both release builds, packaging, and artifact upload
  all passed.
- Final artifact ZIP SHA-256 is
  `075BEBCAB99388F12B939EE456CF37B3D781C3B030A6C90DD1E658488E287888`.
  Host SHA-256 is
  `FC5C1C740A5C89CD43C3AACFA1F10AB41D5D30E6AFE1FF9247420036847A4546`
  (516,096 bytes); console SHA-256 is
  `D5A58D94DB2DF58824A72ECE805A508C54F41F2206C0FAD115C4F0294575439F`
  (17,759,232 bytes).
- A real-machine run of the final host passed Authenticode and NVIDIA signer
  organization extraction, loaded the verified-hash DLL as writable, and opened the
  GPUI diagnostics page. It showed DLL `8.17.15.5527`, probe `通过`, actual hash equal
  to the allowed hash, `已验证可写`, and raw `0x318` diagnostics. Console private
  memory was 95.74 MiB.
- A protocol-3 reversible tuning probe changed `1440 / 50 / 0 / 0` to
  `1400 / 45 / 5 / -5`, verified the readback, and restored the original values.
- The real-machine run caught a variable-size `CERT_INFO` buffer bug in signer
  extraction. Commit `c57feef` allocates the size reported by
  `CryptMsgGetParam(CMSG_SIGNER_CERT_INFO_PARAM)` and the final artifact passed the
  real DLL check.

## Pending

- Verify the unknown-build read-only UI against a real unknown signed DLL when such a
  build is available. This phase intentionally did not replace the local NVIDIA file;
  classification, IPC, and disabled-write behavior are covered by automated tests.
- Manually click `复制诊断` after any remaining layout/UX review. The formatted text
  is unit-tested, but the interactive clipboard click was not exercised because
  computer-use verification was stopped.
- Rebuild and visually verify the DPI-corrected console at the reference 125% scale;
  confirm the removed hero meter, visible ranges for all four controls, always-default
  `还原`, and absent maximize control.
- Verify toggles, tuning apply/reset, both hotkey recordings, foreground/background
  `Ctrl+Alt+G` behavior, settings save, reconnect, and complete console exit on the
  final artifact.
- Visually verify the 500 ms overlay duration and 50% opacity in the final artifact.
- Verify that tray, shortcut, native overlay, startup entry, single instance,
  Explorer restart, rollback injection, no-HDR-display, Windows-HDR-off, and
  Surround behavior remain unchanged.
- Record host idle memory/CPU, console startup time, console private memory, and both
  executable sizes.

## Blockers

- None.

## Next Action

Review draft PR #1, inspect the review diff, and merge only after explicit approval.
