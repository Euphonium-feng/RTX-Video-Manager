# Project

## Goal

Provide a lightweight Windows tray utility that controls NVIDIA RTX Video HDR and
RTX Video Super Resolution (VSR) without opening NVIDIA App. The product name is
`RTX Video Manager`.

## Requirements

- Stay resident in the notification area and use minimal idle CPU and memory.
- Default shortcuts are `Ctrl+Alt+H` for HDR, `Ctrl+Alt+V` for VSR, and
  `Ctrl+Alt+G` for the console. Each is independently recordable; the three values
  must remain unique and register or roll back as one group.
- `Ctrl+Alt+G` opens or foregrounds the console when it is not active and closes the
  console when it is already foreground.
- Toggle RTX Video HDR and RTX VSR independently. Do not toggle Windows HDR.
- VSR supports enabled/disabled plus `Auto` and quality levels `1` through `4`.
  Preserve the last non-zero quality for the next enable operation. Changing quality
  while VSR is disabled updates only the next-enable draft.
- Preserve the existing NVIDIA HDR tuning fields when toggling.
- Let the user adjust peak brightness, middle gray, contrast, and saturation through
  both sliders and direct numeric entry. Peak bounds come from the active driver
  snapshot. The reset action always restores the driver default combination.
- Show a brief bottom-center overlay on successful toggles that identifies HDR or VSR
  and reports the resulting state; include the VSR quality when enabled.
- Provide an on-demand GPUI control console with overview, HDR, VSR, settings, and
  diagnostics pages, live state, hotkey recording, startup/notification settings,
  HDR tuning, and VSR quality selection.
- Ship the host and console with a dedicated video-frame plus dual-enhancement-channel
  icon that remains legible at notification size.
- Keep the resident host lightweight by running GPUI only while the console is open.
- Permit writes only for the verified NvCpl hash. Load other NVIDIA-signed builds in
  read-only compatibility mode only when every export exists and both the `0x318`
  HDR and `0x315`/`0x317` VSR read probes parse correctly; fail closed otherwise.
- Require the NVIDIA UXD display service and an STA COM session before using the
  private NvCpl API.
- Start with Windows by default; expose a tray option to disable this.
- Run without administrator privileges or network access.

## Acceptance Criteria

- On the reference machine, toggling changes the NVIDIA App `System > Video > HDR`
  state and affects supported SDR video in Chrome within one second.
- Reopening the utility reads the current driver state rather than cached state.
- A failed or inconsistent write restores the previous 48-byte HDR state.
- A failed or inconsistent VSR write restores the previous `0x317` quality and
  commits again; the fixed fields from `0x315` remain unchanged.
- HDR and VSR can be toggled independently without changing each other's state.
- With VSR disabled, choosing a quality changes only the remembered next-enable value.
- Read-only compatibility mode presents the current HDR snapshot and diagnostics
  while disabling the HDR toggle, tuning inputs, and all NVIDIA write calls.
- Diagnostics identify the compatibility mode, actual and allowed SHA-256 values,
  DLL file version, probe result, latest raw `0x318`, raw `0x315`, and raw `0x317`.
- The generated release artifact comes from a successful GitHub Actions run.
- Idle private working set is below 15 MiB and idle CPU is effectively zero.
- Closing the console terminates the GPUI process; tray and shortcut behavior remain.
- The console title bar exposes minimize and close only; maximize is not offered.
- The HDR tuning card fills the remaining page height as one rounded surface; it must
  not leave an unwrapped strip above the footer or clip its lower corners.
- The HDR information card contains only compact status fields; full hashes and raw
  records remain in diagnostics. The VSR quality panel fills the remaining page height
  with its live active-quality strip rather than leaving a large empty region.
- The release archive contains `RtxVideoManager.exe` and
  `RtxVideoManagerConsole.exe` together.
- `RtxVideoManagerConsole.exe` displays its embedded product icon in Windows shell surfaces
  rather than the generic executable icon.

## Reference Environment

- Windows 10 Pro build 26300, reported as 26H2
- NVIDIA GeForce RTX 5060, driver 621.01
- NVIDIA App 11.0.9.565
- `nvcpl.dll` 8.17.15.5527
- Chrome 155

## Licensing

- Original source code and project-authored assets are available under
  `MIT OR Apache-2.0`; downstream users may choose either license.
- Third-party dependencies remain under their respective licenses.
- NVIDIA binaries are not redistributed.

## Out of Scope

- Chrome extensions, CSS video filtering, or content classification.
- Windows HDR switching, VSR GPU priority, a playback-status indicator, automatic
  game detection, auto-update, installers, or public support.
- Replacing the native notification-area icon or 500 ms toggle overlay with GPUI.
- Automatically trusting or enabling writes for an unknown NvCpl build.
