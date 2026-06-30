# Codex Plugin Repair Tool

[![Build & Release](https://github.com/Margin-del/codex-plugin-repair-tool/actions/workflows/release.yml/badge.svg)](https://github.com/Margin-del/codex-plugin-repair-tool/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
![Windows](https://img.shields.io/badge/Platform-Windows-blue)

Diagnose and fix **Codex Desktop bundled plugin corruption** caused by Windows file locks (`EBUSY`) during updates.  
Pure Rust GUI, single ~6 MB executable, **zero runtime dependencies**.

---

## Problem

After Codex Desktop updates on Windows:
- `Computer Use` / `Chrome` plugins disappear or show **unavailable**
- Plugin page errors: `marketplace.json does not exist`
- Logs contain `EBUSY`, `resource busy or locked`, `os error 5`

This happens because Windows file locks prevent the bundled marketplace files from being fully written during update.

---

## Features

- **Diagnose mode** — checks 7 components: marketplace.json, plugin cache, extension-host.exe, running processes, extension manifest, config.toml, and WindowsApps source
- **Repair mode** — kills locked processes, creates backups, rebuilds corrupted marketplace from WindowsApps, fixes cache junctions (`latest` symlinks)
- **Win32 FFI backdoor** — reads protected `WindowsApps` directory using `SE_BACKUP_NAME` privilege, bypasses ACL without PowerShell
- **CJK font support** — 13-font fallback chain for Chinese, Japanese, Korean display
- **UAC admin manifest** — auto-requests elevation for junction creation and process management
- **Zero PowerShell dependency** — pure Rust implementation

---

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Language | **Rust 1.70+** |
| GUI Framework | **egui 0.31** / **eframe 0.31** (immediate mode) |
| System Info | **sysinfo 0.33** |
| Junction/Symlink | **junction 1.4** |
| Serialization | **serde 1 + serde_json 1** |
| Timestamps | **chrono 0.4** |
| Windows Resource | **winresource 0.1** (build-time only) |
| Win32 FFI | Raw `extern "system"` kernel32 calls |
| Deploy Target | **Windows x86_64**, single `.exe` (~6 MB) |

---

## Download

Pre-built binaries are available from [Releases](https://github.com/Margin-del/codex-plugin-repair-tool/releases).

1. Download `codex_plugin_repair_tool.exe` from the latest release
2. Run as **Administrator** (UAC prompt appears automatically)
3. Click **诊断** (Diagnose) to check the current state
4. Click **修复** (Repair) to fix issues found
5. Restart Codex Desktop

---

## Build from Source

### Prerequisites
- [Rust](https://rustup.rs/) 1.70 or later (install via `rustup`)
- Windows SDK (for `winresource`)

### Steps
```bash
git clone https://github.com/Margin-del/codex-plugin-repair-tool.git
cd codex-plugin-repair-tool
cargo build --release
```

The binary will be at `target/release/codex_repair_gui.exe`.

---

## GitHub Actions

The repository includes a [GitHub Actions workflow](.github/workflows/release.yml) that:
- Builds the release binary on every push and pull request
- On tag push (`v*`), creates a **GitHub Release** with the `.exe` attached
- Uses `actions/upload-artifact` for development builds

### Trigger a Release
```bash
git tag v1.0.0
git push origin v1.0.0
```

---

## License

MIT
