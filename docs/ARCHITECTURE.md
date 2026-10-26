# Architecture

## Current State

The repository contains a lightweight Rust/Win32 resident host, an on-demand GPUI
control console, GitHub Actions packaging, and unit tests. The native host owns all
system integration and NVIDIA access; the GPUI process is a transient, unprivileged
view over a local IPC contract.

## Target Components

- `src/lib.rs`: pure domain types, settings validation, HDR decoding and encoding,
  write/readback/rollback transaction, and unit-testable transport abstraction.
- `src/main.rs`: Win32 application entry, hidden message window, tray icon, global
  hotkey, native overlay, registry integration, NVIDIA worker, and console lifecycle.
- `src/host_ipc.rs`: local named-pipe server, same-user/session validation, command
  queue, and snapshot/event writer. A single connection thread owns every pipe read
  and write; the host main thread only enqueues outbound events.
- `src/ipc.rs`: protocol version `3` DTOs and length-prefixed JSON framing,
  including compatibility metadata and the latest raw `0x318` block.
- `crates/ui/`: `RtxHdrSettings.exe`, the GPUI console; it never links NvCpl. Its
  IPC worker also owns both directions of its pipe handle on one thread.
- `assets/`: tray state icons, the console product icon, and the host application
  manifest. `crates/ui/build.rs` embeds the console icon into `RtxHdrSettings.exe`;
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
   session, and performs a 48-byte `0x318` read probe.
6. An exact hash match becomes `WritableVerified`. A different hash becomes
   `ReadOnlyUnknownBuild` only after signature, signer, export, UXD, and read-probe
   success; any prerequisite failure closes the backend.
7. Toggle requests read `0x318`, change only the enabled field, write, commit with
   `NvCplApiExecute(0x10000, 0, -1)`, read back, and restore the original bytes on
   failure. Tuning requests use the same transaction for peak, middle gray, contrast,
   and saturation, then verify all four readback fields plus unchanged enable state
   and display limits.
8. `NvBackend::write_hdr_bytes` rejects every write before calling NvCpl when the
   compatibility mode is read-only. The host also rejects tray, hotkey, IPC, and
   legacy-window write intents before queueing them.
9. Worker results return to the tray through posted window messages and are also
   published as snapshots to a connected console.
10. Interactive launches spawn the console; `--background` launches only the
   tray so the current-user startup entry stays silent.
11. Successful toggles show a non-activating, topmost overlay at the bottom center
   of the active monitor for 500 ms at 50% opacity.
12. `Ctrl+Alt+H` toggles HDR. `Ctrl+Alt+G` sends a visibility toggle to the console
    when connected; an active console closes itself, while a background or absent
    console is opened and foregrounded. Both shortcuts are registered as one
    rollback-safe group.
13. Closing the console exits `RtxHdrSettings.exe`; the host, tray, hotkeys, and last
    HDR state remain active.

## Compatibility Diagnostics

The host is the sole source of compatibility state. `CompatibilityInfo` carries the
mode, actual and allowed hashes, optional DLL file version, probe result, and reason.
`DiagnosticsInfo` adds the latest raw 48-byte HDR block as lowercase hexadecimal.
The console shows those values in its diagnostics page and can copy a stable text
report to the clipboard without contacting the host again.

In `ReadOnlyUnknownBuild`, all four tuning controls, `应用调校`, the hero HDR button,
and the tray toggle are disabled. The reset action may still update the local draft,
and the console may refresh/read diagnostics. A hotkey attempt shows a notification
and never queues a worker write.

## Product Icon

`assets/rtx-hdr-console.svg` is the editable master. The checked-in PNG exports are
documentation previews, while `assets/rtx-hdr-console.ico` carries sizes from 16 to
256 pixels for Windows. The GPUI package compiles the ICO through
`crates/ui/resources.rc`, so packaging only needs to ship the two executables. The
console intentionally does not embed the host manifest because the GPUI dependency
chain already links its per-monitor DPI manifest; embedding both caused a duplicate
manifest resource at link time.

## Glossary

- **RTX Video HDR**: NVIDIA's conversion of supported SDR video to HDR10.
- **Windows HDR**: the operating system's HDR display output mode.
- **VSR**: RTX Video Super Resolution, independent from RTX Video HDR.
- **Enabled**: the persisted NVIDIA setting.
- **Active**: whether HDR processing is currently applied to a video.
