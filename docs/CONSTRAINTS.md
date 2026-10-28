# Constraints

- Use Rust. The resident host UI surface remains pure Win32; the on-demand console
  uses GPUI Kit. Do not add Tauri, WebView, `tray-icon`, or `global-hotkey`.
- The host must not link GPUI, own a renderer, or keep the console process alive
  after its only window closes.
- Tray icon, native menu, global shortcut registration, and the 500 ms overlay stay
  in the Win32 host. GPUI has no supported tray API.
- The GPUI console must not load NvCpl, write NVIDIA settings, or persist settings
  directly. All actions go through the versioned local IPC channel to the host.
- Do not install Rust or MSVC on this machine. Builds and automated tests run on
  GitHub Actions `windows-latest`; only the Actions result counts as verified.
- The process runs as the current user and must not require elevation.
- NVIDIA control is limited to the private `nvcpl.dll` exports. No registry writes
  to NVIDIA driver settings and no UI automation are permitted.
- Only `%ProgramW6432%\NVIDIA Corporation\NVIDIA App\NvCpl\nvcpl.dll` is eligible.
  Its SHA-256 must be
  `EEE812AB8FF74975D3CC7FE77130F5766911B6CA2D9C42C1156376A6C13F79F0`.
- An unknown NVIDIA hash is eligible only when Authenticode verifies, the signer
  organization is `NVIDIA Corporation`, every required export exists, and the
  48-byte `0x318` read probe parses correctly. That combination is read-only.
- An invalid signature or signer, missing export, failed read probe, or malformed
  HDR data fails closed and does not enter read-only compatibility mode.
- Read-only compatibility must never call `NvCplApiSetSetting`,
  `NvCplApiExecute`, or persist a proposed setting. The UI disables toggling and
  tuning, while status, diagnostics, copy-diagnostics, and reset-to-draft remain
  available.
- Unknown builds cannot become writable through a runtime override, online
  compatibility list, telemetry, or experimental write mode.
- The NVIDIA worker must initialize an STA COM apartment and confirm
  `NvCplApiIsUxdServiceRunning` before `NvCplApiInit`; writes are refused if either
  prerequisite fails.
- Settings and startup entries may only be written under `HKCU`.
- HDR tuning writes must stay inside the current driver limits: peak brightness is
  clamped to `peak_min..=peak_max`; middle gray is limited to `0..=100`; contrast and
  saturation are limited to `-100..=100`. A failed or mismatched write restores the
  complete original 48-byte block.
- The console reset action always targets peak upper bound, middle gray 50, contrast
  0, and saturation 0. It does not restore an arbitrary earlier user-edited snapshot.
- RTX Video HDR, Windows HDR, and VSR are distinct settings and must not be
  treated interchangeably.
- GPUI must be pinned to `gpui-kit = "=0.8.0"` and its pinned GPUI snapshot; do not
  mix in official `gpui 0.2.2` or experimental `gpui-fast`.
- Host/console IPC protocol version `3` is required for compatibility diagnostics.
  Mixed protocol versions must instruct the user to re-extract both executables.
