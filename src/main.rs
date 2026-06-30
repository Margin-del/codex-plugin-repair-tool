// Codex Desktop Bundled 插件修复工具 - Rust GUI 版
// 纯 Rust 实现，零 PowerShell 依赖

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use chrono::Local;
use eframe::egui;
use junction;
use serde::Deserialize;
use serde_json;
use std::collections::HashMap;
use std::fs;
use std::ffi::OsStr;
use std::path::Path;
use std::sync::mpsc;
use std::thread;
use sysinfo::System;

// ─── 常量 ─────────────────────────────────────────────────────────────────────

const APP_NAME: &str = "Codex Desktop Bundled 插件修复工具";

// ─── 数据结构 ─────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Default)]
struct DetectedPaths {
    codex_home: String,
    bundled_tmp_root: String,
    marketplace_json: String,
    plugin_cache_root: String,
    cache_chrome_latest: String,
    cache_computer_use_latest: String,
    cache_browser_latest: String,
    windowsapps_source: String,
    extension_manifest: String,
    config_toml: String,
    extension_host_exe: String,
    native_hosts: String,
    backup_root: String,
}

#[derive(Clone, Debug)]
enum LogEntry {
    Plain(()),
    Title(String),
    Step(String),
    Ok(String),
    Fail(String),
    Warn(String),
    Info(String),
}

#[derive(Clone, Debug)]
struct Issue(String);

