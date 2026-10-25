# Decisions

## ADR-001: Rust with pure Win32

Use Rust, the `windows`/`windows-sys` APIs, and native Win32 controls. This keeps
the executable small and avoids the runtime and dependency weight of Tauri or
cross-platform UI crates.

Rejected: C# WinForms, Tauri, `tray-icon`, `global-hotkey`.

## ADR-002: Strict private-ABI gate

Control RTX Video HDR through NVIDIA's private `nvcpl.dll` setting `0x318`.
Permit writes only for the verified file hash and after a successful read probe.

Rejected: NVIDIA registry edits, UI automation, MessageBus reverse engineering,
Windows HDR fallback. Each is less precise or carries higher collateral risk.

## ADR-003: Cloud-only build environment

Do not install Rust or MSVC locally. GitHub Actions builds, tests, and packages the
Windows x64 executable. The local machine only runs and validates the artifact.

## ADR-004: Global switch, not per-video filtering

Toggle NVIDIA's global RTX Video HDR setting. Do not add a Chrome extension or CSS
overlay workaround; those change rendering scope and can disable VSR with HDR.

## ADR-005: STA COM and UXD service are mandatory

Initialize STA COM on the NVIDIA worker and require
`NvCplApiIsUxdServiceRunning` before `NvCplApiInit`. Without the complete session
setup, `NvCplApiSetSetting` can report success and change the stored `0x318` value
while Chrome's `hdrStatus` remains inactive.

Evidence: on 2026-10-25 the full `0x318` transaction produced `active=1` during
Chrome playback and returned to `active=0` after restoring the original state.
Rejected: treating `SetSetting`/`Execute` success alone as proof that the feature is
active, or falling back to Windows HDR.

## ADR-006: Native 300 ms toggle overlay

Show a small native Win32 popup at the bottom center of the active monitor after a
successful toggle. The window is topmost, uses a rounded custom-drawn capsule, and
uses `WS_EX_NOACTIVATE` plus `SW_SHOWNOACTIVATE` so the shortcut does not disturb
the foreground video. Hide it with a 300 ms timer.

Rejected: Windows toast notifications as the only feedback because notification
settings and Focus Assist can suppress them, and an HTML/WebView overlay because it
would add substantial runtime weight.
