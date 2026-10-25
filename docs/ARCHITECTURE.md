# Architecture

## Current State

The repository is a new Rust implementation. The NVIDIA ABI and field layout were
verified with a read-only probe before implementation; no runtime code exists yet.

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
3. The worker validates the exact DLL, signature, exports, and a 48-byte read probe.
4. Toggle requests read `0x318`, change only the enabled field, write, read back,
   and restore the original bytes on failure.
5. Worker results return to the tray through posted window messages.

## Glossary

- **RTX Video HDR**: NVIDIA's conversion of supported SDR video to HDR10.
- **Windows HDR**: the operating system's HDR display output mode.
- **VSR**: RTX Video Super Resolution, independent from RTX Video HDR.
- **Enabled**: the persisted NVIDIA setting.
- **Active**: whether HDR processing is currently applied to a video.