#[derive(Clone, Debug, Default, Deserialize)]
struct MarketplacePlugin {
    name: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct MarketplaceData {
    plugins: Vec<MarketplacePlugin>,
}

// ─── 工具函数 ────────────────────────────────────────────────────────────────

fn dir_exists(p: &str) -> bool {
    !p.is_empty() && Path::new(p).is_dir()
}

fn file_exists(p: &str) -> bool {
    !p.is_empty() && Path::new(p).is_file()
}

fn file_size(p: &str) -> u64 {
    fs::metadata(p).map(|m| m.len()).unwrap_or(0)
}

fn read_file(p: &str) -> String {
    fs::read_to_string(p).unwrap_or_default()
}

fn env_var(name: &str) -> String {
    std::env::var(name).unwrap_or_default()
}

fn base_name(p: &str) -> String {
    Path::new(p)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default()
}

fn get_junction_target(p: &str) -> String {
    let path = Path::new(p);
    if junction::exists(path).unwrap_or(false) {
        junction::get_target(path)
            .map(|t| t.to_string_lossy().to_string())
            .unwrap_or_default()
    } else {
        String::new()
    }
}

fn create_junction(target: &str, junc_path: &str) -> bool {
    let target_path = Path::new(target);
    let p = Path::new(junc_path);
    let _ = fs::remove_dir(p);
    junction::create(target_path, p).is_ok()
}


#[allow(dead_code)]
fn is_extension_host_running() -> bool {
    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    sys.processes_by_name(OsStr::new("extension-host")).next().is_some()
        || sys.processes_by_name(OsStr::new("codex-computer-use")).next().is_some()
}

fn kill_extension_host_processes() {
    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    let names = ["extension-host", "codex-computer-use"];
    let pids: Vec<_> = sys
        .processes()
        .iter()
        .filter(|(_, p)| {
            let name = p.name().to_string_lossy();
            names.iter().any(|n| name.contains(n) || name == *n)
        })
        .map(|(pid, _)| *pid)
        .collect();
    for pid in pids {
        if let Some(proc) = sys.process(pid) {
            let _ = proc.kill();
        }
    }
}

fn get_sorted_version_dirs(cache_dir: &str) -> Vec<String> {
    let path = Path::new(cache_dir);
    if !path.is_dir() {
        return vec![];
    }
    let mut dirs: Vec<String> = fs::read_dir(path)
        .into_iter()
        .flat_map(|rd| rd.filter_map(|e| e.ok()))
        .filter(|e| {
            e.file_type().map(|t| t.is_dir()).unwrap_or(false) && e.file_name() != "latest"
        })
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    dirs.sort_by(|a, b| b.cmp(a)); // descending
    dirs
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    if dst.exists() {
        fs::remove_dir_all(dst)?;
    }
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

// ─── 路径检测 ────────────────────────────────────────────────────────────────

/// 使用 Win32 API（FFI 直接调用）读取 WindowsApps 目录
/// 通过 SE_BACKUP_NAME 特权绕过 ACL 限制
#[allow(non_snake_case, non_camel_case_types, dead_code)]
mod win32 {
    // --- 类型定义 ---
    pub type HANDLE = isize;
    pub type BOOL = i32;
    pub type DWORD = u32;
    pub type LPCWSTR = *const u16;
    pub type LPVOID = *mut std::ffi::c_void;
    pub type LPDWORD = *mut u32;

    pub const INVALID_HANDLE_VALUE: HANDLE = -1;
    pub const TRUE: BOOL = 1;
    pub const FALSE: BOOL = 0;

    // File access constants
    pub const FILE_LIST_DIRECTORY: DWORD = 0x0001;
    pub const FILE_SHARE_READ: DWORD = 0x00000001;
    pub const FILE_SHARE_WRITE: DWORD = 0x00000002;
    pub const FILE_SHARE_DELETE: DWORD = 0x00000004;
    pub const OPEN_EXISTING: DWORD = 0x00000003;
    pub const FILE_FLAG_BACKUP_SEMANTICS: DWORD = 0x02000000;

    // Token constants
    pub const TOKEN_ADJUST_PRIVILEGES: DWORD = 0x0020;
    pub const TOKEN_QUERY: DWORD = 0x0008;
    pub const SE_PRIVILEGE_ENABLED: DWORD = 0x00000002;

    // File info class
    pub const FILE_ID_BOTH_DIRECTORY_RESTART_INFO: i32 = 8;
    pub const FILE_ID_BOTH_DIRECTORY_INFO: i32 = 9;

    #[repr(C)]
    pub struct LUID {
        pub LowPart: DWORD,
        pub HighPart: i32,
    }

    #[repr(C)]
    pub struct LUID_AND_ATTRIBUTES {
        pub Luid: LUID,
        pub Attributes: DWORD,
    }

    #[repr(C)]
    pub struct TOKEN_PRIVILEGES {
        pub PrivilegeCount: DWORD,
        pub Privileges: [LUID_AND_ATTRIBUTES; 1],
    }

    #[link(name = "kernel32")]
    extern "system" {
        pub fn GetCurrentProcess() -> HANDLE;
        pub fn OpenProcessToken(
            ProcessHandle: HANDLE,
            DesiredAccess: DWORD,
            TokenHandle: *mut HANDLE,
        ) -> BOOL;
        pub fn LookupPrivilegeValueW(
            lpSystemName: LPCWSTR,
            lpName: LPCWSTR,
            lpLuid: *mut LUID,
        ) -> BOOL;
        pub fn AdjustTokenPrivileges(
            TokenHandle: HANDLE,
            DisableAllPrivileges: BOOL,
            NewState: *const TOKEN_PRIVILEGES,
            BufferLength: DWORD,
            PreviousState: *mut TOKEN_PRIVILEGES,
            ReturnLength: *mut DWORD,
        ) -> BOOL;
        pub fn CloseHandle(hObject: HANDLE) -> BOOL;
        pub fn GetLastError() -> DWORD;
        pub fn CreateFileW(
            lpFileName: LPCWSTR,
            dwDesiredAccess: DWORD,
            dwShareMode: DWORD,
            lpSecurityAttributes: *const std::ffi::c_void,
            dwCreationDisposition: DWORD,
            dwFlagsAndAttributes: DWORD,
            hTemplateFile: HANDLE,
        ) -> HANDLE;
        pub fn GetFileInformationByHandleEx(
            hFile: HANDLE,
            FileInformationClass: i32,
            lpFileInformation: LPVOID,
            dwBufferSize: DWORD,
        ) -> BOOL;
    }
}

fn read_winapps_dir_win32(path: &str) -> Vec<String> {
    let mut results = Vec::new();

    unsafe {
        // 1. 启用 SE_BACKUP_NAME 特权
        let mut token: win32::HANDLE = 0 as isize;
        let cur_proc = win32::GetCurrentProcess();
        if win32::OpenProcessToken(cur_proc, win32::TOKEN_ADJUST_PRIVILEGES | win32::TOKEN_QUERY, &mut token) == 0 {
            // 即使特权启用失败，也尝试直接打开目录
        } else {
            let mut tp = win32::TOKEN_PRIVILEGES {
                PrivilegeCount: 1,
                Privileges: [win32::LUID_AND_ATTRIBUTES {
                    Luid: win32::LUID { LowPart: 0, HighPart: 0 },
                    Attributes: win32::SE_PRIVILEGE_ENABLED,
                }],
            };
            let mut luid = win32::LUID { LowPart: 0, HighPart: 0 };
            let name: Vec<u16> = "SeBackupPrivilege\0".encode_utf16().collect();
            win32::LookupPrivilegeValueW(std::ptr::null::<u16>(), name.as_ptr(), &mut luid);
            tp.Privileges[0].Luid = luid;
            win32::AdjustTokenPrivileges(token, 0, &tp, 0, std::ptr::null_mut(), std::ptr::null_mut());
            win32::CloseHandle(token);
        }

        // 2. CreateFileW 打开目录
        let wide_path: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        let handle = win32::CreateFileW(
            wide_path.as_ptr(),
            win32::FILE_LIST_DIRECTORY,
            win32::FILE_SHARE_READ | win32::FILE_SHARE_WRITE | win32::FILE_SHARE_DELETE,
            std::ptr::null() as *const std::ffi::c_void,
            win32::OPEN_EXISTING,
            win32::FILE_FLAG_BACKUP_SEMANTICS,
            0 as isize,
        );

        if handle == win32::INVALID_HANDLE_VALUE {
            // 记录错误码到桌面（方便调试）
            let err_code = win32::GetLastError();
            let _ = std::fs::write(
                std::env::var("USERPROFILE").unwrap_or_default() + "/Desktop/codex-repair-winapps-debug.txt",
                format!("CreateFileW failed with error code: {} (0x{:08X})\nPath: {}", err_code, err_code, path)
            );
            return results;
        }

        // 3. 枚举目录
        let mut buf = [0u8; 16384];
        let mut first = win32::TRUE;
        loop {
            let ok = win32::GetFileInformationByHandleEx(
                handle,
                if first != 0 { win32::FILE_ID_BOTH_DIRECTORY_RESTART_INFO } else { win32::FILE_ID_BOTH_DIRECTORY_INFO },
                buf.as_mut_ptr() as *mut std::ffi::c_void,
                buf.len() as u32,
            );
            if ok == 0 { break; }
            first = 0;

            let mut offset: usize = 0;
            loop {
                if offset + 60 > buf.len() { break; }
                let info = &buf[offset..];
                let name_len = u32::from_ne_bytes(info[56..60].try_into().unwrap_or([0; 4])) as usize;
                let next_entry = u32::from_ne_bytes(info[0..4].try_into().unwrap_or([0; 4])) as usize;

                if name_len > 0 && offset + 60 + name_len <= buf.len() {
                    let raw = &info[60..60 + name_len];
                    let u16s: Vec<u16> = raw.chunks(2).map(|c| u16::from_ne_bytes([c[0], c[1]])).collect();
                    if let Some(end) = u16s.iter().position(|&c| c == 0) {
                        if let Ok(name) = String::from_utf16(&u16s[..end]) {
                            if name.starts_with("OpenAI.Codex_") && !name.ends_with(".Appx") && !name.ends_with(".msix") && !name.ends_with(".eappx") {
                                let sep = if path.ends_with('\\') { "" } else { "/" };
                                results.push(format!("{}{}{}", path, sep, name));
                            }
                        }
                    }
                }
                if next_entry == 0 { break; }
                offset += next_entry;
            }
        }
        win32::CloseHandle(handle);
    }
    results
}

fn detect_paths() -> DetectedPaths {
    let userprofile = env_var("USERPROFILE");
    let localappdata = env_var("LOCALAPPDATA");
    let programfiles = env_var("ProgramFiles");

    let codex_home = format!("{}/.codex", userprofile);
    let openai_local = format!("{}/OpenAI", localappdata);
    let codex_local = format!("{}/Codex", &openai_local);

    let bundled_tmp = format!("{}/.tmp/bundled-marketplaces/openai-bundled", &codex_home);
    let mkt = format!("{}/.agents/plugins/marketplace.json", &bundled_tmp);
    let plugin_cache = format!("{}/plugins/cache/openai-bundled", &codex_home);

    let cache_chrome = format!("{}/chrome/latest", &plugin_cache);
    let cache_cu = format!("{}/computer-use/latest", &plugin_cache);
    let cache_br = format!("{}/browser/latest", &plugin_cache);

    let ext_manifest = format!("{}/extension/com.openai.codexextension.json", &openai_local);
    let config_toml = format!("{}/config.toml", &codex_home);
    let ext_host = format!(
        "{}/plugins/cache/openai-bundled/chrome/latest/extension-host/windows/x64/extension-host.exe",
        &codex_home
    );
    let native_hosts = format!("{}/chrome-native-hosts.json", &codex_local);
    let backup_root = format!("{}/codex-plugin-backups", userprofile);

        // WindowsAppsï¼åç¨ Rust æ ååºè¯»ï¼å¤±è´¥æ¶ç¨ Win32 FFI åéï¼
    let mut winapps_source = String::new();
    if !programfiles.is_empty() {
        let winapps_dir = format!("{}/WindowsApps", &programfiles);
        let found = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if let Ok(entries) = fs::read_dir(&winapps_dir) {
                let mut dirs: Vec<String> = entries
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_name().to_string_lossy().starts_with("OpenAI.Codex_"))
                    .map(|e| e.path().to_string_lossy().to_string())
                    .collect();
                dirs.sort_by(|a, b| b.cmp(a));
                dirs.first().cloned()
            } else {
                None
            }
        }));
        if let Ok(Some(latest)) = found {
            winapps_source = format!("{}/app/resources/plugins/openai-bundled", latest);
        }
        // åéï¼ç¨ Win32 FFI + SE_BACKUP_NAME ç¹æç»è¿ ACL
        if winapps_source.is_empty() || !Path::new(&winapps_source).exists() {
            let win32_dirs = read_winapps_dir_win32(&winapps_dir);
            for d in &win32_dirs {
                let candidate = format!("{}/app/resources/plugins/openai-bundled", d);
                if Path::new(&candidate).exists() {
                    winapps_source = candidate;
                    break;
                }
            }
            // å³ä½¿æ²¡ææ¾å°ææçæä»¶è·¯å¾ï¼ä¹åç¬¬ä¸ä¸ª Codex ç®å½ä½ä¸ºåé
            if winapps_source.is_empty() && !win32_dirs.is_empty() {
                winapps_source = format!("{}/app/resources/plugins/openai-bundled", win32_dirs[0]);
            }
        }
    }

    DetectedPaths {
        codex_home: if dir_exists(&codex_home) { codex_home } else { String::new() },
        bundled_tmp_root: bundled_tmp.clone(),
        marketplace_json: mkt.clone(),
        plugin_cache_root: if dir_exists(&plugin_cache) { plugin_cache } else { String::new() },
        cache_chrome_latest: cache_chrome,
        cache_computer_use_latest: cache_cu,
        cache_browser_latest: cache_br,
        windowsapps_source: winapps_source,
        extension_manifest: ext_manifest,
        config_toml: config_toml,
        extension_host_exe: ext_host,
        native_hosts: native_hosts,
        backup_root: backup_root,
    }
}

