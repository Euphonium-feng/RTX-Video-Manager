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
- Unknown NVIDIA versions, failed signature checks, missing exports, or invalid
  read data make the application read-only.
- The NVIDIA worker must initialize an STA COM apartment and confirm
  `NvCplApiIsUxdServiceRunning` before `NvCplApiInit`; writes are refused if either
  prerequisite fails.
- Settings and startup entries may only be written under `HKCU`.
- RTX Video HDR, Windows HDR, and VSR are distinct settings and must not be
  treated interchangeably.
- GPUI must be pinned to `gpui-kit = "=0.8.0"` and its pinned GPUI snapshot; do not
  mix in official `gpui 0.2.2` or experimental `gpui-fast`.
