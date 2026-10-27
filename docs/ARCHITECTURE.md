# Architecture

## Current State

The repository contains a lightweight Rust/Win32 resident host, an on-demand GPUI
control console, GitHub Actions packaging, and unit tests. Its product name is
`RTX Video Manager`. The native host owns all system integration and NVIDIA HDR/VSR
access; the GPUI process is a transient, unprivileged view over a local IPC contract.

## Target Components

- `src/lib.rs`: pure domain types, settings validation, HDR and VSR decoding/encoding,
  write/readback/rollback transactions, and unit-testable transport abstractions.
- `src/main.rs`: Win32 application entry, hidden message window, tray icon, global
  hotkey, native overlay, registry integration, NVIDIA worker, and console lifecycle.
- `src/host_ipc.rs`: local named-pipe server, same-user/session validation, command
  queue, and snapshot/event writer. A single connection thread owns every pipe read
  and write; the host main thread only enqueues outbound events.
- `src/ipc.rs`: protocol version `4` DTOs and length-prefixed JSON framing,
  including HDR/VSR state, five page identities, unified state-change events,
  compatibility metadata, and raw `0x318`, `0x315`, and `0x317` data.
- `crates/ui/`: `RtxVideoManagerConsole.exe`, the GPUI console; it never links NvCpl. Its
  IPC worker also owns both directions of its pipe handle on one thread.
- `assets/`: tray state icons, the dual-channel product icon, and the host application
  manifest. `crates/ui/build.rs` embeds the console icon into `RtxVideoManagerConsole.exe`;
  the console does not depend on external asset files at runtime.
- `.github/workflows/build.yml`: formatting, lint, tests, Release build, packaging,
  and artifact upload.

## Runtime Flow

1. A named mutex enforces one instance.
2. The host starts a one-client named-pipe server for the optional GPUI console.
3. A dedicated worker thread owns the NvCpl session so NVIDIA calls never block the
   window message loop.
4. Before loading NvCpl, the worker initializes an STA COM apartment and verifies
   that the NVIDIA UXD display service is running.
5. The worker hashes the exact DLL, verifies Authenticode and an NVIDIA
   `NVIDIA Corporation` signer organization, checks exports, creates the UXD COM
   session, and performs a 48-byte `0x318` HDR read probe plus the `0x315`/`0x317`
   VSR reads.
6. An exact hash match becomes `WritableVerified`. A different hash becomes
   `ReadOnlyUnknownBuild` only after signature, signer, export, UXD, and read-probe
   success; any prerequisite failure closes the backend.
7. HDR toggle requests read `0x318`, change only the enabled field, write, commit with
   `NvCplApiExecute(0x10000, 0, -1)`, read back, and restore the original bytes on
   failure. Tuning requests use the same transaction for peak, middle gray, contrast,
   and saturation, then verify all four readback fields plus unchanged enable state
   and display limits.
8. VSR toggle and quality requests read both `0x315` and `0x317`. They write only the
   4-byte `0x317` quality, commit with `NvCplApiExecute`, verify quality plus fixed
   support fields, and restore plus recommit the previous value on failure. Disabled
   VSR is quality `0`; re-enabling restores the most recent non-zero quality.
9. Both backend write methods reject every write before calling NvCpl when the
   compatibility mode is read-only. The host also rejects tray, hotkey, IPC, and
   legacy-window HDR/VSR write intents before queueing them.
10. Worker results return to the tray through posted window messages and are also
    published as complete unified snapshots to a connected console.
11. Interactive launches spawn the console; `--background` launches only the
   tray so the current-user startup entry stays silent.
12. Successful toggles show a non-activating, topmost overlay at the bottom center
    of the active monitor for 500 ms at 50% opacity. The text identifies HDR or VSR
    and includes the VSR quality when relevant.
13. `Ctrl+Alt+H` toggles HDR, `Ctrl+Alt+V` toggles VSR, and `Ctrl+Alt+G` sends a
    visibility toggle to the console
    when connected; an active console closes itself, while a background or absent
    console is opened and foregrounded. All three shortcuts are registered as one
    rollback-safe group.
14. Closing the console exits `RtxVideoManagerConsole.exe`; the host, tray, hotkeys,
    and last HDR/VSR states remain active.

## Compatibility Diagnostics

The host is the sole source of compatibility state. `CompatibilityInfo` carries the
mode, actual and allowed hashes, optional DLL file version, probe result, and reason.
`DiagnosticsInfo` adds the latest raw 48-byte HDR block, 20-byte VSR info block, and
4-byte VSR value as lowercase hexadecimal.
The console shows those values in its diagnostics page and can copy a stable text
report to the clipboard without contacting the host again.

The HDR page uses a full-height page container. Its hero keeps a fixed height while
the tuning card grows into the remaining space, preserves the shared 14 px corner
radius and border, and ends above the footer instead of leaving an unwrapped strip.

The overview mirrors the approved prototype: HDR and VSR hero cards share one
horizontal row, each with live readouts and a compact switch, followed by a three-cell
status rail for backend compatibility, current video signals, and all three hotkeys.
The HDR and VSR detail pages pair their hero with a narrow right-side information card.
The HDR information card shows only compatibility mode, DLL version, and probe result;
long hashes and raw records stay in diagnostics. The VSR quality panel grows into the
remaining page height and uses a three-part quality guide plus a live `0x315` status
strip, so the page does not end above a large unused region. The HDR detail page shows
only its own hotkey. VSR titles use an explicit `RTX VIDEO` / `SUPER RESOLUTION` break,
and the overview reserves the same title height on both cards so their two legends
share one horizontal baseline.

In `ReadOnlyUnknownBuild`, HDR/VSR toggles, all four HDR tuning controls,
`应用调校`, and VSR quality writes are disabled. The reset action may still update
the local draft, and the console may refresh/read diagnostics. A hotkey attempt shows
a notification and never queues a worker write.

## Product Icon

`assets/rtx-video-manager.svg` is the editable master. The checked-in PNG exports are
documentation previews, while `assets/rtx-video-manager.ico` carries sizes from 16 to
256 pixels for Windows. Separate on/off/error ICO variants carry tray state. The GPUI package compiles the ICO through
`crates/ui/resources.rc`, so packaging only needs to ship the two executables. The
console intentionally does not embed the host manifest because the GPUI dependency
chain already links its per-monitor DPI manifest; embedding both caused a duplicate
manifest resource at link time.

## Glossary

- **RTX Video HDR**: NVIDIA's conversion of supported SDR video to HDR10.
- **Windows HDR**: the operating system's HDR display output mode.
- **VSR**: RTX Video Super Resolution, independent from RTX Video HDR. Its persistent
  quality uses `0`, `1` through `4`, and `5` for `Auto`.
- **Enabled**: the persisted NVIDIA setting for one feature.
- **Active**: whether HDR or VSR processing is currently applied to a video.
- **Active quality**: the driver-reported VSR quality while a compatible video is
  playing; it can differ from the persistent next-enable quality.
