# Constraints

- Use Rust and a pure Win32 UI. Do not add Tauri, WebView, `tray-icon`, or
  `global-hotkey`.
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
