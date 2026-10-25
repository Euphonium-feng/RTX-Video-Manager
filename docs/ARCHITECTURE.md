# Architecture

## Current State

The repository contains the implemented Rust/Win32 tray application, GitHub Actions
build, and unit tests. Local direct probing confirmed the current NVIDIA App's HDR
setting at `0x318`; the final cloud artifact still needs end-to-end revalidation.

## Target Components

- `src/lib.rs`: pure domain types, settings validation, HDR decoding and encoding,
  write/readback/rollback transaction, and unit-testable transport abstraction.
- `src/main.rs`: Win32 application entry, hidden message window, tray icon, global
  hotkey, native settings window, registry integration, and NVIDIA worker.
- `assets/`: icon and manifest resources compiled into the executable.
- `.github/workflows/build.yml`: formatting, lint, tests, Release build, packaging,
  and artifact upload.

## Runtime Flow

1. A named mutex enforces one instance.
2. A dedicated worker thread owns the NvCpl session so NVIDIA calls never block the
   window message loop.
3. Before loading NvCpl, the worker initializes an STA COM apartment and verifies
   that the NVIDIA UXD display service is running.
4. The worker validates the exact DLL, signature, exports, and a 48-byte read probe.
5. Toggle requests read `0x318`, change only the enabled field, write, commit with
   `NvCplApiExecute(0x10000, 0, -1)`, read back, and restore the original bytes on
   failure.
6. Worker results return to the tray through posted window messages.
7. Interactive launches show the settings window; `--background` launches only the
   tray so the current-user startup entry stays silent.
8. Successful toggles show a non-activating, topmost overlay at the bottom center
   of the active monitor for 300 ms.

## Glossary

- **RTX Video HDR**: NVIDIA's conversion of supported SDR video to HDR10.
- **Windows HDR**: the operating system's HDR display output mode.
- **VSR**: RTX Video Super Resolution, independent from RTX Video HDR.
- **Enabled**: the persisted NVIDIA setting.
- **Active**: whether HDR processing is currently applied to a video.
