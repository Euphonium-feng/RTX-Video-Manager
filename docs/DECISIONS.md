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
