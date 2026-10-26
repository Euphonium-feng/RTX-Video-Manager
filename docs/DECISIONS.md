# Decisions

## ADR-001: Rust with pure Win32

Use Rust, the `windows`/`windows-sys` APIs, and native Win32 controls. This keeps
the executable small and avoids the runtime and dependency weight of Tauri or
cross-platform UI crates.

Rejected: C# WinForms, Tauri, `tray-icon`, `global-hotkey`.

Superseded for application pages by ADR-007. The resident host, tray, shortcut, and
overlay remain native.

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

## ADR-006: Native 500 ms toggle overlay

Show a small native Win32 popup at the bottom center of the active monitor after a
successful HDR or VSR toggle. The window is topmost, uses a rounded custom-drawn capsule, and
uses `WS_EX_NOACTIVATE` plus `SW_SHOWNOACTIVATE` so the shortcut does not disturb
the foreground video. Keep it compact and use 50% layered-window opacity so it does
not cover the video. Hide it with a 500 ms timer.

Rejected: Windows toast notifications as the only feedback because notification
settings and Focus Assist can suppress them, and an HTML/WebView overlay because it
would add substantial runtime weight.

## ADR-007: On-demand GPUI console with a native host

Keep `RtxVideoManager.exe` as a GPUI-free Win32 process and run
`RtxVideoManagerConsole.exe`
only while the control console is open. The console communicates with the host over
a one-client, same-user named pipe; it never loads NvCpl or writes registry values.
Tray, hotkey registration, and the 500 ms overlay stay native because GPUI has no
tray API and spawning a renderer for every shortcut would add latency and memory.
All major console surfaces share the same rounded panel treatment. On the HDR page,
the tuning panel flexes into the remaining height and keeps a bottom status line, so
its rounded lower corners stay visible above the footer instead of being clipped.

Rejected: putting GPUI in the resident host, implementing a WebView/Tauri console,
or replacing native system chrome with per-toggle GPUI windows.

## ADR-008: Pin GPUI Kit 0.8.0

Depend on `gpui-kit = "=0.8.0"` and its exact `gpui-pre 0.3.9` snapshot. Do not
mix in official `gpui 0.2.2` or enable `gpui-fast`. The prototype is HTML/CSS only
for design approval; production remains native GPUI.

Rejected: independent `gpui-component` versioning, a WebView production UI, and
unpinned snapshot updates.

## ADR-009: One thread owns all IPC pipe I/O

Both sides assign every operation on a named-pipe handle to one I/O thread. The host
and GPUI threads only enqueue messages; they never call `ReadFile`/`WriteFile` on the
connection handle. This avoids blocking a synchronous pipe that already has a pending
operation and keeps tray, hotkey, HDR events, and the GUI handshake responsive.

Rejected: writing from the host UI thread while the IPC thread blocks in `ReadFile`.

## ADR-010: Embed a calibration-ring product icon

Originally used a graphite rounded tile, broken ice-blue calibration ring, white `H`,
and amber marker. The dual-feature identity in ADR-016 supersedes this artwork; the
embedding and multi-resolution ICO strategy remains.

Rejected: shipping an external icon file, relying on a generic executable icon, or
using the small tray-state glyphs as the full-size application identity. The dedicated
icon stays legible from 16 to 256 pixels and does not add a runtime asset dependency.

## ADR-011: Treat HDR tuning as a full rollback transaction

Expose peak brightness, middle gray, contrast, and saturation through the console.
Peak is bounded by the live `peak_min`/`peak_max` values; tone controls use the
validated integer ranges `0..=100` and `-100..=100`. The write preserves enable state,
display limits, and untouched bytes, commits through the existing NVIDIA execute call,
and verifies every desired tuning field before accepting success.

Rejected: writing only the slider field, trusting `SetSetting` success, or exposing
unbounded raw integers directly from the UI.

## ADR-012: Driver-derived defaults for the reset action