// ─── 诊断 ─────────────────────────────────────────────────────────────────────

fn run_diagnostics(paths: &DetectedPaths) -> (Vec<LogEntry>, Vec<Issue>) {
    let mut logs = Vec::new();
    let mut issues = Vec::new();

    logs.push(LogEntry::Title("=== 开始诊断 ===".to_string()));

    // 1. marketplace.json
    logs.push(LogEntry::Step("检查 marketplace.json".to_string()));
    let mkt = &paths.marketplace_json;
    if file_exists(mkt) {
        match serde_json::from_str::<MarketplaceData>(&read_file(mkt)) {
            Ok(data) => {
                let names: Vec<String> = data.plugins.iter().map(|p| p.name.clone()).collect();
                logs.push(LogEntry::Ok(format!(
                    "marketplace.json: {} plugins ({})",
                    names.len(),
                    names.join(", ")
                )));
            }
            Err(e) => {
                issues.push(Issue("marketplace-json-corrupt".to_string()));
                logs.push(LogEntry::Fail(format!("marketplace.json 解析失败: {}", e)));
            }
        }
    } else {
        issues.push(Issue("marketplace-json-missing".to_string()));
        logs.push(LogEntry::Fail("marketplace.json 缺失".to_string()));
    }

    // 2. Cache
    logs.push(LogEntry::Step("检查插件缓存".to_string()));
    for (name, latest_key) in &[
        ("chrome", &paths.cache_chrome_latest),
        ("computer-use", &paths.cache_computer_use_latest),
        ("browser", &paths.cache_browser_latest),
    ] {
        if dir_exists(latest_key) {
            let target = get_junction_target(latest_key);
            let resolved = if !target.is_empty() { target } else { latest_key.to_string() };
            let ver = base_name(&resolved);
            let has_scripts = dir_exists(&format!("{}/scripts", &resolved));
            let has_skills = dir_exists(&format!("{}/skills", &resolved));
            logs.push(LogEntry::Ok(format!(
                "{}: latest -> {} scripts={} skills={}",
                name, ver, has_scripts, has_skills
            )));
            if !has_scripts { issues.push(Issue(format!("cache-no-scripts-{}", name))); }
            if !has_skills { issues.push(Issue(format!("cache-no-skills-{}", name))); }
        } else {
            issues.push(Issue(format!("cache-no-latest-{}", name)));
            logs.push(LogEntry::Fail(format!("{}: latest 不存在", name)));
        }
    }

    // 3. extension-host.exe
    logs.push(LogEntry::Step("检查 extension-host.exe".to_string()));
    if file_exists(&paths.extension_host_exe) {
        logs.push(LogEntry::Ok(format!("extension-host.exe: {} bytes", file_size(&paths.extension_host_exe))));
    } else {
        issues.push(Issue("extension-host-exe-missing".to_string()));
        logs.push(LogEntry::Fail("extension-host.exe 未找到".to_string()));
    }

    // 4. Processes (pure Rust via sysinfo)
    logs.push(LogEntry::Step("检查运行的插件进程".to_string()));
    {
        let mut sys = System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        let names = ["extension-host", "codex-computer-use"];
        let count: Vec<_> = sys
            .processes()
            .iter()
            .filter(|(_, p)| {
                let n = p.name().to_string_lossy();
                names.iter().any(|x| n.contains(x) || n == *x)
            })
            .collect();
        if count.len() > 0 {
            logs.push(LogEntry::Info(format!("发现 {} 个插件进程在运行", count.len())));
            issues.push(Issue("plugin-processes-running".to_string()));
        } else {
            logs.push(LogEntry::Ok("没有插件进程在运行".to_string()));
        }
    }

    // 5. Extension manifest
    logs.push(LogEntry::Step("检查扩展清单".to_string()));
    if file_exists(&paths.extension_manifest) {
        match serde_json::from_str::<serde_json::Value>(&read_file(&paths.extension_manifest)) {
            Ok(val) => {
                let exe = val.get("path").and_then(|v| v.as_str()).unwrap_or("");
                if file_exists(exe) {
                    logs.push(LogEntry::Ok("扩展清单有效".to_string()));
                } else {
                    logs.push(LogEntry::Warn(format!("扩展清单指向无效路径: {}", exe)));
                }
            }
            Err(_) => {
                issues.push(Issue("extension-manifest-corrupt".to_string()));
                logs.push(LogEntry::Fail("扩展清单已损坏".to_string()));
            }
        }
    } else {
        issues.push(Issue("extension-manifest-missing".to_string()));
        logs.push(LogEntry::Warn("扩展清单未找到".to_string()));
    }

    // 6. Config
    logs.push(LogEntry::Step("检查 config.toml".to_string()));
    if file_exists(&paths.config_toml) {
        let content = read_file(&paths.config_toml);
        for name in &["computer-use@openai-bundled", "chrome@openai-bundled", "browser@openai-bundled"] {
            if content.contains(name) {
                logs.push(LogEntry::Ok(format!("{} 已配置", name)));
            } else {
                issues.push(Issue(format!("config-missing-{}", name)));
                logs.push(LogEntry::Warn(format!("{} 未配置", name)));
            }
        }
    } else {
        logs.push(LogEntry::Fail("config.toml 不存在".to_string()));
    }

    // 7. WindowsApps（修复源检测）
    logs.push(LogEntry::Step("检查修复源（WindowsApps / 本地缓存）".to_string()));
    if !paths.windowsapps_source.is_empty() && dir_exists(&paths.windowsapps_source) {
        let src_mkt = format!("{}/.agents/plugins/marketplace.json", &paths.windowsapps_source);
        if file_exists(&src_mkt) {
            logs.push(LogEntry::Ok("WindowsApps 有完整 marketplace.json（可用作修复源）".to_string()));
        } else {
            logs.push(LogEntry::Warn("WindowsApps 存在但 marketplace.json 缺失".to_string()));
        }
    } else {
        logs.push(LogEntry::Warn("WindowsApps 不可读（非 Store 安装或权限限制）".to_string()));
        // 检查是否有可用的缓存已安装插件
        let cache_ok = dir_exists(&paths.plugin_cache_root);
        let mkt_ok = file_exists(&paths.marketplace_json);
        if mkt_ok {
            logs.push(LogEntry::Info("本地 marketplace.json 已存在，无需外部修复源".to_string()));
        } else if cache_ok {
            logs.push(LogEntry::Info("插件缓存存在，可作为备用修复源".to_string()));
        }
    }

    logs.push(LogEntry::Title(format!("=== 诊断完成：共 {} 个问题 ===", issues.len())));
    for i in &issues {
        logs.push(LogEntry::Warn(format!("  * {}", i.0)));
    }
    (logs, issues)
}

