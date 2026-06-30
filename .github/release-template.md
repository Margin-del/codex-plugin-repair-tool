# Codex Plugin Repair Tool ${{ github.ref_name }}

## 更新内容
- 自动构建：GitHub Actions 自动编译 Windows x64 二进制
- 本发布版对应 commit: ${{ github.sha }}

## 使用方式
1. 下载 `codex_plugin_repair_tool.exe`
2. 右键 → **以管理员身份运行**
3. 点击「诊断」检查插件状态
4. 点击「修复」修复问题
5. 重启 Codex Desktop

## 技术栈
- Rust + egui/eframe (GUI)
- Win32 FFI 直接读取 WindowsApps
- 零 PowerShell 依赖