The reset action always produces the default tuning draft: peak equals the current
display's `peak_max`, middle gray is 50, contrast is 0, and saturation is 0. It remains
available after a successful apply and only changes the draft until the user presses
`应用调校`.

Rejected: restoring the last read snapshot under the label "reset", or disabling reset
after apply.

## ADR-013: Separate console shortcut and custom title-bar controls

Register `Ctrl+Alt+G` as a configurable global shortcut that opens or
foregrounds the console. When the console is already active, the same shortcut closes
it to the tray. The GPUI window decides using `Window::is_window_active`, so the host
does not need to guess which process owns the foreground. Register HDR, VSR, and
console shortcuts together and roll all three back if any fails. Render a custom title bar with minimize
and close only because the GPUI window-controls helper always advertises maximize on
Windows even for non-resizable windows.

## ADR-014: Read-only compatibility for unknown NVIDIA builds

Keep the exact NvCpl hash as the only writable identity. If the file is at the
canonical NVIDIA App path, passes Authenticode with an `NVIDIA Corporation` signer
organization, exposes every required export, and returns a valid 48-byte `0x318`
snapshot, allow the UI to load in read-only compatibility mode. Never call
`NvCplApiSetSetting` or `NvCplApiExecute` in that mode.

Rejected: treating any valid Authenticode signature as a writable build, enabling
experimental writes behind a toggle, maintaining an online trust list, or disabling
the console entirely when the hash is unknown. Signature, signer, export, UXD, or
read-probe failures still fail closed because diagnostic-only operation is not safe
if the private API shape has not been demonstrated.

Authenticode signer extraction queries the variable-size `CERT_INFO` buffer first,
then allocates an aligned `u64` buffer using the size returned by
`CryptMsgGetParam(CMSG_SIGNER_CERT_INFO_PARAM)`. Passing only
`size_of::<CERT_INFO>()` validates in unit tests but fails on the real NVIDIA
signature because issuer and serial payloads follow the fixed structure.

## ADR-015: Independent VSR control with fixed-field verification

Expose RTX VSR as an independent switch and quality selector backed by NVIDIA info
setting `0x315` and value setting `0x317`. `0x315` is 20 bytes: support at `+0x00`,
quality at `+0x04`, status at `+0x08`, and eight uninterpreted trailing bytes.
`0x317` is the four-byte quality value used for writes.

Encode `Off=0`, `Level1..Level4=1..4`, and `Auto=5`. A write changes only `0x317`,
commits with `NvCplApiExecute(0x10000, 0, -1)`, verifies the quality and support
fields, and restores plus recommits the previous quality on failure. Never compare or
preserve the volatile trailing bytes as a write condition or call setting `0x31b`.

Rejected: deriving VSR quality from undocumented `0x31b` data, writing a whole
20-byte info block, treating `SetSetting` success as proof of activation, or coupling
VSR writes to HDR tuning.

Evidence: on 2026-10-26 a reversible probe changed `0x317` from `4` to `3`, committed,
read `0x315` quality `3`, restored `0x317` to `4`, committed, and read quality `4`
again with no HDR field change.

## ADR-016: Dual-channel product identity and compatibility identifiers

Rename the outward product, binaries, package names, manifest identity, ZIP, window
titles, documentation, and icon to `RTX Video Manager`. Use
`RtxVideoManager.exe` and `RtxVideoManagerConsole.exe`. The new icon is a video frame
with cyan HDR and amber VSR enhancement channels, plus a calibration marker; keep the
SVG master and generate on/off/error ICO variants.

Keep the old registry key `Software\RTXHDRSwitch`, startup value name `RtxHdrSwitch`,
single-instance mutex, window classes, and named-pipe family as compatibility
identifiers. Renaming them would orphan existing settings and could create a second
startup entry during upgrades.

Rejected: changing internal identifiers together with the display name, keeping the
old single-purpose `H` icon, or shipping only the console icon without tray-state
variants.
