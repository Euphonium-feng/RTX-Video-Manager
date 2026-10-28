# Project

## Goal

Provide a lightweight Windows tray utility with a configurable global shortcut that
turns NVIDIA RTX Video HDR on or off without opening NVIDIA App.

## Requirements

- Stay resident in the notification area and use minimal idle CPU and memory.
- Default shortcut: `Ctrl+Alt+H`; allow the user to record a different shortcut.
- Default `Ctrl+Alt+G` opens or foregrounds the console when it is not active and
  closes the console when it is already foreground. Allow it to be recorded
  independently and reject duplicate shortcut assignments.
- Toggle only RTX Video HDR. Do not toggle Windows HDR or RTX Video Super Resolution.
- Preserve the existing NVIDIA HDR tuning fields when toggling.
- Let the user adjust peak brightness, middle gray, contrast, and saturation through
  both sliders and direct numeric entry. Peak bounds come from the active driver
  snapshot. The reset action always restores the driver default combination.
- Show a brief bottom-center overlay on successful toggles that reports whether
  RTX Video HDR is on or off.
- Provide an on-demand GPUI control console with a direct HDR action, live state,
  hotkey recording, startup/notification settings, and diagnostics.
- Ship the control console with a dedicated product icon that follows the console's
  calibration-dashboard visual language and remains legible at notification-size.
- Keep the resident host lightweight by running GPUI only while the console is open.
- Permit writes only for the verified NvCpl hash. Load other NVIDIA-signed
  builds with complete exports and a valid 48-byte read probe in read-only
  compatibility mode; fail closed on signature, export, or read-probe failure.
- Require the NVIDIA UXD display service and an STA COM session before using the
  private NvCpl API.
- Start with Windows by default; expose a tray option to disable this.
- Run without administrator privileges or network access.

## Acceptance Criteria

- On the reference machine, toggling changes the NVIDIA App `System > Video > HDR`
  state and affects supported SDR video in Chrome within one second.
- Reopening the utility reads the current driver state rather than cached state.
- A failed or inconsistent write restores the previous 48-byte HDR state.
- Read-only compatibility mode presents the current HDR snapshot and diagnostics
  while disabling the HDR toggle, tuning inputs, and all NVIDIA write calls.
- Diagnostics identify the compatibility mode, actual and allowed SHA-256 values,
  DLL file version, probe result, and latest raw `0x318` bytes.
- The generated release artifact comes from a successful GitHub Actions run.
- Idle private working set is below 15 MiB and idle CPU is effectively zero.
- Closing the console terminates the GPUI process; tray and shortcut behavior remain.
- The console title bar exposes minimize and close only; maximize is not offered.
- The release archive contains `RtxHdrSwitch.exe` and `RtxHdrSettings.exe` together.
- `RtxHdrSettings.exe` displays its embedded product icon in Windows shell surfaces
  rather than the generic executable icon.

## Reference Environment

- Windows 10 Pro build 26300, reported as 26H2
- NVIDIA GeForce RTX 5060, driver 621.01
- NVIDIA App 11.0.9.565
- `nvcpl.dll` 8.17.15.5527
- Chrome 155

## Out of Scope

- Chrome extensions, CSS video filtering, or content classification.
- Windows HDR switching, VSR control, auto-update, installers, or public support.
- Replacing the native notification-area icon or 500 ms toggle overlay with GPUI.
- Automatically trusting or enabling writes for an unknown NvCpl build.