// ─── 修复 ─────────────────────────────────────────────────────────────────────

fn run_repair(paths: &DetectedPaths, issues: &[Issue], log_tx: mpsc::Sender<LogEntry>) {
    let s = |entry| { let _ = log_tx.send(entry); };

    s(LogEntry::Title("=== 开始修复 ===".to_string()));

    // 1. Kill processes
    s(LogEntry::Step("停止插件进程".to_string()));
    kill_extension_host_processes();
    thread::sleep(std::time::Duration::from_secs(1));
    s(LogEntry::Ok("已停止插件进程".to_string()));

    // 2. Backup
    s(LogEntry::Step("创建备份".to_string()));
    let stamp = Local::now().format("%Y%m%d-%H%M%S").to_string();
    let backup_dir = format!("{}/openai-bundled-repair-{}", paths.backup_root, stamp);
    let _ = fs::create_dir_all(&backup_dir);

    for (label, src) in &[("config.toml", &paths.config_toml), ("扩展清单", &paths.extension_manifest), ("native-hosts", &paths.native_hosts)] {
        if file_exists(src) {
            let name = Path::new(src).file_name().unwrap().to_string_lossy();
            let _ = fs::copy(src, format!("{}/{}", &backup_dir, name));
            s(LogEntry::Ok(format!("已备份: {}", label)));
        }
    }
    if dir_exists(&paths.bundled_tmp_root) && issues.iter().any(|i| i.0 == "marketplace-json-missing") {
        let forensic = format!("{}/bundled-marketplace-forensic", &backup_dir);
        if copy_dir_recursive(Path::new(&paths.bundled_tmp_root), Path::new(&forensic)).is_ok() {
            s(LogEntry::Ok("已对损坏的 marketplace 做取证备份".to_string()));
        }
    }
    s(LogEntry::Ok(format!("备份位置: {}", &backup_dir)));

    // 3. Rebuild marketplace（从 WindowsApps 复制，不可用时跳过）
    s(LogEntry::Step("检查 marketplace".to_string()));
    let mkt_ok = file_exists(&paths.marketplace_json);
    if !mkt_ok && dir_exists(&paths.windowsapps_source) {
        s(LogEntry::Step("重建 marketplace".to_string()));
        let target = Path::new(&paths.bundled_tmp_root);
        if target.exists() {
            let _ = fs::remove_dir_all(target);
        }
        match copy_dir_recursive(Path::new(&paths.windowsapps_source), target) {
            Ok(_) => {
                let mkt = format!("{}/.agents/plugins/marketplace.json", &paths.bundled_tmp_root);
                if file_exists(&mkt) {
                    s(LogEntry::Ok(format!("marketplace.json 已恢复：{} 字节", file_size(&mkt))));
                } else {
                    s(LogEntry::Fail("marketplace.json 仍然缺失".to_string()));
                }
            }
            Err(e) => s(LogEntry::Fail(format!("复制失败: {}", e))),
        }
    } else if mkt_ok {
        s(LogEntry::Ok("marketplace.json 已存在，无需重建".to_string()));
    } else {
        s(LogEntry::Warn("marketplace.json 缺失且 WindowsApps 不可读，无法自动重建".to_string()));
        s(LogEntry::Info("解决办法：重新安装 Codex 或手动复制 marketplace.json".to_string()));
    }

    // 4. Fix cache junctions
    s(LogEntry::Step("修复缓存软连接".to_string()));
    for name in &["chrome", "computer-use", "browser"] {
        let cache_dir = format!("{}/{}", paths.plugin_cache_root, name);
        if !dir_exists(&cache_dir) { continue; }

        let versions = get_sorted_version_dirs(&cache_dir);
        if versions.is_empty() {
            s(LogEntry::Warn(format!("{}: 没有版本目录", name)));
            continue;
        }

        let best_ver = &versions[0];
        let best_path = format!("{}/{}", &cache_dir, best_ver);
        let latest_junc = format!("{}/latest", &cache_dir);
        let has_scripts = dir_exists(&format!("{}/scripts", &best_path));
        let has_skills = dir_exists(&format!("{}/skills", &best_path));

        // Check current
        let needs_fix = if !Path::new(&latest_junc).exists() {
            true
        } else {
            let current = get_junction_target(&latest_junc);
            current.is_empty() || base_name(&current) != *best_ver || !has_scripts || !has_skills
        };

        if needs_fix {
            if create_junction(&best_path, &latest_junc) {
                s(LogEntry::Ok(format!(
                    "{}: 已修复 -> {} (scripts={} skills={})",
                    name, best_ver, has_scripts, has_skills
                )));
            } else {
                s(LogEntry::Fail(format!("{}: 修复失败（需要管理员权限）", name)));
            }
        } else {
            s(LogEntry::Ok(format!(
                "{}: latest -> {} (scripts={} skills={})",
                name, best_ver, has_scripts, has_skills
            )));
        }
    }

    s(LogEntry::Step("检查 chrome-native-hosts.json".to_string()));
    if file_exists(&paths.native_hosts) {
        s(LogEntry::Ok("chrome-native-hosts.json 存在".to_string()));
    } else {
        s(LogEntry::Info("未找到 chrome-native-hosts.json（重启后生成）".to_string()));
    }

    s(LogEntry::Title("=== 修复完成 ===".to_string()));
    s(LogEntry::Info("请重启 Codex Desktop 使改动生效".to_string()));
    s(LogEntry::Plain(()));
}

