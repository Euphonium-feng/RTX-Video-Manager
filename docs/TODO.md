# TODO

## In Progress

- `feature/rtx-video-manager` implements the `RTX Video Manager` rename, HDR/VSR
  dual-function model, protocol version `4`, three-hotkey grouping, the five-page
  GPUI console, and the new dual-channel product icon.
- Real-machine review found that the GPUI console is functionally connected but its
  visual density and hierarchy are still weaker than `prototype/rtx-video-manager.html`.
  The overview now uses side-by-side HDR/VSR cards with a status rail, and the
  HDR/VSR pages have matching right-side information cards.
- The requested final spacing pass is implemented: the overview uses a two-line
  `RTX VIDEO` / `SUPER RESOLUTION` title, both feature legends share one baseline, the
  HDR spectrum shows only the driver-derived `peak_min` / `peak_max` endpoints, the HDR
  page shows only its own shortcut, both compact information cards no longer clip their
  lower text, and each hero carries the prototype's low-opacity clipped calibration arc.
- The runnable dual-function prototype is `prototype/rtx-video-manager.html`. It has
  been rendered and visually reviewed at 100% and 125% preview scales.
- GitHub Actions run `40357825705` is green for commit `9d8ebc2`; strict
  `fmt --check`, Clippy, all tests, both Release builds, packaging, and upload passed.
- Final artifact ZIP SHA-256 is
  `A4453C2B574B9B115412AFC715C06B52AC07319E0E32ABCA3449C554B0CF95CA`;
  host SHA-256 is
  `5B73016B60A68E1537B3B86C79760E0103AD3B54F1B3BF79DC44E0E2AF23B54D`
  (656,384 bytes); console SHA-256 is
  `4326D3CAFA73783A13750CA6EDC92FF61899354B4F8FF5DF92445BB819E7AE79`
  (17,763,840 bytes).
- The final host launched in `--background`, updated the preserved `RtxHdrSwitch`
  startup value to the new executable path, and reported 5.37 MiB private memory.
  The final console connected over protocol `4` and rendered HDR/VSR state correctly.
- The HDR tuning card now fills the page and keeps its rounded lower edge above the
  footer. Its real-machine screenshot was reviewed after the layout change.
- The HTML prototype navigation bug was fixed by adding `#page-hdr[hidden]`; all five
  pages were verified not to be covered by the HDR page.
- GitHub Actions run `40365737946` is green for commit `f25481b`; strict formatting,
  Clippy, all tests, both Release builds, packaging, and upload passed.
- Visual artifact ZIP SHA-256 is
  `8887B47C81FA66E633FD40B1CC569B106CEA0CC3EEF893637BB8DFC79CC397A8`;
  host SHA-256 is
  `433825645BA5F5F8533FD9D13AC6968FD456182C4531B7A6DAD5FF1FEDC37DFD`
  (656,384 bytes); console SHA-256 is
  `4832E37BEF218B2F7C0E97C7A2FE69847524DAF0E72AC4F4B211C547552AF6B9`
  (17,789,952 bytes).
- A DPI-aware, off-screen background capture of the final overview at 125% scaling
  confirmed that both cards fit, the HDR spectrum and VSR ladder are visible, the
  summary rail is not clipped, and the footer remains fixed. The checked-in preview is
  `prototype/gpui-overview-125.png`.
- GitHub Actions run `40466747582` is green for commit `397e674`; strict formatting,
  Clippy, all tests, both Release builds, packaging, and upload passed.
- Final artifact ZIP SHA-256 is
  `113AD08EFA5299E54AF47CB9C7793479036E6DEAF681167FE657929D40A0572E`;
  host SHA-256 is `E0BA85B3B897CEC48498B924A1F5B1A29B2E5690B7588CB38D5D6244BBE1A091`
  (656,384 bytes); console SHA-256 is
  `9A08D4B1C4D3B59513D8C8634106C4D759F699449D70ABE89E13823D03A06C08`
  (17,798,144 bytes).
- Off-screen 125% captures of the final artifact confirmed: the VSR title is split as
  `RTX VIDEO` / `SUPER RESOLUTION`; overview legends align; HDR labels show the
  driver-derived `400` and `1440` endpoints; HDR shows only `Ctrl+Alt+H`; VSR's right
  card has no clipped note; the quality panel fills the remaining height; and
  diagnostics stays inside its rounded panel. Checked-in previews are
  `prototype/gpui-overview-125.png`, `prototype/gpui-hdr-125.png`, and
  `prototype/gpui-vsr-125.png`.
- The temporary verification host updated the preserved `RtxHdrSwitch` startup value;
  it was restored to the pre-test installed executable path after capture.
- The new host idled at 5.50 MiB private memory. The off-screen GPUI console used
  approximately 133 MiB, above the 120 MiB target; reducing it is a follow-up
  optimization rather than a blocker for visual acceptance.

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

- Capture and review the final HDR/VSR detail pages through the same off-screen
  background method at 125%, then repeat overview/detail spot checks at 100% and 150%.
- Keep the GPUI console hidden or off-screen during acceptance. Do not activate a
  foreground review window while the user is working.
- Download the green artifact and verify the renamed host and console launch, the
  old HKCU settings remain readable, and only one `RtxHdrSwitch` startup value exists.
- On the final artifact, verify `Ctrl+Alt+H`, `Ctrl+Alt+V`, and `Ctrl+Alt+G`, both
  tray toggles, and console open/close behavior. Confirm a second console hotkey
  press hides the active console.
- In Chrome, verify VSR `status` changes while playback starts and stops, HDR and VSR
  toggle independently, and the four HDR tuning fields remain unchanged by VSR writes.
- Repeat the documented reversible VSR probe (`4 -> 3 -> 4`) only against the final
  artifact path used for acceptance; do not substitute or modify the installed NVIDIA
  DLL.
- Verify read-only mode through automated compatibility tests and injected UI state.
  A real unknown signed DLL is not available locally and must not be fabricated by
  replacing NVIDIA files.
- Inspect the taskbar, window, tray on/off/error states, and file icon after building.
  Confirm the product renders as `RTX Video Manager` and uses the new video-frame icon.
- Verify the updated 860 x 620 console at 100%, 125%, and 150% DPI, including overview,
  HDR, VSR, settings, and diagnostics; confirm all quality and tuning ranges are visible,
  read-only disables both write surfaces, and no phantom controls overlap adjacent rows.
- Verify the 500 ms overlay text for HDR on/off and VSR on/off/quality, plus the
  diagnostics copy button and both raw VSR data fields.
- Record host idle memory/CPU, console startup time, console private memory, executable
  sizes, and both SHA-256 hashes in this file after the final green run.
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

The requested HDR/VSR spacing work is complete on the 125% reference display. Continue
the remaining manual acceptance checks at 100% and 150%, then exercise the three
hotkeys, both toggles, VSR active-state changes, and the clipboard diagnostic action.
