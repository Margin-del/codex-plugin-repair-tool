# Codex Plugin Repair Tool

[![Build & Release](https://github.com/Margin-del/codex-plugin-repair-tool/actions/workflows/release.yml/badge.svg)](https://github.com/Margin-del/codex-plugin-repair-tool/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
![Windows](https://img.shields.io/badge/Platform-Windows-blue)
[![GitHub stars](https://img.shields.io/github/stars/Margin-del/codex-plugin-repair-tool?style=social)](https://github.com/Margin-del/codex-plugin-repair-tool/stargazers)

诊断并修复 **Codex Desktop bundled 插件损坏** 问题，由 Windows 文件锁（`EBUSY`）在更新时导致。  
纯 Rust GUI，单文件 ~6 MB 可执行程序，**零运行时依赖**。

---

## 问题现象

Codex Desktop 更新后可能出现：
- `Computer Use` / `Chrome` 插件消失或显示 **unavailable**
- 插件页报错：`marketplace.json does not exist`
- 日志出现 `EBUSY`、`resource busy or locked`、`os error 5`

原因是 Windows 文件锁导致 bundled marketplace 文件在更新时只被部分写入。

---

## 功能

- **诊断模式** — 检查 7 个组件：marketplace.json、插件缓存、extension-host.exe、运行进程、扩展清单、config.toml、WindowsApps 源
- **修复模式** — 杀死锁进程、创建备份、从 WindowsApps 重建 marketplace、修复缓存软连接
- **Win32 FFI 特权读取** — 用 `SE_BACKUP_NAME` 权限绕过 ACL 读取 WindowsApps 目录
- **CJK 字体支持** — 13 字体回退链，兼容中/日/韩显示
- **UAC 管理员清单** — 启动时自动请求管理员权限以创建软连接和管理进程

---

## 技术栈

| 层 | 技术 |
|---|---|
| 语言 | **Rust 1.70+** |
| GUI 框架 | **egui 0.31** / **eframe 0.31**（即时模式） |
| 系统信息 | **sysinfo 0.33** |
| 软连接 | **junction 1.4** |
| 序列化 | **serde 1 + serde_json 1** |
| 时间戳 | **chrono 0.4** |
| Windows 资源 | **winresource 0.1**（仅构建时） |
| Win32 调用 | 原生 `extern "system"` kernel32 FFI |
| 部署目标 | **Windows x86_64**，单 `.exe` ~6 MB |

---

## 下载

预编译二进制见 [Releases](https://github.com/Margin-del/codex-plugin-repair-tool/releases)。

### 使用步骤

1. 从最新 Release 下载 `codex_plugin_repair_tool.exe`
2. 右键 → **以管理员身份运行**（自动弹出 UAC 确认）
3. 点击 **诊断** 检查当前状态
4. 点击 **修复** 修复发现的问题
5. 重启 Codex Desktop

---

## 从源码编译

### 前置要求
- [Rust](https://rustup.rs/) 1.70 或更新版本（通过 `rustup` 安装）
- Windows SDK（供 `winresource` 使用）

### 编译步骤
```bash
git clone https://github.com/Margin-del/codex-plugin-repair-tool.git
cd codex-plugin-repair-tool
cargo build --release
```

二进制文件位于 `target/release/codex_repair_gui.exe`。

---

## GitHub Actions 自动构建

仓库包含 [GitHub Actions 工作流](.github/workflows/release.yml)，功能如下：
- 每次推送和 PR 自动编译 Release 二进制
- 推送 `v*` 标签时自动创建 **GitHub Release** 并上传 `.exe`
- 开发构建通过 `actions/upload-artifact` 保存

### 触发 Release
```bash
git tag v1.0.0
git push origin v1.0.0
```

---

## 许可证

MIT