// ─── GUI ─────────────────────────────────────────────────────────────────────

struct CodexRepairApp {
    paths: DetectedPaths,
    logs: Vec<LogEntry>,
    issues: Vec<Issue>,
    diagnose_running: bool,
    repair_running: bool,
    log_rx: Option<mpsc::Receiver<LogEntry>>,
    diag_rx: Option<mpsc::Receiver<(Vec<LogEntry>, Vec<Issue>)>>,
    path_inputs: HashMap<String, String>,
}

impl Default for CodexRepairApp {
    fn default() -> Self {
        let detected = detect_paths();
        let mut inputs = HashMap::new();
        inputs.insert("codex_home".into(), detected.codex_home.clone());
        inputs.insert("marketplace_json".into(), detected.marketplace_json.clone());
        inputs.insert("windowsapps_source".into(), detected.windowsapps_source.clone());
        inputs.insert("plugin_cache_root".into(), detected.plugin_cache_root.clone());
        inputs.insert("extension_manifest".into(), detected.extension_manifest.clone());
        inputs.insert("config_toml".into(), detected.config_toml.clone());
        inputs.insert("extension_host_exe".into(), detected.extension_host_exe.clone());
        Self {
            paths: detected,
            logs: Vec::new(),
            issues: Vec::new(),
            diagnose_running: false,
            repair_running: false,
            log_rx: None,
            diag_rx: None,
            path_inputs: inputs,
        }
    }
}

