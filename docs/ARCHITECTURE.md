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
- `src/ipc.rs`: versioned IPC DTOs and length-prefixed JSON framing.
- `crates/ui/`: `RtxHdrSettings.exe`, the GPUI console; it never links NvCpl. Its
  IPC worker also owns both directions of its pipe handle on one thread.
- `assets/`: tray state icons, the console product icon, and the shared application
  manifest. `crates/ui/build.rs` embeds the console icon and manifest into
  `RtxHdrSettings.exe`; the console does not depend on external asset files at runtime.
- `.github/workflows/build.yml`: formatting, lint, tests, Release build, packaging,
  and artifact upload.

## Runtime Flow

1. A named mutex enforces one instance.
2. The host starts a one-client named-pipe server for the optional GPUI console.
3. A dedicated worker thread owns the NvCpl session so NVIDIA calls never block the
   window message loop.
4. Before loading NvCpl, the worker initializes an STA COM apartment and verifies
   that the NVIDIA UXD display service is running.
5. The worker validates the exact DLL, signature, exports, and a 48-byte read probe.
6. Toggle requests read `0x318`, change only the enabled field, write, commit with
   `NvCplApiExecute(0x10000, 0, -1)`, read back, and restore the original bytes on
   failure.
7. Worker results return to the tray through posted window messages and are also
   published as snapshots to a connected console.
8. Interactive launches spawn the console; `--background` launches only the
   tray so the current-user startup entry stays silent.
9. Successful toggles show a non-activating, topmost overlay at the bottom center
   of the active monitor for 500 ms at 50% opacity.
10. Closing the console exits `RtxHdrSettings.exe`; the host, tray, hotkey, and last
    HDR state remain active.

## Product Icon

`assets/rtx-hdr-console.svg` is the editable master. The checked-in PNG exports are
documentation previews, while `assets/rtx-hdr-console.ico` carries sizes from 16 to
256 pixels for Windows. The GPUI package compiles the ICO and shared manifest through
`crates/ui/resources.rc`, so packaging only needs to ship the two executables.

## Glossary

- **RTX Video HDR**: NVIDIA's conversion of supported SDR video to HDR10.
- **Windows HDR**: the operating system's HDR display output mode.
- **VSR**: RTX Video Super Resolution, independent from RTX Video HDR.
- **Enabled**: the persisted NVIDIA setting.
- **Active**: whether HDR processing is currently applied to a video.
