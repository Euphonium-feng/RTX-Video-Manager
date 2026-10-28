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
  organization is `NVIDIA Corporation`, every required export exists, the 48-byte
  `0x318` HDR read probe parses correctly, and the 20-byte `0x315` plus 4-byte
  `0x317` VSR reads are consistent. That combination is read-only for both features.
- An invalid signature or signer, missing export, failed read probe, or malformed
  HDR data fails closed and does not enter read-only compatibility mode.
- Read-only compatibility must never call `NvCplApiSetSetting`,
  `NvCplApiExecute`, or persist a proposed setting for either feature. The UI
  disables HDR/VSR toggles, HDR tuning, and VSR quality writes, while status,
  diagnostics, copy-diagnostics, and local reset-to-draft remain available.
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
- VSR reads the 20-byte `0x315` info block and writes the 4-byte `0x317` value.
  `0x315` fields are support at `+0x00`, quality at `+0x04`, and status at `+0x08`;
  the remaining eight bytes are uninterpreted. Never call setting `0x31b`.
- A VSR write commits with `NvCplApiExecute(0x10000, 0, -1)`, verifies quality and
  support, preserves the fixed `0x315` fields, and restores plus recommits the prior
  `0x317` value on failure.
- VSR disabled state is quality `0`. Re-enabling uses the most recent non-zero quality;
  changing quality while disabled only updates the next-enable persisted draft.
- The three global shortcuts must be non-duplicated and registered as one rollback
  group. Defaults are `Ctrl+Alt+H`, `Ctrl+Alt+V`, and `Ctrl+Alt+G`.
- Public binaries, package names, manifest identity, window titles, ZIP name, README,
  and product icon use `RTX Video Manager`. Keep the compatibility identifiers
  `Software\RTXHDRSwitch`, startup value `RtxHdrSwitch`, and the existing host mutex,
  window-class, and pipe families so upgrades do not lose settings or start twice.
- GPUI must be pinned to `gpui-kit = "=0.8.0"` and its pinned GPUI snapshot; do not
  mix in official `gpui 0.2.2` or experimental `gpui-fast`.
- Host/console IPC protocol version `4` is required for HDR/VSR snapshots, unified
  state-change events, and dual-feature diagnostics.
  Mixed protocol versions must instruct the user to re-extract both executables.
- Original source and project-authored assets are dual-licensed under
  `MIT OR Apache-2.0`. Keep both root license files and the Cargo SPDX metadata
  synchronized, preserve third-party notices, and do not imply that the licenses
  grant rights to NVIDIA binaries or trademarks.