impl eframe::App for CodexRepairApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // ── 标题 ──
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.heading(APP_NAME);
        });

        // ── 路径面板 ──
        egui::TopBottomPanel::top("paths").resizable(false).show(ctx, |ui| {
            egui::CollapsingHeader::new("路径配置（可手动修改）")
                .default_open(true)
                .show(ui, |ui| {
                    egui::Grid::new("path_grid").num_columns(2).striped(true).spacing([8.0, 4.0]).show(ui, |ui| {
                        for (label, key) in &[
                            ("Codex 目录", "codex_home"),
                            ("marketplace.json", "marketplace_json"),
                            ("WindowsApps 源", "windowsapps_source"),
                            ("插件缓存根目录", "plugin_cache_root"),
                            ("扩展清单", "extension_manifest"),
                            ("config.toml", "config_toml"),
                            ("extension-host.exe", "extension_host_exe"),
                        ] {
                            ui.label(*label);
                            let mut val = self.path_inputs.get(*key).cloned().unwrap_or_default();
                            if ui.text_edit_singleline(&mut val).changed() {
                                self.path_inputs.insert(key.to_string(), val);
                            }
                            ui.end_row();
                        }
                    });
                });
        });

        // ── 按钮 ──
        egui::TopBottomPanel::top("buttons").resizable(false).show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("🔄 刷新路径").clicked() {
                    let d = detect_paths();
                    self.paths = d.clone();
                    for (k, v) in [("codex_home", &d.codex_home), ("marketplace_json", &d.marketplace_json),
                                   ("windowsapps_source", &d.windowsapps_source), ("plugin_cache_root", &d.plugin_cache_root),
                                   ("extension_manifest", &d.extension_manifest), ("config_toml", &d.config_toml),
                                   ("extension_host_exe", &d.extension_host_exe)] {
                        self.path_inputs.insert(k.to_string(), v.clone());
                    }
                    self.logs.push(LogEntry::Info("路径已刷新".to_string()));
                }

                let diag_label = if self.diagnose_running { "⏳ 诊断中..." } else { "🔍 诊断" };
                if ui.button(diag_label).clicked() && !self.diagnose_running {
                    self.diagnose_running = true;
                    self.logs.clear();
                    self.issues.clear();

                    self.paths.codex_home = self.path_inputs.get("codex_home").cloned().unwrap_or_default();
                    self.paths.marketplace_json = self.path_inputs.get("marketplace_json").cloned().unwrap_or_default();
                    self.paths.windowsapps_source = self.path_inputs.get("windowsapps_source").cloned().unwrap_or_default();
                    self.paths.plugin_cache_root = self.path_inputs.get("plugin_cache_root").cloned().unwrap_or_default();
                    self.paths.extension_manifest = self.path_inputs.get("extension_manifest").cloned().unwrap_or_default();
                    self.paths.config_toml = self.path_inputs.get("config_toml").cloned().unwrap_or_default();
                    self.paths.extension_host_exe = self.path_inputs.get("extension_host_exe").cloned().unwrap_or_default();

                    let paths = self.paths.clone();
                    let (tx, rx) = mpsc::channel();
                    self.diag_rx = Some(rx);
                    let ctx_clone = ctx.clone();
                    thread::spawn(move || {
                        let result = run_diagnostics(&paths);
                        let _ = tx.send(result);
                        ctx_clone.request_repaint();
                    });
                }

                let repair_label = if self.repair_running { "⏳ 修复中..." } else { "🔧 修复" };
                let has_issues = !self.issues.is_empty();
                if ui.button(repair_label).clicked() && !self.repair_running && has_issues {
                    self.repair_running = true;
                    let paths = self.paths.clone();
                    let issues = self.issues.clone();
                    let (tx, rx) = mpsc::channel();
                    self.log_rx = Some(rx);
                    let ctx_clone = ctx.clone();
                    thread::spawn(move || {
                        run_repair(&paths, &issues, tx);
                        ctx_clone.request_repaint();
                    });
                }
            });
        });

        // ── 日志区 ──
        egui::CentralPanel::default().show(ctx, |ui| {
            // Drain diag channel
            if let Some(rx) = self.diag_rx.take() {
                if let Ok((logs, issues)) = rx.try_recv() {
                    self.logs = logs;
                    self.issues = issues;
                    self.diagnose_running = false;
                } else {
                    self.diag_rx = Some(rx);
                }
            }
            // Drain repair channel
            if let Some(rx) = self.log_rx.take() {
                let mut done = false;
                while let Ok(entry) = rx.try_recv() {
                    match &entry {
                        LogEntry::Plain(_) => done = true,
                        _ => self.logs.push(entry),
                    }
                }
                if done { self.repair_running = false; }
                else { self.log_rx = Some(rx); }
            }

            egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
                for entry in &self.logs {
                    match entry {
                        LogEntry::Title(s) => { ui.colored_label(egui::Color32::from_rgb(0, 100, 200), s); }
                        LogEntry::Step(s) => { ui.colored_label(egui::Color32::from_rgb(0, 100, 200), s); }
                        LogEntry::Ok(s) => { ui.colored_label(egui::Color32::from_rgb(0, 160, 0), &format!("  [OK]    {}", s)); }
                        LogEntry::Fail(s) => { ui.colored_label(egui::Color32::from_rgb(200, 0, 0), &format!("  [FAIL]  {}", s)); }
                        LogEntry::Warn(s) => { ui.colored_label(egui::Color32::from_rgb(200, 130, 0), &format!("  [WARN]  {}", s)); }
                        LogEntry::Info(s) => { ui.colored_label(egui::Color32::from_rgb(0, 100, 200), &format!("  [INFO]  {}", s)); }
                        LogEntry::Plain(_) => {}
                    }
                }
                if self.logs.is_empty() {
                    ui.colored_label(egui::Color32::GRAY, "请点击「诊断」开始检查");
                }
            });
        });
    }
}

