# Codex Plugin Repair Tool

A Rust GUI tool to diagnose and fix Codex Desktop bundled plugin corruption on Windows.

## Problem

After Codex Desktop updates on Windows, file locks (EBUSY) can cause the bundled marketplace to be only partially written, resulting in:
- Missing `marketplace.json`
- Computer Use / Chrome plugins showing "unavailable"
- Plugin page errors: "marketplace.json does not exist"

## Features

- Pure Rust, single ~5MB executable, zero dependencies at runtime
- Auto-detects all Codex paths (WindowsApps, plugin cache, config, etc.)
- GUI interface with Chinese language support
- Diagnose mode: checks all 7 components
- Repair mode: rebuilds marketplace, fixes cache junctions, creates backups
- Uses Win32 FFI to read protected WindowsApps directory

## Usage

1. Download `CodexPluginRepair.exe`
2. Run as Administrator (UAC prompt will appear)
3. Click **诊断** (Diagnose) to check the current state
4. Click **修复** (Repair) to fix any issues found
5. Restart Codex Desktop

## Build from source

```bash
cargo build --release
```

Requires: Rust 1.70+, Windows SDK (for `winresource`)

---

# Codex 插件修复工具

诊断并修复 Codex Desktop bundled 插件因 Windows 文件锁导致的损坏问题。

## 使用方法

1. 以管理员身份运行 `CodexPluginRepair.exe`
2. 点击「诊断」检查当前状态
3. 点击「修复」修复发现的问题
4. 重启 Codex Desktop

## 从源码编译

```bash
cargo build --release
```