// ─── 入口 ─────────────────────────────────────────────────────────────────────

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([860.0, 640.0])
            .with_min_inner_size([600.0, 400.0]),
        ..Default::default()
    };
    eframe::run_native(APP_NAME, options, Box::new(|cc| {
        // 加载 CJK 字体（多字体回退，兼容不同语言 Windows）
        let def = egui::FontDefinitions::default();
        let mut fonts = egui::FontDefinitions::empty();
        for (k, v) in &def.font_data {
            fonts.font_data.insert(k.clone(), v.clone());
        }
        for (fam, list) in &def.families {
            fonts.families.insert(fam.clone(), list.clone());
        }

        // 按优先级检查系统字体（中文 → 日文 → 韩文 → 其他）
        let font_candidates = [
            // 简体中文
            ("C:/Windows/Fonts/msyh.ttc", "msyh"),
            ("C:/Windows/Fonts/msyhbd.ttc", "msyhbd"),
            ("C:/Windows/Fonts/Deng.ttf", "deng"),
            ("C:/Windows/Fonts/simfang.ttf", "simfang"),
            ("C:/Windows/Fonts/simhei.ttf", "simhei"),
            ("C:/Windows/Fonts/simsun.ttc", "simsun"),
            ("C:/Windows/Fonts/simkai.ttf", "simkai"),
            // 繁体中文
            ("C:/Windows/Fonts/msjh.ttc", "msjh"),
            // 日文
            ("C:/Windows/Fonts/msgothic.ttc", "msgothic"),
            // 韩文
            ("C:/Windows/Fonts/malgun.ttf", "malgun"),
            // 英文 Windows 上的泛中日韩字体
            ("C:/Windows/Fonts/NotoSansSC-VF.ttf", "notosans"),
            ("C:/Windows/Fonts/NotoSerifSC-VF.ttf", "notoserif"),
        ];

        let mut cjk_loaded = false;
        for (path, name) in &font_candidates {
            if let Ok(data) = std::fs::read(path) {
                let key = name.to_string();
                fonts.font_data.insert(key.clone(),
                    std::sync::Arc::new(egui::FontData::from_owned(data)));
                for (_, list) in fonts.families.iter_mut() {
                    list.insert(0, key.clone());
                }
                cjk_loaded = true;
                break;
            }
        }

        if !cjk_loaded {
            // 写调试文件
            let debug_path = std::env::var("USERPROFILE").unwrap_or_default() + "/Desktop/codex-repair-font-debug.txt";
            let tried: String = font_candidates.iter().map(|(p,_)| format!("  - {}\n", p)).collect();
            let _ = std::fs::write(&debug_path, format!("ERROR: No CJK font could be loaded.\nTried:\n{}", tried));
        }
        cc.egui_ctx.set_fonts(fonts);
        Ok(Box::new(CodexRepairApp::default()))
    }))
}






























