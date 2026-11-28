use crate::layers::{
    parse_hex_color, BgGradient, DropShadow, GlowEffect, Layer, LayerEffects, LayerGroup,
    LayerKind, LayerOutline, SharedProjectState,
};
use crate::tools::{internal_get_preview_base64, render_and_respond};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, Write};
use tauri::Emitter;

// ── Universal MCP Auto-Connector ────────────────────────────────────────────

#[derive(Clone, serde::Serialize)]
pub struct AiClientStatus {
    pub id: String,
    pub name: String,
    pub install_type: String, // "Store (MSIX)" | "Web" | "CLI" | ""
    pub installed: bool,
    pub connected: bool,
    pub outdated: bool,
    pub status: String, // "not_detected" | "installed" | "connected" | "outdated"
    pub config_path: String,
}

#[derive(Clone, serde::Serialize)]
pub struct McpSnippets {
    pub exe_path: String,
    pub exe_command: String,
    pub json_snippet: String,
}

fn appdata_dir() -> String {
    std::env::var("APPDATA").unwrap_or_else(|_| r"C:\Users\Public\AppData\Roaming".to_string())
}

fn local_appdata_dir() -> String {
    std::env::var("LOCALAPPDATA").unwrap_or_else(|_| r"C:\Users\Public\AppData\Local".to_string())
}

fn userprofile_dir() -> String {
    std::env::var("USERPROFILE").unwrap_or_else(|_| r"C:\Users\Public".to_string())
}

fn norm_path_str(p: &str) -> String {
    p.trim().replace('\\', "/").to_lowercase()
}

fn current_exe_norm() -> String {
    std::env::current_exe()
        .map(|p| norm_path_str(&p.to_string_lossy()))
        .unwrap_or_default()
}

fn has_exe_in_dir(dir: &std::path::Path, exe_names: &[&str]) -> bool {
    if !dir.is_dir() {
        return false;
    }
    for name in exe_names {
        if dir.join(name).is_file() {
            return true;
        }
    }
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                if let Some(dname) = p.file_name().and_then(|n| n.to_str()) {
                    if dname.starts_with("app-") {
                        for name in exe_names {
                            if p.join(name).is_file() {
                                return true;
                            }
                        }
                    }
                }
            }
        }
    }
    false
}

fn cmd_in_path(bin: &str) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        std::process::Command::new("where")
            .arg(bin)
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new("which")
            .arg(bin)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

fn uninstall_reg_has(keyword: &str) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let keys = [
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall",
            r"HKLM\Software\Microsoft\Windows\CurrentVersion\Uninstall",
            r"HKLM\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
        ];
        for key in keys {
            if let Ok(out) = std::process::Command::new("reg")
                .args(["query", key, "/s", "/f", keyword, "/d"])
                .creation_flags(CREATE_NO_WINDOW)
                .output()
            {
                if out.status.success() {
                    let s = String::from_utf8_lossy(&out.stdout);
                    if s.to_lowercase().contains(&keyword.to_lowercase()) {
                        return true;
                    }
                }
            }
        }
        false
    }
    #[cfg(not(windows))]
    {
        let _ = keyword;
        false
    }
}

/// Returns `(connected, outdated)` for a JSON MCP config file.
fn json_veltrax_status(path: &std::path::Path, cur_exe: &str) -> (bool, bool) {
    if !path.is_file() {
        return (false, false);
    }
    let Ok(raw) = std::fs::read_to_string(path) else {
        return (false, false);
    };
    let Ok(v) = serde_json::from_str::<Value>(&raw) else {
        return (false, false);
    };
    let Some(entry) = v.get("mcpServers").and_then(|s| s.get("veltrax-studios")) else {
        return (false, false);
    };
    let cmd = entry.get("command").and_then(|c| c.as_str()).unwrap_or("");
    if !cur_exe.is_empty() && norm_path_str(cmd) == cur_exe {
        (true, false)
    } else {
        (false, true)
    }
}

/// Returns `(connected, outdated)` for a TOML MCP config file.
fn toml_veltrax_status(path: &std::path::Path, cur_exe: &str) -> (bool, bool) {
    if !path.is_file() {
        return (false, false);
    }
    let Ok(raw) = std::fs::read_to_string(path) else {
        return (false, false);
    };
    let mut in_section = false;
    let mut found_section = false;
    let mut cmd_val: Option<String> = None;
    for line in raw.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_section = t == "[mcp_servers.veltrax-studios]";
            if in_section {
                found_section = true;
            }
            continue;
        }
        if in_section && t.starts_with("command") {
            if let Some((_, rhs)) = t.split_once('=') {
                let cleaned = rhs.trim().trim_matches('"').trim_matches('\'');
                cmd_val = Some(cleaned.to_string());
            }
        }
    }
    if !found_section {
        return (false, false);
    }
    match cmd_val {
        Some(c) if !cur_exe.is_empty() && norm_path_str(&c) == cur_exe => (true, false),
        _ => (false, true),
    }
}

fn status_label(installed: bool, connected: bool, outdated: bool) -> String {
    if connected {
        "connected".to_string()
    } else if outdated {
        "outdated".to_string()
    } else if installed {
        "installed".to_string()
    } else {
        "not_detected".to_string()
    }
}

/// Find Microsoft Store (MSIX) Claude Desktop package config path without touching WindowsApps.
/// Looks in `%LOCALAPPDATA%\Packages\Claude_*` (or `Claude*` containing `LocalCache\Roaming\Claude`).
fn find_claude_store_config(local_appdata: &str) -> Option<std::path::PathBuf> {
    let packages = std::path::Path::new(local_appdata).join("Packages");
    let entries = std::fs::read_dir(&packages).ok()?;
    let mut best: Option<std::path::PathBuf> = None;
    for entry in entries.flatten() {
        let pkg_dir = entry.path();
        if !pkg_dir.is_dir() {
            continue;
        }
        let Some(name) = pkg_dir.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.starts_with("Claude_") && !name.eq_ignore_ascii_case("Claude") {
            if !name.starts_with("Claude") {
                continue;
            }
            if !pkg_dir
                .join("LocalCache")
                .join("Roaming")
                .join("Claude")
                .is_dir()
            {
                continue;
            }
        }
        let cfg = pkg_dir
            .join("LocalCache")
            .join("Roaming")
            .join("Claude")
            .join("claude_desktop_config.json");
        if cfg.is_file() {
            return Some(cfg);
        }
        if best.is_none() {
            best = Some(cfg);
        }
    }
    best
}

#[tauri::command]
pub fn scan_desktop_ai_clients() -> Vec<AiClientStatus> {
    let appdata = appdata_dir();
    let local_appdata = local_appdata_dir();
    let userprofile = userprofile_dir();
    let cur_exe = current_exe_norm();

    let claude_classic_cfg = std::path::Path::new(&appdata)
        .join("Claude")
        .join("claude_desktop_config.json");
    let claude_classic_installed = has_exe_in_dir(
        &std::path::Path::new(&local_appdata).join("AnthropicClaude"),
        &["claude.exe", "Claude.exe"],
    ) || has_exe_in_dir(
        &std::path::Path::new(&local_appdata)
            .join("Programs")
            .join("claude"),
        &["Claude.exe", "claude.exe"],
    ) || std::path::Path::new(r"C:\Program Files\Claude\Claude.exe")
        .is_file()
        || uninstall_reg_has("Claude");

    let claude_store_cfg = find_claude_store_config(&local_appdata);
    let claude_store_installed = claude_store_cfg.is_some();

    let mut out = Vec::new();

    if claude_store_installed && claude_classic_installed {
        // Both installed: expose both clearly (Store + Web)
        let store_path = claude_store_cfg.unwrap();
        let (sc, so) = json_veltrax_status(&store_path, &cur_exe);
        out.push(AiClientStatus {
            id: "claude_store".to_string(),
            name: "Claude Desktop".to_string(),
            install_type: "Store (MSIX)".to_string(),
            installed: true,
            connected: sc,
            outdated: so,
            status: status_label(true, sc, so),
            config_path: store_path.display().to_string(),
        });
        let (cc, co) = json_veltrax_status(&claude_classic_cfg, &cur_exe);
        out.push(AiClientStatus {
            id: "claude".to_string(),
            name: "Claude Desktop".to_string(),
            install_type: "Web".to_string(),
            installed: true,
            connected: cc,
            outdated: co,
            status: status_label(true, cc, co),
            config_path: claude_classic_cfg.display().to_string(),
        });
    } else if let Some(store_path) = claude_store_cfg {
        // Store (MSIX) installation detected
        let (sc, so) = json_veltrax_status(&store_path, &cur_exe);
        out.push(AiClientStatus {
            id: "claude".to_string(),
            name: "Claude Desktop".to_string(),
            install_type: "Store (MSIX)".to_string(),
            installed: true,
            connected: sc,
            outdated: so,
            status: status_label(true, sc, so),
            config_path: store_path.display().to_string(),
        });
    } else {
        // Classic (Web) or not detected
        let (cc, co) = json_veltrax_status(&claude_classic_cfg, &cur_exe);
        out.push(AiClientStatus {
            id: "claude".to_string(),
            name: "Claude Desktop".to_string(),
            install_type: if claude_classic_installed {
                "Web".to_string()
            } else {
                String::new()
            },
            installed: claude_classic_installed,
            connected: cc,
            outdated: co,
            status: status_label(claude_classic_installed, cc, co),
            config_path: claude_classic_cfg.display().to_string(),
        });
    }

    let lmstudio_cfg = std::path::Path::new(&userprofile)
        .join(".lmstudio")
        .join("mcp.json");
    let lmstudio_installed = has_exe_in_dir(
        &std::path::Path::new(&local_appdata).join("LM-Studio"),
        &["LM Studio.exe"],
    ) || has_exe_in_dir(
        &std::path::Path::new(&local_appdata)
            .join("Programs")
            .join("LM Studio"),
        &["LM Studio.exe"],
    ) || std::path::Path::new(r"C:\Program Files\LM Studio\LM Studio.exe")
        .is_file()
        || uninstall_reg_has("LM Studio");
    let (lm_conn, lm_out) = json_veltrax_status(&lmstudio_cfg, &cur_exe);
    out.push(AiClientStatus {
        id: "lmstudio".to_string(),
        name: "LM Studio".to_string(),
        install_type: if lmstudio_installed {
            "Desktop".to_string()
        } else {
            String::new()
        },
        installed: lmstudio_installed,
        connected: lm_conn,
        outdated: lm_out,
        status: status_label(lmstudio_installed, lm_conn, lm_out),
        config_path: lmstudio_cfg.display().to_string(),
    });

    let codex_cfg = [
        std::path::Path::new(&userprofile)
            .join(".codex")
            .join("config.toml"),
        std::path::Path::new(&appdata)
            .join("codex")
            .join("config.toml"),
    ]
    .into_iter()
    .find(|p| p.is_file())
    .unwrap_or_else(|| {
        std::path::Path::new(&userprofile)
            .join(".codex")
            .join("config.toml")
    });
    let codex_installed = cmd_in_path("codex")
        || std::path::Path::new(&appdata)
            .join("npm")
            .join("codex.cmd")
            .is_file()
        || std::path::Path::new(&userprofile)
            .join(".cargo")
            .join("bin")
            .join("codex.exe")
            .is_file();
    let (codex_conn, codex_out) = toml_veltrax_status(&codex_cfg, &cur_exe);
    out.push(AiClientStatus {
        id: "codex".to_string(),
        name: "ChatGPT / Codex".to_string(),
        install_type: if codex_installed {
            "CLI".to_string()
        } else {
            String::new()
        },
        installed: codex_installed,
        connected: codex_conn,
        outdated: codex_out,
        status: status_label(codex_installed, codex_conn, codex_out),
        config_path: codex_cfg.display().to_string(),
    });

    let cursor_cfg = [
        std::path::Path::new(&userprofile)
            .join(".cursor")
            .join("mcp.json"),
        std::path::Path::new(&appdata)
            .join("Cursor")
            .join("User")
            .join("globalStorage")
            .join("mcp.json"),
    ]
    .into_iter()
    .find(|p| p.is_file())
    .unwrap_or_else(|| {
        std::path::Path::new(&userprofile)
            .join(".cursor")
            .join("mcp.json")
    });
    let cursor_installed = has_exe_in_dir(
        &std::path::Path::new(&local_appdata)
            .join("Programs")
            .join("cursor"),
        &["Cursor.exe"],
    ) || has_exe_in_dir(
        &std::path::Path::new(&local_appdata).join("cursor"),
        &["Cursor.exe"],
    ) || std::path::Path::new(r"C:\Program Files\cursor\Cursor.exe")
        .is_file()
        || uninstall_reg_has("Cursor");
    let (cursor_conn, cursor_out) = json_veltrax_status(&cursor_cfg, &cur_exe);
    out.push(AiClientStatus {
        id: "cursor".to_string(),
        name: "Cursor".to_string(),
        install_type: if cursor_installed {
            "Desktop".to_string()
        } else {
            String::new()
        },
        installed: cursor_installed,
        connected: cursor_conn,
        outdated: cursor_out,
        status: status_label(cursor_installed, cursor_conn, cursor_out),
        config_path: cursor_cfg.display().to_string(),
    });

    out
}

fn backup_existing_file(path: &std::path::Path) -> Result<(), String> {
    if path.is_file() {
        let bak = std::path::PathBuf::from(format!("{}.bak", path.display()));
        std::fs::copy(path, &bak).map_err(|e| format!("Failed to create backup: {}", e))?;
    }
    Ok(())
}

fn write_json_mcp_entry(config_path: &std::path::Path, exe_str: &str) -> Result<(), String> {
    let mut config: Value = if config_path.is_file() {
        backup_existing_file(config_path)?;
        let raw = std::fs::read_to_string(config_path)
            .map_err(|e| format!("Failed to read existing config: {}", e))?;
        serde_json::from_str(&raw).map_err(|e| {
            format!(
                "Existing config JSON is invalid ({}). Backup saved at {}.bak; aborting to avoid data loss.",
                e,
                config_path.display()
            )
        })?
    } else {
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        }
        json!({})
    };
    if !config.is_object() {
        return Err("Existing config root is not a JSON object; aborting.".to_string());
    }
    if config.get("mcpServers").is_none() || !config["mcpServers"].is_object() {
        config["mcpServers"] = json!({});
    }
    config["mcpServers"]["veltrax-studios"] = json!({
        "command": exe_str,
        "args": ["--mcp"]
    });
    let pretty = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    std::fs::write(config_path, pretty).map_err(|e| format!("Failed to write config: {}", e))
}

#[tauri::command]
pub fn connect_ai_client(client_id: String) -> Result<String, String> {
    let clients = scan_desktop_ai_clients();
    let client = clients
        .iter()
        .find(|c| c.id == client_id)
        .ok_or_else(|| format!("Unknown AI client '{}'", client_id))?;

    if !client.installed {
        return Err(format!(
            "{} is not detected as installed on this PC. Install it first or use Manual Setup.",
            client.name
        ));
    }

    let exe_path =
        std::env::current_exe().map_err(|e| format!("Failed to get executable path: {}", e))?;
    let exe_str = exe_path.to_string_lossy().replace('\\', "/");

    let cfg = std::path::PathBuf::from(&client.config_path);
    match client_id.as_str() {
        "claude" | "claude_store" | "lmstudio" | "cursor" => {
            write_json_mcp_entry(&cfg, &exe_str)?;
            Ok(format!("Connected to {} ({})", client.name, cfg.display()))
        }
        "codex" => {
            if cfg.is_file() {
                backup_existing_file(&cfg)?;
            } else if let Some(parent) = cfg.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create .codex dir: {}", e))?;
            }
            let raw = if cfg.is_file() {
                std::fs::read_to_string(&cfg)
                    .map_err(|e| format!("Failed to read config.toml: {}", e))?
            } else {
                String::new()
            };
            // Remove any existing [mcp_servers.veltrax-studios] block so outdated paths are replaced cleanly
            let mut out = String::new();
            let mut skipping = false;
            for line in raw.lines() {
                let trimmed = line.trim();
                if trimmed == "[mcp_servers.veltrax-studios]" {
                    skipping = true;
                    continue;
                }
                if skipping && trimmed.starts_with('[') {
                    skipping = false;
                }
                if !skipping {
                    out.push_str(line);
                    out.push('\n');
                }
            }
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(&format!(
                "\n[mcp_servers.veltrax-studios]\ncommand = '{}'\nargs = [\"--mcp\"]\n",
                exe_str
            ));
            std::fs::write(&cfg, out).map_err(|e| format!("Failed to write config.toml: {}", e))?;
            Ok(format!("Connected to ChatGPT / Codex ({})", cfg.display()))
        }
        other => Err(format!("Unknown AI client '{}'", other)),
    }
}

#[tauri::command]
pub fn disconnect_ai_client(client_id: String) -> Result<String, String> {
    let clients = scan_desktop_ai_clients();
    let cfg = clients
        .iter()
        .find(|c| c.id == client_id)
        .map(|c| std::path::PathBuf::from(&c.config_path))
        .ok_or_else(|| format!("Unknown AI client '{}'", client_id))?;

    match client_id.as_str() {
        "claude" | "claude_store" | "lmstudio" | "cursor" => {
            if cfg.is_file() {
                backup_existing_file(&cfg)?;
                let raw = std::fs::read_to_string(&cfg).map_err(|e| e.to_string())?;
                let mut v: Value = serde_json::from_str(&raw)
                    .map_err(|e| format!("Existing config JSON is invalid ({}); aborting.", e))?;
                if let Some(obj) = v.get_mut("mcpServers").and_then(|s| s.as_object_mut()) {
                    obj.remove("veltrax-studios");
                    let pretty = serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?;
                    std::fs::write(&cfg, pretty).map_err(|e| e.to_string())?;
                }
            }
            Ok(format!("Disconnected {}", client_id))
        }
        "codex" => {
            if cfg.is_file() {
                backup_existing_file(&cfg)?;
                let raw = std::fs::read_to_string(&cfg).unwrap_or_default();
                let mut out = String::new();
                let mut skipping = false;
                for line in raw.lines() {
                    let trimmed = line.trim();
                    if trimmed == "[mcp_servers.veltrax-studios]" {
                        skipping = true;
                        continue;
                    }
                    if skipping && trimmed.starts_with('[') {
                        skipping = false;
                    }
                    if !skipping {
                        out.push_str(line);
                        out.push('\n');
                    }
                }
                std::fs::write(&cfg, out).map_err(|e| e.to_string())?;
            }
            Ok("Disconnected ChatGPT / Codex".to_string())
        }
        other => Err(format!("Unknown AI client '{}'", other)),
    }
}

#[tauri::command]
pub fn connect_all_detected() -> Result<Vec<String>, String> {
    let clients = scan_desktop_ai_clients();
    let mut results = Vec::new();
    for c in clients {
        if c.installed {
            let msg = connect_ai_client(c.id)?;
            results.push(msg);
        }
    }
    Ok(results)
}

#[tauri::command]
pub fn get_mcp_snippets() -> Result<McpSnippets, String> {
    let exe_path =
        std::env::current_exe().map_err(|e| format!("Failed to get executable path: {}", e))?;
    let exe_str = exe_path.to_string_lossy().replace('\\', "/");
    let exe_command = format!("\"{}\" --mcp", exe_str);
    let json_snippet = serde_json::to_string_pretty(&json!({
        "mcpServers": {
            "veltrax-studios": {
                "command": exe_str,
                "args": ["--mcp"]
            }
        }
    }))
    .unwrap_or_default();

    Ok(McpSnippets {
        exe_path: exe_str,
        exe_command,
        json_snippet,
    })
}

#[tauri::command]
pub fn connect_claude_desktop() -> Result<String, String> {
    connect_ai_client("claude".to_string())
}

#[derive(serde::Serialize)]
pub struct McpRealStatus {
    pub bridge_online: bool,
    pub config_valid: bool,
    pub claude_running: bool,
    pub config_path: String,
    pub exe_path: String,
}

#[tauri::command]
pub fn check_mcp_real_status() -> McpRealStatus {
    let bridge_online = ureq::get("http://127.0.0.1:3847/health").call().is_ok();

    let clients = scan_desktop_ai_clients();
    let claude_client = clients
        .iter()
        .find(|c| (c.id == "claude" || c.id == "claude_store") && c.connected)
        .or_else(|| {
            clients
                .iter()
                .find(|c| c.id == "claude" || c.id == "claude_store")
        });
    let config_path_str = claude_client
        .map(|c| c.config_path.clone())
        .unwrap_or_default();
    let config_path = std::path::PathBuf::from(&config_path_str);

    let mut exe_path_str = String::new();
    let mut config_valid = false;
    if config_path.exists() {
        if let Ok(raw) = std::fs::read_to_string(&config_path) {
            if let Ok(v) = serde_json::from_str::<Value>(&raw) {
                if let Some(cmd) = v["mcpServers"]["veltrax-studios"]["command"].as_str() {
                    exe_path_str = cmd.to_string();
                    config_valid = std::path::Path::new(cmd).exists();
                }
            }
        }
    }

    let claude_running = check_claude_process_running();

    McpRealStatus {
        bridge_online,
        config_valid,
        claude_running,
        config_path: config_path_str,
        exe_path: exe_path_str,
    }
}

#[cfg(target_os = "windows")]
fn check_claude_process_running() -> bool {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    std::process::Command::new("tasklist")
        .args(["/FI", "IMAGENAME eq Claude.exe", "/NH"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("Claude.exe"))
        .unwrap_or(false)
}

#[cfg(not(target_os = "windows"))]
fn check_claude_process_running() -> bool {
    false
}

// ── Internal HTTP Bridge (127.0.0.1:3847) ───────────────────────────────────

pub fn start_internal_server(state: SharedProjectState, app_handle: tauri::AppHandle) {
    std::thread::spawn(move || {
        let server = match tiny_http::Server::http("127.0.0.1:3847") {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[Veltrax Studios MCP] Server failed on port 3847: {}", e);
                return;
            }
        };
        eprintln!("[Veltrax Studios MCP] Listening on 127.0.0.1:3847");

        for mut request in server.incoming_requests() {
            let mut body = String::new();
            request.as_reader().read_to_string(&mut body).ok();

            let url = request.url().to_string();
            let response_v = handle_internal_request(&url, &body, &state, &app_handle);
            let response_s = serde_json::to_string(&response_v).unwrap_or_default();

            let resp = tiny_http::Response::from_string(response_s).with_header(
                tiny_http::Header::from_bytes(b"Content-Type", b"application/json").unwrap(),
            );
            let _ = request.respond(resp);
        }
    });
}

fn handle_mcp_project_tool(
    tool: &str,
    args: &Value,
    state: &SharedProjectState,
    app_handle: &tauri::AppHandle,
) -> Option<Value> {
    let result: Result<Value, String> = (|| match tool {
        "list_projects" => {
            let (projects, warning) = crate::storage::get_recent_projects()?;
            Ok(json!({"projects":projects,"warning":warning}))
        }
        "list_templates" => {
            let templates = crate::templates::list_all_templates()?;
            let list: Vec<Value> = templates.into_iter().map(|t| json!({
                    "id":t.id,"name":t.name,"category":t.category,"builtin":t.builtin,
                    "file_path":t.file_path,"canvas_width":t.canvas_width,"canvas_height":t.canvas_height,
                    "fields":t.fields
                })).collect();
            Ok(json!(list))
        }
        "get_brand_kit" => {
            let store = crate::brand::list_brand_kits()?;
            if let Some(id) = args["brand_kit_id"].as_str() {
                let kit = store
                    .kits
                    .iter()
                    .find(|k| k.id == id)
                    .ok_or_else(|| format!("Brand kit '{}' not found.", id))?;
                Ok(json!({"active_id":store.active_id,"kit":kit}))
            } else {
                Ok(json!(store))
            }
        }
        "list_library" => Ok(json!(crate::tools::get_user_library()?)),
        "list_fonts" => Ok(json!(crate::tools::list_imported_fonts()?)),
        "create_project" => {
            let width = args["width"]
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .ok_or("Missing or invalid 'width'")?;
            let height = args["height"]
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .ok_or("Missing or invalid 'height'")?;
            if width > 16_384 || height > 16_384 || width as u64 * height as u64 > 100_000_000 {
                return Err("Canvas must be at most 16384 px per side and 100 MP.".into());
            }
            let mut resp = crate::tools::create_project_from_state(
                state,
                width,
                height,
                args["transparent_bg"].as_bool().unwrap_or(false),
                args["project_name"].as_str().map(str::to_string),
            )?;
            if let Some(hex) = args["background_color"].as_str() {
                if !args["transparent_bg"].as_bool().unwrap_or(false) {
                    let mut s = state.lock().map_err(|e| e.to_string())?;
                    s.current.bg_color = parse_hex_color(hex);
                    s.dirty = true;
                    resp = render_and_respond(&mut s)?;
                    if let Some(path) = s.project_file_path.clone() {
                        let json =
                            serde_json::to_string_pretty(&s.current).map_err(|e| e.to_string())?;
                        crate::storage::atomic_write_str(std::path::Path::new(&path), &json)?;
                        s.dirty = false;
                        let _ = crate::storage::record_recent_project(&path, &s.current, None);
                    }
                }
            }
            app_handle
                .emit("mcp-project-created", &resp)
                .map_err(|e| e.to_string())?;
            Ok(serde_json::to_value(resp).map_err(|e| e.to_string())?)
        }
        "create_from_template" => {
            let template_id = args["template_id"]
                .as_str()
                .ok_or("Missing 'template_id'")?
                .to_string();
            let mut variables = HashMap::new();
            if let Some(object) = args["variables"].as_object() {
                for (k, v) in object {
                    let text = match v {
                        Value::String(s) => s.clone(),
                        Value::Null => String::new(),
                        _ => v.to_string(),
                    };
                    variables.insert(k.clone(), text);
                }
            }
            let resp = crate::templates::create_project_from_vtemplate_state(
                state,
                template_id,
                variables,
                args["project_name"].as_str().map(str::to_string),
                args["brand_kit_id"].as_str().map(str::to_string),
            )?;
            app_handle
                .emit("mcp-project-created", &resp)
                .map_err(|e| e.to_string())?;
            Ok(serde_json::to_value(resp).map_err(|e| e.to_string())?)
        }
        "export_image" => {
            let path = args["output_path"]
                .as_str()
                .ok_or("Missing 'output_path'")?;
            let format = args["format"]
                .as_str()
                .or_else(|| {
                    std::path::Path::new(path)
                        .extension()
                        .and_then(|s| s.to_str())
                })
                .unwrap_or("png");
            let quality = args["quality"].as_u64().unwrap_or(90);
            let quality = u8::try_from(quality).map_err(|_| "Invalid 'quality'")?;
            let path = crate::tools::export_image_from_state(state, path, format, quality)?;
            Ok(json!({"path":path,"format":format}))
        }
        "save_project" => {
            {
                let s = state.lock().map_err(|e| e.to_string())?;
                if !s.is_in_editor || !s.has_project {
                    return Err("Action blocked: open a project in the Editor first.".into());
                }
            }
            let resp = if let Some(path) = args["path"].as_str().filter(|p| !p.trim().is_empty()) {
                crate::tools::save_project_file_from_state(state, path)?
            } else {
                crate::tools::save_current_project_from_state(state)?
            };
            Ok(serde_json::to_value(resp).map_err(|e| e.to_string())?)
        }
        "save_as_template" => {
            {
                let s = state.lock().map_err(|e| e.to_string())?;
                if !s.is_in_editor || !s.has_project {
                    return Err("Action blocked: open a project in the Editor first.".into());
                }
            }
            let fields: Vec<crate::templates::TemplateField> =
                serde_json::from_value(args["fields"].clone())
                    .map_err(|e| format!("Invalid 'fields': {}", e))?;
            let summary = crate::templates::save_current_as_vtemplate_from_state(
                state,
                args["name"].as_str().ok_or("Missing 'name'")?.to_string(),
                args["category"].as_str().unwrap_or("Custom").to_string(),
                fields,
            )?;
            Ok(serde_json::to_value(summary).map_err(|e| e.to_string())?)
        }
        "apply_brand_kit" => {
            let mut s = state.lock().map_err(|e| e.to_string())?;
            if !s.is_in_editor || !s.has_project {
                return Err("Action blocked: open a project in the Editor first.".into());
            }
            let store = crate::brand::load_brand_store();
            let id = args["brand_kit_id"].as_str().unwrap_or(&store.active_id);
            let kit = store
                .kits
                .iter()
                .find(|k| k.id == id)
                .ok_or_else(|| format!("Brand kit '{}' not found.", id))?;
            s.push_undo();
            let primary_color = kit.colors.first().map(|c| parse_hex_color(&c.hex));
            let primary_font = kit.fonts.first().cloned();
            let primary_logo = kit.logos.first().map(|l| l.path.clone());
            for layer in &mut s.current.layers {
                match &mut layer.kind {
                    LayerKind::Shape { fill_color, .. } => {
                        if let Some(color) = primary_color {
                            *fill_color = color;
                        }
                    }
                    LayerKind::Text3D { font_style, .. } => {
                        if let Some(font) = &primary_font {
                            *font_style = font.clone();
                        }
                    }
                    LayerKind::Image { source_path, .. } => {
                        if let Some(path) = &primary_logo {
                            if std::path::Path::new(path).is_file() {
                                *source_path = path.clone();
                            }
                        }
                    }
                    _ => {}
                }
            }
            s.dirty = true;
            let resp = render_and_respond(&mut s)?;
            drop(s);
            app_handle
                .emit("mcp-project-updated", &resp)
                .map_err(|e| e.to_string())?;
            Ok(serde_json::to_value(resp).map_err(|e| e.to_string())?)
        }
        _ => return Ok(Value::Null),
    })();
    match result {
        Ok(v) if v.is_null() => None,
        Ok(v) => Some(json!({"success":true,"result":v})),
        Err(e)
            if tool == "create_project"
                || tool == "create_from_template"
                || tool == "apply_brand_kit"
                || tool == "save_project" =>
        {
            Some(json!({"error":e}))
        }
        Err(e) => Some(json!({"error":e})),
    }
}

fn blocked_response() -> Value {
    json!({
        "error": "blocked",
        "message": "Action blocked: Veltrax Studios is currently on the Dashboard. Ask the user to open or create a project in the Editor first."
    })
}

fn handle_internal_request(
    url: &str,
    body: &str,
    state: &SharedProjectState,
    app_handle: &tauri::AppHandle,
) -> Value {
    match url {
        "/health" => json!({
            "status": "ok",
            "version": env!("CARGO_PKG_VERSION"),
            "engine": "veltrax-studios"
        }),
        "/tool/inspect" => {
            let s = state.lock().unwrap();
            if !s.is_in_editor || !s.has_project || s.preview_path.is_empty() {
                return blocked_response();
            }
            let project = serde_json::to_value(&s.current).unwrap_or(json!({}));
            let layers_json = serde_json::to_value(&s.current.layers).unwrap_or(json!([]));
            let (w, h) = (s.current.canvas_width, s.current.canvas_height);
            let preview_path = s.preview_path.clone();
            let transparent = s.current.transparent_bg;
            let dirty = s.dirty;
            let project_file_path = s.project_file_path.clone();
            drop(s);

            match internal_get_preview_base64(&preview_path) {
                Ok((b64, _, _)) => json!({
                    "success": true,
                    "project": project,
                    "project_file_path": project_file_path,
                    "dirty": dirty,
                    "canvas_width": w,
                    "canvas_height": h,
                    "transparent_bg": transparent,
                    "layers": layers_json,
                    "image_base64": b64
                }),
                Err(e) => json!({ "error": e }),
            }
        }
        "/tool/execute" => {
            let params: Value = serde_json::from_str(body).unwrap_or(json!({}));
            let tool = params["tool"].as_str().unwrap_or("");
            if let Some(response) = handle_mcp_project_tool(tool, &params, state, app_handle) {
                return response;
            }

            let mut s = state.lock().unwrap();
            if !s.is_in_editor || !s.has_project {
                return blocked_response();
            }

            let exec_res = execute_mcp_layer_tool(&mut s, tool, &params);
            let resp_obj = match exec_res {
                Ok(resp) => resp,
                Err(e) => return json!({ "error": e }),
            };
            let preview_path = s.preview_path.clone();
            let (cw, ch) = (s.current.canvas_width, s.current.canvas_height);
            let layers_json = serde_json::to_value(&s.current.layers).unwrap_or(json!([]));
            drop(s);

            let _ = app_handle.emit("mcp-project-updated", resp_obj);

            match internal_get_preview_base64(&preview_path) {
                Ok((b64, _, _)) => json!({
                    "success": true,
                    "canvas_width": cw,
                    "canvas_height": ch,
                    "layers": layers_json,
                    "image_base64": b64
                }),
                Err(e) => json!({ "error": e }),
            }
        }
        _ => json!({ "error": "Not found" }),
    }
}

fn execute_mcp_layer_tool(
    s: &mut crate::layers::ProjectState,
    tool: &str,
    p: &Value,
) -> Result<crate::layers::ProjectResponse, String> {
    match tool {
        "add_image_overlay" => {
            let path = p["image_path"]
                .as_str()
                .ok_or("Missing 'image_path'")?
                .to_string();
            image::open(&path).map_err(|e| format!("Failed to open '{}': {}", path, e))?;
            let name = std::path::Path::new(&path)
                .file_stem()
                .and_then(|n| n.to_str())
                .unwrap_or("Overlay")
                .to_string();
            s.push_undo();
            let id = s.next_id();
            s.current.layers.push(Layer {
                id,
                name,
                visible: true,
                locked: false,
                opacity: 1.0,
                x: p["x"].as_i64().unwrap_or(40),
                y: p["y"].as_i64().unwrap_or(40),
                scale: p["scale"].as_f64().unwrap_or(1.0) as f32,
                rotation: 0.0,
                flip_h: false,
                flip_v: false,
                blend_mode: "normal".to_string(),
                bounds_width: 0,
                bounds_height: 0,
                group_id: None,
                effects: LayerEffects::default(),
                outline: LayerOutline::default(),
                shadow: DropShadow::default(),
                outer_glow: GlowEffect::default(),
                inner_glow: GlowEffect::default(),
                clip_to_below: false,
                mask_shape: None,
                mask_radius: 0,
                alpha_mask_path: None,
                kind: LayerKind::Image {
                    source_path: path,
                    bg_remove_color: None,
                    bg_remove_tolerance: 30,
                },
            });
            render_and_respond(s)
        }
        "remove_background_color" => {
            let lid = p["layer_id"].as_str().ok_or("Missing 'layer_id'")?;
            let hex = p["target_hex"].as_str().unwrap_or("#FFFFFF");
            let tol = p["tolerance"].as_u64().unwrap_or(35).min(255) as u8;
            s.push_undo();
            let target = parse_hex_color(hex);
            let layer = s
                .current
                .layers
                .iter_mut()
                .find(|l| l.id == lid)
                .ok_or_else(|| format!("Layer '{}' not found", lid))?;
            match &mut layer.kind {
                LayerKind::Image {
                    bg_remove_color,
                    bg_remove_tolerance,
                    ..
                } => {
                    *bg_remove_color = Some(target);
                    *bg_remove_tolerance = tol;
                }
                _ => return Err("Layer is not an Image layer".to_string()),
            }
            render_and_respond(s)
        }
        "add_3d_text" => {
            let content = p["content"]
                .as_str()
                .ok_or("Missing 'content'")?
                .to_string();
            let font_style = p["font_style"]
                .as_str()
                .unwrap_or("bold_display")
                .to_string();
            let size = p["size"].as_f64().unwrap_or(72.0) as f32;
            let color = parse_hex_color(p["color_hex"].as_str().unwrap_or("#FFFFFF"));
            let depth = p["depth_3d"].as_u64().unwrap_or(6) as u32;
            let stroke = parse_hex_color(p["stroke_hex"].as_str().unwrap_or("#000000"));
            let stroke_w = p["stroke_width"].as_u64().unwrap_or(3) as u32;
            let x = p["x"].as_i64().unwrap_or(60);
            let y = p["y"].as_i64().unwrap_or(60);
            s.push_undo();
            let id = s.next_id();
            let label: String = content.chars().take(18).collect();
            s.current.layers.push(Layer {
                id,
                name: format!("Text: {}", label),
                visible: true,
                locked: false,
                opacity: 1.0,
                x,
                y,
                scale: 1.0,
                rotation: 0.0,
                flip_h: false,
                flip_v: false,
                blend_mode: "normal".to_string(),
                bounds_width: 0,
                bounds_height: 0,
                group_id: None,
                effects: LayerEffects::default(),
                outline: LayerOutline::default(),
                shadow: DropShadow::default(),
                outer_glow: GlowEffect::default(),
                inner_glow: GlowEffect::default(),
                clip_to_below: false,
                mask_shape: None,
                mask_radius: 0,
                alpha_mask_path: None,
                kind: LayerKind::Text3D {
                    content,
                    font_style,
                    color,
                    depth,
                    shadow_color: [0, 0, 0, 180],
                    stroke_color: stroke,
                    stroke_width: stroke_w,
                    size,
                    bold: p["bold"].as_bool().unwrap_or(false),
                    italic: p["italic"].as_bool().unwrap_or(false),
                    align: p["align"].as_str().unwrap_or("left").to_string(),
                    line_height: p["line_height"].as_f64().unwrap_or(1.2) as f32,
                    letter_spacing: p["letter_spacing"].as_f64().unwrap_or(0.0) as f32,
                    underline: p["underline"].as_bool().unwrap_or(false),
                    gradient_colors: None,
                    gradient_angle: 90.0,
                    curve_amount: p["curve_amount"].as_f64().unwrap_or(0.0) as f32,
                },
            });
            render_and_respond(s)
        }
        "update_text_layer" => {
            let lid = p["layer_id"].as_str().ok_or("Missing 'layer_id'")?;
            let content = p["content"]
                .as_str()
                .ok_or("Missing 'content'")?
                .to_string();
            s.push_undo();
            let layer = s
                .current
                .layers
                .iter_mut()
                .find(|l| l.id == lid)
                .ok_or_else(|| format!("Layer '{}' not found", lid))?;
            let label: String = content.chars().take(18).collect();
            layer.name = format!("Text: {}", label);
            match &mut layer.kind {
                LayerKind::Text3D {
                    content: c,
                    font_style,
                    color,
                    depth,
                    stroke_color,
                    stroke_width,
                    size,
                    bold,
                    italic,
                    align,
                    line_height,
                    letter_spacing,
                    ..
                } => {
                    *c = content;
                    if let Some(fs) = p["font_style"].as_str() {
                        *font_style = fs.to_string();
                    }
                    if let Some(sz) = p["size"].as_f64() {
                        *size = sz as f32;
                    }
                    if let Some(ch) = p["color_hex"].as_str() {
                        *color = parse_hex_color(ch);
                    }
                    if let Some(d) = p["depth_3d"].as_u64() {
                        *depth = d as u32;
                    }
                    if let Some(sh) = p["stroke_hex"].as_str() {
                        *stroke_color = parse_hex_color(sh);
                    }
                    if let Some(sw) = p["stroke_width"].as_u64() {
                        *stroke_width = sw as u32;
                    }
                    if let Some(v) = p["bold"].as_bool() {
                        *bold = v;
                    }
                    if let Some(v) = p["italic"].as_bool() {
                        *italic = v;
                    }
                    if let Some(v) = p["align"].as_str() {
                        *align = v.to_string();
                    }
                    if let Some(v) = p["line_height"].as_f64() {
                        *line_height = (v as f32).clamp(0.7, 3.0);
                    }
                    if let Some(v) = p["letter_spacing"].as_f64() {
                        *letter_spacing = (v as f32).clamp(-20.0, 80.0);
                    }
                }
                _ => return Err("Layer is not a Text layer".to_string()),
            }
            render_and_respond(s)
        }
        "update_shape_layer" => {
            let lid = p["layer_id"].as_str().ok_or("Missing 'layer_id'")?;
            s.push_undo();
            let layer = s
                .current
                .layers
                .iter_mut()
                .find(|l| l.id == lid)
                .ok_or_else(|| format!("Layer '{}' not found", lid))?;
            match &mut layer.kind {
                LayerKind::Shape {
                    width,
                    height,
                    fill_color,
                    border_radius,
                    border_color,
                    border_width,
                    ..
                } => {
                    if let Some(w) = p["width"].as_u64() {
                        *width = (w as u32).max(4);
                    }
                    if let Some(h) = p["height"].as_u64() {
                        *height = (h as u32).max(4);
                    }
                    if let Some(fh) = p["fill_hex"].as_str() {
                        *fill_color = parse_hex_color(fh);
                    }
                    if let Some(br) = p["border_radius"].as_u64() {
                        *border_radius = br as u32;
                    }
                    if let Some(bh) = p["border_hex"].as_str() {
                        *border_color = parse_hex_color(bh);
                    }
                    if let Some(bw) = p["border_width"].as_u64() {
                        *border_width = bw as u32;
                    }
                }
                _ => return Err("Layer is not a Shape layer".to_string()),
            }
            render_and_respond(s)
        }
        "add_shape" => {
            let shape_type = p["shape_type"].as_str().unwrap_or("rectangle").to_string();
            let width = p["width"].as_u64().unwrap_or(300) as u32;
            let height = p["height"].as_u64().unwrap_or(160) as u32;
            let fill = parse_hex_color(p["fill_hex"].as_str().unwrap_or("#6366F1"));
            let radius = p["border_radius"].as_u64().unwrap_or(16) as u32;
            let border_color = p["border_hex"]
                .as_str()
                .map(parse_hex_color)
                .unwrap_or([0, 0, 0, 0]);
            let border_width = p["border_width"].as_u64().unwrap_or(0) as u32;
            let x = p["x"].as_i64().unwrap_or(80);
            let y = p["y"].as_i64().unwrap_or(80);
            s.push_undo();
            let id = s.next_id();
            s.current.layers.push(Layer {
                id,
                name: format!("Shape: {}", shape_type),
                visible: true,
                locked: false,
                opacity: 1.0,
                x,
                y,
                scale: 1.0,
                rotation: 0.0,
                flip_h: false,
                flip_v: false,
                blend_mode: "normal".to_string(),
                bounds_width: 0,
                bounds_height: 0,
                group_id: None,
                effects: LayerEffects::default(),
                outline: LayerOutline::default(),
                shadow: DropShadow::default(),
                outer_glow: GlowEffect::default(),
                inner_glow: GlowEffect::default(),
                clip_to_below: false,
                mask_shape: None,
                mask_radius: 0,
                alpha_mask_path: None,
                kind: LayerKind::Shape {
                    shape_type,
                    width: width.max(4),
                    height: height.max(4),
                    fill_color: fill,
                    border_radius: radius,
                    border_color,
                    border_width,
                    sides: 5,
                    corner_radii: None,
                    fill_gradient: None,
                },
            });
            render_and_respond(s)
        }
        "set_canvas_background" => {
            s.push_undo();
            if p["transparent"].as_bool() == Some(true) {
                s.current.transparent_bg = true;
                s.current.bg_gradient = None;
            } else if let (Some(fh), Some(th)) = (p["from_hex"].as_str(), p["to_hex"].as_str()) {
                s.current.transparent_bg = false;
                s.current.bg_gradient = Some(BgGradient {
                    from_color: parse_hex_color(fh),
                    to_color: parse_hex_color(th),
                    angle: p["angle"].as_f64().unwrap_or(135.0) as f32,
                });
            } else if let Some(hex) = p["color_hex"].as_str() {
                s.current.transparent_bg = false;
                s.current.bg_color = parse_hex_color(hex);
                s.current.bg_gradient = None;
            }
            render_and_respond(s)
        }
        "add_gradient_layer" => {
            let cw = s.current.canvas_width;
            let ch = s.current.canvas_height;
            let width = p["width"].as_u64().unwrap_or(cw as u64) as u32;
            let height = p["height"].as_u64().unwrap_or(ch as u64) as u32;
            let from_color = parse_hex_color(p["from_hex"].as_str().unwrap_or("#1a1d26"));
            let to_color = parse_hex_color(p["to_hex"].as_str().unwrap_or("#6366f1"));
            let angle = p["angle"].as_f64().unwrap_or(135.0) as f32;
            let x = p["x"].as_i64().unwrap_or(0);
            let y = p["y"].as_i64().unwrap_or(0);
            s.push_undo();
            let id = s.next_id();
            s.current.layers.push(Layer {
                id,
                name: "Gradient".to_string(),
                visible: true,
                locked: false,
                opacity: 1.0,
                x,
                y,
                scale: 1.0,
                rotation: 0.0,
                flip_h: false,
                flip_v: false,
                blend_mode: "normal".to_string(),
                bounds_width: 0,
                bounds_height: 0,
                group_id: None,
                effects: LayerEffects::default(),
                outline: LayerOutline::default(),
                shadow: DropShadow::default(),
                outer_glow: GlowEffect::default(),
                inner_glow: GlowEffect::default(),
                clip_to_below: false,
                mask_shape: None,
                mask_radius: 0,
                alpha_mask_path: None,
                kind: LayerKind::Gradient {
                    width: width.max(4),
                    height: height.max(4),
                    from_color,
                    to_color,
                    angle,
                },
            });
            render_and_respond(s)
        }
        "add_watermark_layer" => {
            let text = p["text"].as_str().ok_or("Missing 'text'")?.to_string();
            let opacity = (p["opacity"].as_f64().unwrap_or(0.4) as f32).clamp(0.05, 1.0);
            let position = p["position"].as_str().unwrap_or("bottom-right").to_string();
            let font_style = p["font_style"]
                .as_str()
                .unwrap_or("classic_serif")
                .to_string();
            let color = parse_hex_color(p["color_hex"].as_str().unwrap_or("#FFFFFF80"));
            let size = (p["size"].as_f64().unwrap_or(28.0) as f32).max(8.0);
            let cw = s.current.canvas_width;
            let ch = s.current.canvas_height;
            s.push_undo();
            let id = s.next_id();
            let label: String = text.chars().take(14).collect();
            s.current.layers.push(Layer {
                id,
                name: format!("Watermark: {}", label),
                visible: true,
                locked: false,
                opacity,
                x: 0,
                y: 0,
                scale: 1.0,
                rotation: 0.0,
                flip_h: false,
                flip_v: false,
                blend_mode: "normal".to_string(),
                bounds_width: 0,
                bounds_height: 0,
                group_id: None,
                effects: LayerEffects::default(),
                outline: LayerOutline::default(),
                shadow: DropShadow::default(),
                outer_glow: GlowEffect::default(),
                inner_glow: GlowEffect::default(),
                clip_to_below: false,
                mask_shape: None,
                mask_radius: 0,
                alpha_mask_path: None,
                kind: LayerKind::Watermark {
                    text,
                    font_style,
                    color,
                    size,
                    position,
                    opacity,
                    canvas_width: cw,
                    canvas_height: ch,
                },
            });
            render_and_respond(s)
        }
        "set_layer_outline" => {
            let lid = p["layer_id"].as_str().ok_or("Missing 'layer_id'")?;
            let enabled = p["enabled"].as_bool().unwrap_or(true);
            let color = parse_hex_color(p["color_hex"].as_str().unwrap_or("#FFFFFF"));
            let thickness = p["thickness"].as_u64().unwrap_or(6).min(24) as u32;
            s.push_undo();
            let layer = s
                .current
                .layers
                .iter_mut()
                .find(|l| l.id == lid)
                .ok_or_else(|| format!("Layer '{}' not found", lid))?;
            layer.outline = LayerOutline {
                enabled,
                color,
                thickness,
            };
            render_and_respond(s)
        }
        "update_layer_transform" => {
            let lid = p["layer_id"].as_str().ok_or("Missing 'layer_id'")?;
            s.push_undo();
            let layer = s
                .current
                .layers
                .iter_mut()
                .find(|l| l.id == lid)
                .ok_or_else(|| format!("Layer '{}' not found", lid))?;
            if let Some(x) = p["x"].as_i64() {
                layer.x = x;
            }
            if let Some(y) = p["y"].as_i64() {
                layer.y = y;
            }
            if let Some(sc) = p["scale"].as_f64() {
                layer.scale = (sc as f32).clamp(0.05, 8.0);
            }
            if let Some(rot) = p["rotation"].as_f64() {
                layer.rotation = rot as f32;
            }
            if let Some(op) = p["opacity"].as_f64() {
                layer.opacity = (op as f32).clamp(0.0, 1.0);
            }
            if let Some(bm) = p["blend_mode"].as_str() {
                layer.blend_mode = bm.to_string();
            }
            if let Some(fh) = p["flip_h"].as_bool() {
                layer.flip_h = fh;
            }
            if let Some(fv) = p["flip_v"].as_bool() {
                layer.flip_v = fv;
            }
            render_and_respond(s)
        }
        "update_layer_effects" => {
            let lid = p["layer_id"].as_str().ok_or("Missing 'layer_id'")?;
            s.push_undo();
            let layer = s
                .current
                .layers
                .iter_mut()
                .find(|l| l.id == lid)
                .ok_or_else(|| format!("Layer '{}' not found", lid))?;
            if let Some(b) = p["brightness"].as_i64() {
                layer.effects.brightness = b as i32;
            }
            if let Some(c) = p["contrast"].as_f64() {
                layer.effects.contrast = c as f32;
            }
            if let Some(sat) = p["saturation"].as_f64() {
                layer.effects.saturation = sat as f32;
            }
            if let Some(bl) = p["blur"].as_f64() {
                layer.effects.blur = (bl as f32).max(0.0);
            }
            if let Some(f) = p["filter"].as_str() {
                layer.effects.filter = f.to_string();
            }
            if let Some(v) = p["hue_shift"].as_f64() {
                layer.effects.hue_shift = (v as f32).clamp(-180.0, 180.0);
            }
            if let Some(v) = p["temperature"].as_f64() {
                layer.effects.temperature = (v as f32).clamp(-100.0, 100.0);
            }
            if let Some(v) = p["sharpness"].as_f64() {
                layer.effects.sharpness = (v as f32).clamp(0.0, 100.0);
            }
            if let Some(v) = p["vignette"].as_f64() {
                layer.effects.vignette = (v as f32).clamp(0.0, 100.0);
            }
            if let Some(v) = p["levels_black"].as_u64() {
                layer.effects.levels_black = v.min(250) as u8;
            }
            if let Some(v) = p["levels_white"].as_u64() {
                layer.effects.levels_white = v.clamp(5, 255) as u8;
            }
            if let Some(v) = p["levels_gamma"].as_f64() {
                layer.effects.levels_gamma = (v as f32).clamp(0.2, 3.0);
            }
            if let Some(v) = p["lut_preset"].as_str() {
                layer.effects.lut_preset = v.to_string();
            }
            render_and_respond(s)
        }
        "reorder_layer" => {
            let lid = p["layer_id"].as_str().ok_or("Missing 'layer_id'")?;
            let new_idx = p["new_index"].as_u64().ok_or("Missing 'new_index'")? as usize;
            let idx = s
                .current
                .layers
                .iter()
                .position(|l| l.id == lid)
                .ok_or_else(|| format!("Layer '{}' not found", lid))?;
            s.push_undo();
            let layer = s.current.layers.remove(idx);
            let target = new_idx.min(s.current.layers.len());
            s.current.layers.insert(target, layer);
            render_and_respond(s)
        }
        "delete_layer" => {
            let lid = p["layer_id"].as_str().ok_or("Missing 'layer_id'")?;
            let idx = s
                .current
                .layers
                .iter()
                .position(|l| l.id == lid)
                .ok_or_else(|| format!("Layer '{}' not found", lid))?;
            s.push_undo();
            s.current.layers.remove(idx);
            render_and_respond(s)
        }
        "resize_canvas" => {
            let width = p["width"].as_u64().ok_or("Missing 'width'")? as u32;
            let height = p["height"].as_u64().ok_or("Missing 'height'")? as u32;
            let scale_layers = p["scale_layers"].as_bool().unwrap_or(true);
            if width == 0 || height == 0 {
                return Err("Canvas width and height must be > 0".to_string());
            }
            s.push_undo();
            let old_w = s.current.canvas_width.max(1) as f32;
            let old_h = s.current.canvas_height.max(1) as f32;
            let sx = width as f32 / old_w;
            let sy = height as f32 / old_h;
            let uniform = sx.min(sy);
            s.current.canvas_width = width;
            s.current.canvas_height = height;
            if scale_layers {
                for l in &mut s.current.layers {
                    l.x = ((l.x as f32) * sx).round() as i64;
                    l.y = ((l.y as f32) * sy).round() as i64;
                    l.scale = (l.scale * uniform).clamp(0.05, 8.0);
                }
            }
            render_and_respond(s)
        }
        "crop_canvas" => {
            let x = p["x"].as_i64().unwrap_or(0);
            let y = p["y"].as_i64().unwrap_or(0);
            let width = p["width"].as_u64().ok_or("Missing 'width'")? as u32;
            let height = p["height"].as_u64().ok_or("Missing 'height'")? as u32;
            if width == 0 || height == 0 {
                return Err("Crop width and height must be > 0".to_string());
            }
            s.push_undo();
            s.current.canvas_width = width;
            s.current.canvas_height = height;
            for l in &mut s.current.layers {
                l.x -= x;
                l.y -= y;
            }
            render_and_respond(s)
        }
        "duplicate_layer" => {
            let lid = p["layer_id"].as_str().ok_or("Missing 'layer_id'")?;
            let idx = s
                .current
                .layers
                .iter()
                .position(|l| l.id == lid)
                .ok_or_else(|| format!("Layer '{}' not found", lid))?;
            s.push_undo();
            let mut copy = s.current.layers[idx].clone();
            copy.id = s.next_id();
            copy.name = format!("{} Copy", copy.name);
            copy.x += 20;
            copy.y += 20;
            s.current.layers.insert(idx + 1, copy);
            render_and_respond(s)
        }
        "group_layers" => {
            let ids: Vec<String> = p["layer_ids"]
                .as_array()
                .ok_or("Missing 'layer_ids'")?
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            if ids.is_empty() {
                return Err("layer_ids cannot be empty".to_string());
            }
            s.push_undo();
            let gid = format!("group-{}", s.next_layer_seq);
            s.next_layer_seq += 1;
            let name = p["name"]
                .as_str()
                .map(String::from)
                .unwrap_or_else(|| format!("Group {}", s.current.groups.len() + 1));
            s.current.groups.push(LayerGroup {
                id: gid.clone(),
                name,
                parent_group_id: p["parent_group_id"].as_str().map(String::from),
                visible: true,
                locked: false,
                collapsed: false,
            });
            for l in &mut s.current.layers {
                if ids.contains(&l.id) {
                    l.group_id = Some(gid.clone());
                }
            }
            render_and_respond(s)
        }
        "ungroup_layers" => {
            let gid = p["group_id"].as_str().ok_or("Missing 'group_id'")?;
            s.push_undo();
            for l in &mut s.current.layers {
                if l.group_id.as_deref() == Some(gid) {
                    l.group_id = None;
                }
            }
            s.current.groups.retain(|g| g.id != gid);
            render_and_respond(s)
        }
        "align_layers" => {
            let ids: Vec<String> = p["layer_ids"]
                .as_array()
                .ok_or("Missing 'layer_ids'")?
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            let alignment = p["alignment"].as_str().ok_or("Missing 'alignment'")?;
            if ids.is_empty() {
                return Err("layer_ids cannot be empty".to_string());
            }
            s.push_undo();
            let cw = s.current.canvas_width as f32;
            let ch = s.current.canvas_height as f32;
            let selected: Vec<(f32, f32, f32, f32)> = s
                .current
                .layers
                .iter()
                .filter(|l| ids.contains(&l.id))
                .map(|l| {
                    (
                        l.x as f32,
                        l.y as f32,
                        (l.bounds_width.max(1) as f32) * l.scale,
                        (l.bounds_height.max(1) as f32) * l.scale,
                    )
                })
                .collect();
            let (rl, rt, rr, rb) = if selected.len() <= 1 {
                (0.0, 0.0, cw, ch)
            } else {
                (
                    selected.iter().map(|i| i.0).fold(f32::MAX, f32::min),
                    selected.iter().map(|i| i.1).fold(f32::MAX, f32::min),
                    selected.iter().map(|i| i.0 + i.2).fold(f32::MIN, f32::max),
                    selected.iter().map(|i| i.1 + i.3).fold(f32::MIN, f32::max),
                )
            };
            for l in &mut s.current.layers {
                if !ids.contains(&l.id) || l.locked {
                    continue;
                }
                let w = (l.bounds_width.max(1) as f32) * l.scale;
                let h = (l.bounds_height.max(1) as f32) * l.scale;
                match alignment {
                    "left" => l.x = rl.round() as i64,
                    "center" => l.x = ((rl + rr) / 2.0 - w / 2.0).round() as i64,
                    "right" => l.x = (rr - w).round() as i64,
                    "top" => l.y = rt.round() as i64,
                    "middle" => l.y = ((rt + rb) / 2.0 - h / 2.0).round() as i64,
                    "bottom" => l.y = (rb - h).round() as i64,
                    _ => {}
                }
            }
            render_and_respond(s)
        }
        "undo" => {
            if let Some(prev) = s.undo_stack.pop() {
                let curr = std::mem::replace(&mut s.current, prev);
                s.redo_stack.push(curr);
            }
            render_and_respond(s)
        }
        "redo" => {
            if let Some(next) = s.redo_stack.pop() {
                let curr = std::mem::replace(&mut s.current, next);
                s.undo_stack.push(curr);
            }
            render_and_respond(s)
        }
        "add_drop_shadow" => {
            let lid = p["layer_id"].as_str().ok_or("Missing 'layer_id'")?;
            let enabled = p["enabled"].as_bool().unwrap_or(true);
            s.push_undo();
            let layer = s
                .current
                .layers
                .iter_mut()
                .find(|l| l.id == lid)
                .ok_or_else(|| format!("Layer '{}' not found.", lid))?;
            layer.shadow.enabled = enabled;
            if let Some(c) = p["color_hex"].as_str() {
                layer.shadow.color = parse_hex_color(c);
            }
            if let Some(v) = p["blur"].as_f64() {
                layer.shadow.blur = v as f32;
            }
            if let Some(v) = p["distance"].as_f64() {
                layer.shadow.distance = v as f32;
            }
            if let Some(v) = p["angle"].as_f64() {
                layer.shadow.angle = v as f32;
            }
            if let Some(v) = p["opacity"].as_u64() {
                layer.shadow.opacity = (v.min(100) as f32) / 100.0;
            }
            render_and_respond(s)
        }
        "add_glow" => {
            let lid = p["layer_id"].as_str().ok_or("Missing 'layer_id'")?;
            let kind = p["kind"].as_str().unwrap_or("outer");
            let enabled = p["enabled"].as_bool().unwrap_or(true);
            s.push_undo();
            let layer = s
                .current
                .layers
                .iter_mut()
                .find(|l| l.id == lid)
                .ok_or_else(|| format!("Layer '{}' not found.", lid))?;
            let glow = if kind == "inner" {
                &mut layer.inner_glow
            } else {
                &mut layer.outer_glow
            };
            glow.enabled = enabled;
            if let Some(c) = p["color_hex"].as_str() {
                glow.color = parse_hex_color(c);
            }
            if let Some(v) = p["intensity"].as_u64() {
                glow.intensity = (v as u32).clamp(1, 60) as f32;
            }
            render_and_respond(s)
        }
        "set_layer_mask" => {
            let lid = p["layer_id"].as_str().ok_or("Missing 'layer_id'")?;
            s.push_undo();
            let layer = s
                .current
                .layers
                .iter_mut()
                .find(|l| l.id == lid)
                .ok_or_else(|| format!("Layer '{}' not found.", lid))?;
            layer.mask_shape = p["mask_shape"]
                .as_str()
                .filter(|m| !m.is_empty() && *m != "none")
                .map(|m| m.to_string());
            if let Some(r) = p["mask_radius"].as_u64() {
                layer.mask_radius = r as u32;
            }
            if let Some(clip) = p["clip_to_below"].as_bool() {
                layer.clip_to_below = clip;
            }
            render_and_respond(s)
        }
        "update_text_advanced" => {
            let lid = p["layer_id"].as_str().ok_or("Missing 'layer_id'")?;
            s.push_undo();
            let layer = s
                .current
                .layers
                .iter_mut()
                .find(|l| l.id == lid)
                .ok_or_else(|| format!("Layer '{}' not found.", lid))?;
            match &mut layer.kind {
                LayerKind::Text3D {
                    underline,
                    gradient_colors,
                    gradient_angle,
                    curve_amount,
                    ..
                } => {
                    if let Some(u) = p["underline"].as_bool() {
                        *underline = u;
                    }
                    if let Some(ca) = p["curve_amount"].as_f64() {
                        *curve_amount = (ca as f32).clamp(-180.0, 180.0);
                    }
                    if let Some(ga) = p["gradient_angle"].as_f64() {
                        *gradient_angle = ga as f32;
                    }
                    if p["clear_gradient"].as_bool() == Some(true) {
                        *gradient_colors = None;
                    } else if p["gradient_from_hex"].is_string() || p["gradient_to_hex"].is_string()
                    {
                        let cur =
                            gradient_colors.unwrap_or([[255, 255, 255, 255], [56, 189, 248, 255]]);
                        let from = p["gradient_from_hex"]
                            .as_str()
                            .map(parse_hex_color)
                            .unwrap_or(cur[0]);
                        let to = p["gradient_to_hex"]
                            .as_str()
                            .map(parse_hex_color)
                            .unwrap_or(cur[1]);
                        *gradient_colors = Some([from, to]);
                    }
                }
                _ => return Err("Selected layer is not a Text layer.".to_string()),
            }
            render_and_respond(s)
        }
        "add_shape_advanced" => {
            let shape_type = p["shape_type"].as_str().unwrap_or("star").to_string();
            let width = p["width"].as_u64().unwrap_or(240).max(4) as u32;
            let height = p["height"].as_u64().unwrap_or(240).max(4) as u32;
            let fill_color = parse_hex_color(p["fill_hex"].as_str().unwrap_or("#6366F1"));
            let border_radius = p["border_radius"].as_u64().unwrap_or(0) as u32;
            let border_color = p["border_hex"]
                .as_str()
                .map(parse_hex_color)
                .unwrap_or([255, 255, 255, 255]);
            let border_width = p["border_width"].as_u64().unwrap_or(0) as u32;
            let sides = p["sides"].as_u64().unwrap_or(5).clamp(3, 12) as u32;
            let fill_gradient = match (
                p["fill_gradient_from"].as_str(),
                p["fill_gradient_to"].as_str(),
            ) {
                (Some(a), Some(b)) => Some(BgGradient {
                    from_color: parse_hex_color(a),
                    to_color: parse_hex_color(b),
                    angle: p["fill_gradient_angle"].as_f64().unwrap_or(45.0) as f32,
                }),
                _ => None,
            };
            s.push_undo();
            let cw = s.current.canvas_width as i64;
            let ch = s.current.canvas_height as i64;
            let lx = p["x"].as_i64().unwrap_or((cw - width as i64) / 2);
            let ly = p["y"].as_i64().unwrap_or((ch - height as i64) / 2);
            let id = format!(
                "layer_{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis()
            );
            s.current.layers.push(Layer {
                id,
                name: shape_type.clone(),
                visible: true,
                locked: false,
                x: lx,
                y: ly,
                scale: 1.0,
                rotation: 0.0,
                opacity: 1.0,
                blend_mode: "normal".into(),
                flip_h: false,
                flip_v: false,
                bounds_width: width,
                bounds_height: height,
                effects: LayerEffects::default(),
                outline: LayerOutline::default(),
                group_id: None,
                shadow: DropShadow::default(),
                outer_glow: GlowEffect::default(),
                inner_glow: GlowEffect::default(),
                clip_to_below: false,
                mask_shape: None,
                mask_radius: 0,
                alpha_mask_path: None,
                kind: LayerKind::Shape {
                    shape_type,
                    width,
                    height,
                    fill_color,
                    border_radius,
                    border_color,
                    border_width,
                    sides,
                    corner_radii: None,
                    fill_gradient,
                },
            });
            render_and_respond(s)
        }
        "import_font" => {
            let source_path = p["source_path"].as_str().ok_or("Missing 'source_path'")?;
            let _ = crate::tools::import_font(source_path.to_string())?;
            Ok(s.to_response())
        }
        "reset_alpha_mask" => {
            let lid = p["layer_id"].as_str().ok_or("Missing 'layer_id'")?;
            s.push_undo();
            let layer = s
                .current
                .layers
                .iter_mut()
                .find(|l| l.id == lid)
                .ok_or_else(|| format!("Layer '{}' not found.", lid))?;
            layer.alpha_mask_path = None;
            render_and_respond(s)
        }
        "trim_transparency" => {
            let rendered = crate::layers::render_project_composite(&mut s.current, "")?.to_rgba8();
            let (w, h) = (rendered.width(), rendered.height());
            let (mut min_x, mut min_y, mut max_x, mut max_y) = (w, h, 0u32, 0u32);
            for (x, y, px) in rendered.enumerate_pixels() {
                if px[3] > 0 {
                    if x < min_x {
                        min_x = x;
                    }
                    if y < min_y {
                        min_y = y;
                    }
                    if x > max_x {
                        max_x = x;
                    }
                    if y > max_y {
                        max_y = y;
                    }
                }
            }
            if min_x > max_x || min_y > max_y {
                return Err("Canvas is completely transparent.".to_string());
            }
            s.push_undo();
            s.current.canvas_width = (max_x - min_x + 1).max(1);
            s.current.canvas_height = (max_y - min_y + 1).max(1);
            for l in &mut s.current.layers {
                l.x -= min_x as i64;
                l.y -= min_y as i64;
            }
            render_and_respond(s)
        }
        "magic_wand_mask" => {
            let lid = p["layer_id"].as_str().ok_or("Missing 'layer_id'")?;
            let x = p["x"].as_u64().ok_or("Missing 'x'")? as u32;
            let y = p["y"].as_u64().ok_or("Missing 'y'")? as u32;
            let tol = p["tolerance"].as_u64().unwrap_or(32).min(255) as u8;
            let erase = p["erase"].as_bool().unwrap_or(true);
            let contiguous = p["contiguous"].as_bool().unwrap_or(true);
            s.push_undo();
            let layer = s
                .current
                .layers
                .iter_mut()
                .find(|l| l.id == lid)
                .ok_or_else(|| format!("Layer '{}' not found.", lid))?;
            let base_img = crate::layers::render_layer_kind(&layer.kind)?;
            let (w, h) = (base_img.width(), base_img.height());
            if x >= w || y >= h {
                return Err("Point outside layer bounds.".to_string());
            }
            let seed = base_img.get_pixel(x, y).0;
            let tol_sq = (tol as i32) * (tol as i32) * 3;
            let tval = if erase { 0u8 } else { 255u8 };
            let mut mask = if let Some(ref mp) = layer.alpha_mask_path {
                image::open(mp)
                    .map(|d| {
                        d.resize_exact(w, h, image::imageops::FilterType::Triangle)
                            .to_luma8()
                    })
                    .unwrap_or_else(|_| image::GrayImage::from_pixel(w, h, image::Luma([255u8])))
            } else {
                image::GrayImage::from_pixel(w, h, image::Luma([255u8]))
            };
            let matches = |px: [u8; 4]| -> bool {
                let dr = px[0] as i32 - seed[0] as i32;
                let dg = px[1] as i32 - seed[1] as i32;
                let db = px[2] as i32 - seed[2] as i32;
                dr * dr + dg * dg + db * db <= tol_sq
            };
            if contiguous {
                let mut vis = vec![false; (w * h) as usize];
                let mut q = std::collections::VecDeque::new();
                q.push_back((x, y));
                vis[(y * w + x) as usize] = true;
                while let Some((cx, cy)) = q.pop_front() {
                    mask.put_pixel(cx, cy, image::Luma([tval]));
                    for (nx, ny) in [
                        (cx.wrapping_sub(1), cy),
                        (cx + 1, cy),
                        (cx, cy.wrapping_sub(1)),
                        (cx, cy + 1),
                    ] {
                        if nx < w && ny < h {
                            let idx = (ny * w + nx) as usize;
                            if !vis[idx] && matches(base_img.get_pixel(nx, ny).0) {
                                vis[idx] = true;
                                q.push_back((nx, ny));
                            }
                        }
                    }
                }
            } else {
                for py in 0..h {
                    for px in 0..w {
                        if matches(base_img.get_pixel(px, py).0) {
                            mask.put_pixel(px, py, image::Luma([tval]));
                        }
                    }
                }
            }
            let appdata = appdata_dir();
            let dir = std::path::PathBuf::from(appdata)
                .join("Veltrax Studios")
                .join("masks");
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let dest = dir.join(format!(
                "{}_mcp_{}.png",
                lid,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis()
            ));
            image::DynamicImage::ImageLuma8(mask)
                .save(&dest)
                .map_err(|e| e.to_string())?;
            layer.alpha_mask_path = Some(dest.to_string_lossy().to_string());
            render_and_respond(s)
        }
        "lasso_mask" => {
            let lid = p["layer_id"].as_str().ok_or("Missing 'layer_id'")?;
            let pts_arr = p["points"].as_array().ok_or("Missing 'points'")?;
            if pts_arr.len() < 3 {
                return Err("Lasso requires at least 3 points.".to_string());
            }
            let verts: Vec<(f32, f32)> = pts_arr
                .iter()
                .filter_map(|v| {
                    let a = v.as_array()?;
                    Some((a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32))
                })
                .collect();
            let erase_inside = p["mode"].as_str().unwrap_or("erase_inside") == "erase_inside";
            s.push_undo();
            let layer = s
                .current
                .layers
                .iter_mut()
                .find(|l| l.id == lid)
                .ok_or_else(|| format!("Layer '{}' not found.", lid))?;
            let base_img = crate::layers::render_layer_kind(&layer.kind)?;
            let (w, h) = (base_img.width(), base_img.height());
            let mut mask = if let Some(ref mp) = layer.alpha_mask_path {
                image::open(mp)
                    .map(|d| {
                        d.resize_exact(w, h, image::imageops::FilterType::Triangle)
                            .to_luma8()
                    })
                    .unwrap_or_else(|_| image::GrayImage::from_pixel(w, h, image::Luma([255u8])))
            } else {
                image::GrayImage::from_pixel(w, h, image::Luma([255u8]))
            };
            for py in 0..h {
                for px in 0..w {
                    let inside =
                        crate::layers::point_in_polygon(px as f32 + 0.5, py as f32 + 0.5, &verts);
                    if (erase_inside && inside) || (!erase_inside && !inside) {
                        mask.put_pixel(px, py, image::Luma([0u8]));
                    }
                }
            }
            let appdata = appdata_dir();
            let dir = std::path::PathBuf::from(appdata)
                .join("Veltrax Studios")
                .join("masks");
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let dest = dir.join(format!(
                "{}_lasso_{}.png",
                lid,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis()
            ));
            image::DynamicImage::ImageLuma8(mask)
                .save(&dest)
                .map_err(|e| e.to_string())?;
            layer.alpha_mask_path = Some(dest.to_string_lossy().to_string());
            render_and_respond(s)
        }
        "add_smart_text" => {
            let tpl = p["template"].as_str().ok_or("Missing 'template'")?;
            let mut content = tpl.to_string();
            if let Some(vars) = p["vars"].as_object() {
                for (k, v) in vars {
                    if let Some(vs) = v.as_str() {
                        content = content.replace(&format!("{{{{{}}}}}", k), vs);
                    }
                }
            }
            let font_size = p["font_size"].as_u64().unwrap_or(60) as f32;
            let color = parse_hex_color(p["color"].as_str().unwrap_or("#FFFFFF"));
            s.push_undo();
            let (cx, cy) = (
                s.current.canvas_width as i64 / 2,
                s.current.canvas_height as i64 / 2,
            );
            let id = format!(
                "smart_text_{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis()
            );
            s.current.layers.push(Layer {
                id,
                name: "Smart Text".into(),
                visible: true,
                locked: false,
                opacity: 1.0,
                x: p["x"].as_i64().unwrap_or(cx - 200),
                y: p["y"].as_i64().unwrap_or(cy - 40),
                scale: 1.0,
                rotation: 0.0,
                flip_h: false,
                flip_v: false,
                blend_mode: "normal".into(),
                bounds_width: 400,
                bounds_height: 80,
                group_id: None,
                effects: Default::default(),
                outline: Default::default(),
                shadow: Default::default(),
                outer_glow: Default::default(),
                inner_glow: Default::default(),
                clip_to_below: false,
                mask_shape: None,
                mask_radius: 0,
                alpha_mask_path: None,
                kind: LayerKind::Text3D {
                    content,
                    font_style: "modern_sans".into(),
                    color,
                    depth: 0,
                    shadow_color: [0, 0, 0, 180],
                    stroke_color: [0, 0, 0, 255],
                    stroke_width: 0,
                    size: font_size,
                    bold: false,
                    italic: false,
                    align: "center".into(),
                    line_height: 1.2,
                    letter_spacing: 0.0,
                    underline: false,
                    gradient_colors: None,
                    gradient_angle: 0.0,
                    curve_amount: 0.0,
                },
            });
            render_and_respond(s)
        }
        "get_project_state" => {
            let snap_json = serde_json::to_value(&s.current).map_err(|e| e.to_string())?;
            let resp = s.to_response();
            // Return standard ProjectResponse while also logging/attaching state
            let _ = snap_json;
            Ok(resp)
        }
        other => Err(format!("Unknown MCP tool '{}'", other)),
    }
}

// ── MCP stdio Server (--mcp) ────────────────────────────────────────────────

fn mcp_result(result: Result<String, String>) -> Value {
    match result {
        Ok(text) => json!({ "content": [{ "type": "text", "text": text }] }),
        Err(text) => json!({ "content": [{ "type": "text", "text": text }], "isError": true }),
    }
}

fn mcp_render_template(args: &Value) -> Value {
    let result = (|| -> Result<String, String> {
        let template_ref = args["template"]
            .as_str()
            .filter(|value| !value.trim().is_empty())
            .ok_or("Missing 'template'")?;
        let output_path = args["output_file"]
            .as_str()
            .filter(|value| !value.trim().is_empty())
            .ok_or("Missing 'output_file'")?;
        let mut variables = std::collections::HashMap::new();
        if let Some(values) = args["variables"].as_object() {
            for (key, value) in values {
                let text = match value {
                    Value::String(text) => text.clone(),
                    Value::Null => String::new(),
                    other => other.to_string(),
                };
                variables.insert(key.clone(), text);
            }
        } else if !args["variables"].is_null() {
            return Err("'variables' must be an object.".to_string());
        }
        let size = match (args["width"].as_u64(), args["height"].as_u64()) {
            (None, None) => None,
            (Some(width), Some(height)) => {
                if width == 0
                    || height == 0
                    || width > 16_384
                    || height > 16_384
                    || width * height > 100_000_000
                {
                    return Err(
                        "Size must be between 1 and 16384 px per side and at most 100 MP."
                            .to_string(),
                    );
                }
                Some((width as u32, height as u32))
            }
            _ => return Err("'width' and 'height' must be supplied together.".to_string()),
        };
        let format = args["format"]
            .as_str()
            .or_else(|| {
                std::path::Path::new(output_path)
                    .extension()
                    .and_then(|e| e.to_str())
            })
            .unwrap_or("png");
        let quality = args["quality"].as_u64().unwrap_or(90);
        if !(1..=100).contains(&quality) {
            return Err("'quality' must be an integer from 1 to 100.".to_string());
        }
        let output = std::path::Path::new(output_path);
        if let Some(parent) = output.parent().filter(|path| !path.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("Could not create output directory: {}", error))?;
        }
        crate::batch::cli_render_template(
            template_ref,
            &variables,
            output,
            format,
            quality as u8,
            size,
        )?;
        Ok(format!("Rendered template to {}", output_path))
    })();
    mcp_result(result)
}

fn mcp_render_template_batch(args: &Value) -> Value {
    let result = (|| -> Result<String, String> {
        let template_ref = args["template"]
            .as_str()
            .filter(|value| !value.trim().is_empty())
            .ok_or("Missing 'template'")?;
        let data = args["data"]
            .as_str()
            .filter(|value| !value.trim().is_empty())
            .ok_or("Missing CSV/JSON 'data'")?;
        let output_dir = args["output_dir"]
            .as_str()
            .filter(|value| !value.trim().is_empty())
            .ok_or("Missing 'output_dir'")?;
        let rows = crate::batch::parse_batch_rows(data)?;
        let format = args["format"].as_str().unwrap_or("png");
        let quality = args["quality"].as_u64().unwrap_or(90);
        if !(1..=100).contains(&quality) {
            return Err("'quality' must be an integer from 1 to 100.".to_string());
        }
        let pattern = args["filename_pattern"].as_str().unwrap_or("{index}");
        let summary = crate::batch::cli_batch_render(
            template_ref,
            &rows,
            std::path::Path::new(output_dir),
            pattern,
            format,
            quality as u8,
        )?;
        let mut text = format!("Batch rendered {} image(s).", summary.saved.len());
        for path in summary.saved {
            text.push_str(&format!("\nSaved: {}", path));
        }
        if !summary.errors.is_empty() {
            for error in &summary.errors {
                text.push_str(&format!("\nError: {}", error));
            }
            return Err(text);
        }
        Ok(text)
    })();
    mcp_result(result)
}

pub fn run_mcp_stdio() {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let tools_desc = build_tools_description();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }
        let req: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let r = json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":format!("Parse error: {}",e)}});
                writeln!(out, "{}", r).ok();
                out.flush().ok();
                continue;
            }
        };

        let id = req.get("id").cloned().unwrap_or(json!(null));
        let method = req["method"].as_str().unwrap_or("");
        let params = req.get("params").cloned().unwrap_or(json!({}));

        let result: Option<Value> = match method {
            "initialize" => Some(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "veltrax-studios", "version": env!("CARGO_PKG_VERSION") }
            })),
            "notifications/initialized" => continue,
            "ping" => Some(json!({})),
            "tools/list" => Some(tools_desc.clone()),
            "tools/call" => {
                let tool_name = params["name"].as_str().unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                Some(match tool_name {
                    "render_template" => mcp_render_template(&args),
                    "render_template_batch" | "batch_render" => mcp_render_template_batch(&args),
                    "get_app_version" => mcp_result(Ok(crate::support::get_app_version())),
                    "get_feedback_diagnostics" => {
                        mcp_result(Ok(crate::support::get_feedback_diagnostics(
                            args["screen_width"]
                                .as_u64()
                                .and_then(|value| u32::try_from(value).ok()),
                            args["screen_height"]
                                .as_u64()
                                .and_then(|value| u32::try_from(value).ok()),
                        )))
                    }
                    "open_feedback_site" => mcp_result(crate::support::open_feedback_site()),
                    "record_local_error" => mcp_result(
                        crate::support::record_local_error(
                            args["source"].as_str().unwrap_or("").to_string(),
                            args["error_message"].as_str().unwrap_or("").to_string(),
                        )
                        .map(|()| "Error appended to local log.".to_string()),
                    ),
                    _ => forward_tool_call(tool_name, &args),
                })
            }
            _ => {
                let r = json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":format!("Method '{}' not found",method)}});
                writeln!(out, "{}", r).ok();
                out.flush().ok();
                continue;
            }
        };

        if let Some(res) = result {
            let r = json!({ "jsonrpc": "2.0", "id": id, "result": res });
            writeln!(out, "{}", r).ok();
            out.flush().ok();
        }
    }
}

fn build_tools_description() -> Value {
    let mut tools: Vec<Value> = serde_json::from_value(json!([
            {
                "name": "inspect_project",
                "description": "Inspect the active Veltrax Studios project: returns canvas dimensions, all layers with their IDs/positions, and a JPEG preview.",
                "inputSchema": { "type": "object", "properties": {}, "required": [] }
            },
            {
                "name": "add_image_overlay",
                "description": "Add a local PNG/image file as a new overlay layer.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "image_path": { "type": "string" },
                        "x": { "type": "integer" },
                        "y": { "type": "integer" },
                        "scale": { "type": "number" }
                    },
                    "required": ["image_path"]
                }
            },
            {
                "name": "remove_background_color",
                "description": "Remove a solid background color from an Image layer to create clean transparency.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "layer_id": { "type": "string" },
                        "target_hex": { "type": "string", "description": "Hex color e.g. #FFFFFF" },
                        "tolerance": { "type": "integer", "minimum": 0, "maximum": 255 }
                    },
                    "required": ["layer_id", "target_hex"]
                }
            },
            {
                "name": "add_3d_text",
                "description": "Add a high-impact 3D extruded text layer with stroke and shadow.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "content": { "type": "string" },
                        "font_style": { "type": "string", "enum": ["modern_sans","classic_serif","tech_mono","bold_display","script"] },
                        "size": { "type": "number" },
                        "color_hex": { "type": "string" },
                        "depth_3d": { "type": "integer", "minimum": 0, "maximum": 30 },
                        "stroke_hex": { "type": "string" },
                        "stroke_width": { "type": "integer", "minimum": 0, "maximum": 10 },
                        "x": { "type": "integer" },
                        "y": { "type": "integer" }
                    },
                    "required": ["content"]
                }
            },
            {
                "name": "update_text_layer",
                "description": "Edit an existing Text layer's content, font style, size, colors, stroke, or 3D depth.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "layer_id": { "type": "string" },
                        "content": { "type": "string" },
                        "font_style": { "type": "string", "enum": ["modern_sans","classic_serif","tech_mono","bold_display","script"] },
                        "size": { "type": "number" },
                        "color_hex": { "type": "string" },
                        "depth_3d": { "type": "integer", "minimum": 0, "maximum": 30 },
                        "stroke_hex": { "type": "string" },
                        "stroke_width": { "type": "integer", "minimum": 0, "maximum": 10 },
                        "bold": { "type": "boolean" },
                        "italic": { "type": "boolean" },
                        "align": { "type": "string", "enum": ["left","center","right"] },
                        "line_height": { "type": "number" },
                        "letter_spacing": { "type": "number" }
                    },
                    "required": ["layer_id", "content"]
                }
            },
            {
                "name": "update_shape_layer",
                "description": "Edit an existing Shape layer's size, fill, corner radius, or border.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "layer_id": { "type": "string" },
                        "width": { "type": "integer", "minimum": 4 },
                        "height": { "type": "integer", "minimum": 4 },
                        "fill_hex": { "type": "string" },
                        "border_radius": { "type": "integer", "minimum": 0 },
                        "border_hex": { "type": "string" },
                        "border_width": { "type": "integer", "minimum": 0 }
                    },
                    "required": ["layer_id"]
                }
            },
            {
                "name": "add_shape",
                "description": "Add a geometric shape layer (rectangle, circle, or badge) with optional border.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "shape_type": { "type": "string", "enum": ["rectangle","circle","badge"] },
                        "width": { "type": "integer", "minimum": 4 },
                        "height": { "type": "integer", "minimum": 4 },
                        "fill_hex": { "type": "string" },
                        "border_radius": { "type": "integer", "minimum": 0 },
                        "border_hex": { "type": "string" },
                        "border_width": { "type": "integer", "minimum": 0 },
                        "x": { "type": "integer" },
                        "y": { "type": "integer" }
                    },
                    "required": ["shape_type"]
                }
            },
            {
                "name": "set_canvas_background",
                "description": "Set the canvas background to a solid color, linear gradient, or transparent PNG.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "color_hex": { "type": "string" },
                        "from_hex": { "type": "string" },
                        "to_hex": { "type": "string" },
                        "angle": { "type": "number" },
                        "transparent": { "type": "boolean" }
                    }
                }
            },
            {
                "name": "add_gradient_layer",
                "description": "Add a linear gradient layer to the canvas.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "width": { "type": "integer", "minimum": 4 },
                        "height": { "type": "integer", "minimum": 4 },
                        "from_hex": { "type": "string" },
                        "to_hex": { "type": "string" },
                        "angle": { "type": "number" },
                        "x": { "type": "integer" },
                        "y": { "type": "integer" }
                    }
                }
            },
            {
                "name": "add_watermark_layer",
                "description": "Add a semi-transparent text watermark stamp at a corner or center.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "text": { "type": "string" },
                        "opacity": { "type": "number", "minimum": 0.05, "maximum": 1.0 },
                        "position": { "type": "string", "enum": ["top-left","top-right","bottom-left","bottom-right","center"] },
                        "size": { "type": "number" }
                    },
                    "required": ["text"]
                }
            },
            {
                "name": "set_layer_outline",
                "description": "Add or remove a glowing/solid outer border around any layer.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "layer_id": { "type": "string" },
                        "enabled": { "type": "boolean" },
                        "color_hex": { "type": "string" },
                        "thickness": { "type": "integer", "minimum": 1, "maximum": 24 }
                    },
                    "required": ["layer_id", "enabled"]
                }
            },
            {
                "name": "update_layer_transform",
                "description": "Update position, scale, rotation, opacity, or blend mode of a layer.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "layer_id": { "type": "string" },
                        "x": { "type": "integer" },
                        "y": { "type": "integer" },
                        "scale": { "type": "number" },
                        "rotation": { "type": "number" },
                        "opacity": { "type": "number", "minimum": 0, "maximum": 1 },
                        "blend_mode": { "type": "string", "enum": ["normal","multiply","screen","overlay"] },
                        "flip_h": { "type": "boolean" },
                        "flip_v": { "type": "boolean" }
                    },
                    "required": ["layer_id"]
                }
            }
    ])).unwrap_or_default();
    let more: Vec<Value> = serde_json::from_value(json!([
{
                "name": "update_layer_effects",
                "description": "Update non-destructive adjustments and filter for a layer. Use filter='original' to clear filters.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "layer_id": { "type": "string" },
                        "brightness": { "type": "integer", "minimum": -100, "maximum": 100 },
                        "contrast": { "type": "number", "minimum": -100, "maximum": 100 },
                        "saturation": { "type": "number", "minimum": -100, "maximum": 100 },
                        "blur": { "type": "number", "minimum": 0, "maximum": 20 },
                        "filter": { "type": "string", "enum": ["original","grayscale","noir","warm","cool","invert"] },
                        "hue_shift": { "type": "number", "minimum": -180, "maximum": 180 },
                        "temperature": { "type": "number", "minimum": -100, "maximum": 100 },
                        "sharpness": { "type": "number", "minimum": 0, "maximum": 100 },
                        "vignette": { "type": "number", "minimum": 0, "maximum": 100 },
                        "levels_black": { "type": "integer", "minimum": 0, "maximum": 250 },
                        "levels_white": { "type": "integer", "minimum": 5, "maximum": 255 },
                        "levels_gamma": { "type": "number", "minimum": 0.2, "maximum": 3.0 },
                        "lut_preset": { "type": "string", "enum": ["none","cinematic","cyberpunk","vintage","moody","pastel"] }
                    },
                    "required": ["layer_id"]
                }
            },
            {
                "name": "reorder_layer",
                "description": "Move a layer to a new z-order index (0 = bottom).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "layer_id": { "type": "string" },
                        "new_index": { "type": "integer", "minimum": 0 }
                    },
                    "required": ["layer_id", "new_index"]
                }
            },
            {
                "name": "delete_layer",
                "description": "Delete a layer by its ID.",
                "inputSchema": {
                    "type": "object",
                    "properties": { "layer_id": { "type": "string" } },
                    "required": ["layer_id"]
                }
            },
            {
                "name": "resize_canvas",
                "description": "Magic Resize: change canvas dimensions and optionally scale all layers proportionally.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "width": { "type": "integer", "minimum": 1 },
                        "height": { "type": "integer", "minimum": 1 },
                        "scale_layers": { "type": "boolean" }
                    },
                    "required": ["width", "height"]
                }
            },
            {
                "name": "crop_canvas",
                "description": "Crop the canvas to a specific rectangle and shift layers accordingly.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "x": { "type": "integer" },
                        "y": { "type": "integer" },
                        "width": { "type": "integer", "minimum": 1 },
                        "height": { "type": "integer", "minimum": 1 }
                    },
                    "required": ["width", "height"]
                }
            },
            {
                "name": "duplicate_layer",
                "description": "Duplicate an existing layer by its ID.",
                "inputSchema": {
                    "type": "object",
                    "properties": { "layer_id": { "type": "string" } },
                    "required": ["layer_id"]
                }
            },
            {
                "name": "group_layers",
                "description": "Group multiple layers into a named (optionally nested) group.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "layer_ids": { "type": "array", "items": { "type": "string" } },
                        "name": { "type": "string" },
                        "parent_group_id": { "type": "string" }
                    },
                    "required": ["layer_ids"]
                }
            },
            {
                "name": "ungroup_layers",
                "description": "Ungroup a group by its ID.",
                "inputSchema": {
                    "type": "object",
                    "properties": { "group_id": { "type": "string" } },
                    "required": ["group_id"]
                }
            },
            {
                "name": "align_layers",
                "description": "Align one or more layers (left, center, right, top, middle, bottom).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "layer_ids": { "type": "array", "items": { "type": "string" } },
                        "alignment": { "type": "string", "enum": ["left","center","right","top","middle","bottom"] }
                    },
                    "required": ["layer_ids", "alignment"]
                }
            },
            {
                "name": "add_drop_shadow",
                "description": "Enable or configure drop shadow on any layer.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "layer_id": { "type": "string" },
                        "enabled": { "type": "boolean" },
                        "color_hex": { "type": "string" },
                        "blur": { "type": "integer", "minimum": 0, "maximum": 40 },
                        "distance": { "type": "integer", "minimum": 0, "maximum": 100 },
                        "angle": { "type": "number" },
                        "opacity": { "type": "integer", "minimum": 0, "maximum": 100 }
                    },
                    "required": ["layer_id", "enabled"]
                }
            },
            {
                "name": "add_glow",
                "description": "Enable or configure outer or inner glow on any layer.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "layer_id": { "type": "string" },
                        "kind": { "type": "string", "enum": ["outer", "inner"] },
                        "enabled": { "type": "boolean" },
                        "color_hex": { "type": "string" },
                        "intensity": { "type": "integer", "minimum": 1, "maximum": 60 }
                    },
                    "required": ["layer_id", "kind", "enabled"]
                }
            },
            {
                "name": "set_layer_mask",
                "description": "Apply a shape mask (none, circle, rounded, star) or clip_to_below on a layer.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "layer_id": { "type": "string" },
                        "mask_shape": { "type": "string", "enum": ["none", "circle", "rounded", "star"] },
                        "mask_radius": { "type": "integer", "minimum": 0 },
                        "clip_to_below": { "type": "boolean" }
                    },
                    "required": ["layer_id"]
                }
            },
            {
                "name": "update_text_advanced",
                "description": "Configure underline, gradient fill, or arc curve on a Text3D layer.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "layer_id": { "type": "string" },
                        "underline": { "type": "boolean" },
                        "gradient_from_hex": { "type": "string" },
                        "gradient_to_hex": { "type": "string" },
                        "gradient_angle": { "type": "number" },
                        "clear_gradient": { "type": "boolean" },
                        "curve_amount": { "type": "number", "minimum": -180, "maximum": 180 }
                    },
                    "required": ["layer_id"]
                }
            },
            {
                "name": "add_shape_advanced",
                "description": "Add a shape layer (line, arrow, triangle, star, polygon, rectangle, circle, badge) with optional gradient fill.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "shape_type": { "type": "string", "enum": ["line","arrow","triangle","star","polygon","rectangle","circle","badge"] },
                        "width": { "type": "integer", "minimum": 4 },
                        "height": { "type": "integer", "minimum": 4 },
                        "fill_hex": { "type": "string" },
                        "border_radius": { "type": "integer", "minimum": 0 },
                        "border_hex": { "type": "string" },
                        "border_width": { "type": "integer", "minimum": 0 },
                        "sides": { "type": "integer", "minimum": 3, "maximum": 12 },
                        "fill_gradient_from": { "type": "string" },
                        "fill_gradient_to": { "type": "string" },
                        "fill_gradient_angle": { "type": "number" },
                        "x": { "type": "integer" },
                        "y": { "type": "integer" }
                    },
                    "required": ["shape_type", "width", "height", "fill_hex"]
                }
            },
            {
                "name": "import_font",
                "description": "Import a custom .ttf or .otf font file from disk into Veltrax Studios.",
                "inputSchema": {
                    "type": "object",
                    "properties": { "source_path": { "type": "string" } },
                    "required": ["source_path"]
                }
            },
            {
                "name": "magic_wand_mask",
                "description": "Erase or restore a region on a layer's alpha mask using a tolerance flood fill from (x, y).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "layer_id": { "type": "string" },
                        "x": { "type": "integer", "minimum": 0 },
                        "y": { "type": "integer", "minimum": 0 },
                        "tolerance": { "type": "integer", "minimum": 0, "maximum": 255 },
                        "erase": { "type": "boolean" },
                        "contiguous": { "type": "boolean" }
                    },
                    "required": ["layer_id", "x", "y"]
                }
            },
            {
                "name": "lasso_mask",
                "description": "Apply a polygon lasso selection to erase inside or keep inside on a layer's alpha mask.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "layer_id": { "type": "string" },
                        "points": { "type": "array", "items": { "type": "array", "items": { "type": "number" } } },
                        "mode": { "type": "string", "enum": ["erase_inside", "keep_inside"] }
                    },
                    "required": ["layer_id", "points", "mode"]
                }
            },
            {
                "name": "reset_alpha_mask",
                "description": "Clear the non-destructive alpha mask from a layer.",
                "inputSchema": {
                    "type": "object",
                    "properties": { "layer_id": { "type": "string" } },
                    "required": ["layer_id"]
                }
            },
            {
                "name": "trim_transparency",
                "description": "Automatically crop empty transparent borders around the canvas.",
                "inputSchema": { "type": "object", "properties": {}, "required": [] }
            },
            {
                "name": "undo",
                "description": "Undo the last layer action in Veltrax Studios.",
                "inputSchema": { "type": "object", "properties": {}, "required": [] }
            },
            {
                "name": "redo",
                "description": "Redo the previously undone layer action in Veltrax Studios.",
                "inputSchema": { "type": "object", "properties": {}, "required": [] }
            },
            {
                "name": "add_smart_text",
                "description": "Add a Smart Text layer with {{variable}} template substitution.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "template": { "type": "string" },
                        "vars": { "type": "object" },
                        "font_size": { "type": "integer" },
                        "color": { "type": "string" },
                        "x": { "type": "integer" },
                        "y": { "type": "integer" }
                    },
                    "required": ["template"]
                }
            },
            {
                "name": "get_project_state",
                "description": "Return the current Veltrax Studios project state including all layers, canvas dimensions, and effects.",
                "inputSchema": { "type": "object", "properties": {}, "required": [] }
            },
            {
                "name": "render_template",
                "description": "Render a .vtemplate by ID, name, or file path to one PNG, JPEG, or WebP image. This is distinct from add_smart_text, which edits a layer in the active project.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "template": { "type": "string" },
                        "variables": { "type": "object" },
                        "output_file": { "type": "string" },
                        "format": { "type": "string", "enum": ["png", "jpeg", "webp"] },
                        "quality": { "type": "integer", "minimum": 1, "maximum": 100 },
                        "width": { "type": "integer", "minimum": 1, "maximum": 16384 },
                        "height": { "type": "integer", "minimum": 1, "maximum": 16384 }
                    },
                    "required": ["template", "output_file"]
                }
            },
            {
                "name": "render_template_batch",
                "description": "Render a CSV (with header) or JSON array of rows using a .vtemplate and save one image per row. Existing output files are never overwritten.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "template": { "type": "string" },
                        "data": { "type": "string", "description": "CSV text with a header row or a JSON array of objects." },
                        "output_dir": { "type": "string" },
                        "filename_pattern": { "type": "string", "description": "Supports {index} and {column_name} placeholders." },
                        "format": { "type": "string", "enum": ["png", "jpeg", "webp"] },
                        "quality": { "type": "integer", "minimum": 1, "maximum": 100 }
                    },
                "required": ["template", "data", "output_dir"]
            },
            },
            {
                "name": "record_local_error",
                "description": "Append an explicitly provided frontend error to the local rotating log only. The message is truncated and is not transmitted or included verbatim in feedback diagnostics.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "source": { "type": "string", "enum": ["webview", "unhandled_promise", "frontend"] },
                        "error_message": { "type": "string", "maxLength": 2000 }
                    },
                    "required": ["source", "error_message"]
                }
            },
            {
                "name": "get_feedback_diagnostics",
                "description": "Return a privacy-filtered local diagnostic block (version, OS, optional screen resolution, and recent local error categories). No data is transmitted.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "screen_width": { "type": "integer", "minimum": 1 },
                        "screen_height": { "type": "integer", "minimum": 1 }
                    },
                    "required": []
                }
            },
            {
                "name": "open_feedback_site",
                "description": "Open the configured project feedback URL in the user's browser. Does not submit diagnostics or any other data.",
                "inputSchema": { "type": "object", "properties": {}, "required": [] }
            },
            {
                "name": "get_app_version",
                "description": "Return the running Veltrax Studios version from the Cargo package metadata.",
                "inputSchema": { "type": "object", "properties": {}, "required": [] }
            }
        ])).unwrap_or_default();
    tools.extend(more);
    let phase_g: Vec<Value> = serde_json::from_value(json!([
        {"name":"create_project","description":"Create a new blank .veltrax project (1–16384 px per side, maximum 100 MP). Available from Dashboard; the app switches to Editor after creation.","inputSchema":{"type":"object","properties":{"width":{"type":"integer","minimum":1,"maximum":16384},"height":{"type":"integer","minimum":1,"maximum":16384},"transparent_bg":{"type":"boolean","default":false},"project_name":{"type":"string"},"background_color":{"type":"string","description":"Optional hex background color; currently used only when transparent_bg is false."}},"required":["width","height"]}},
        {"name":"export_image","description":"Render the active project to an image file. Allowed from Dashboard when a project is loaded; writes atomically and refuses to overwrite an existing file.","inputSchema":{"type":"object","properties":{"output_path":{"type":"string"},"format":{"type":"string","enum":["png","jpeg","jpg","webp","svg","pdf","ico"]},"quality":{"type":"integer","minimum":1,"maximum":100}},"required":["output_path"]}},
        {"name":"save_project","description":"Save the active project in the existing .veltrax JSON format. Omit path to save to its current location; set path to Save As. Editor gate applies.","inputSchema":{"type":"object","properties":{"path":{"type":"string","description":"Optional destination path, normally ending in .veltrax."}},"required":[]}},
        {"name":"list_projects","description":"List recent Veltrax projects (paths, names, dimensions, timestamps, thumbnails and existence). Available from Dashboard.","inputSchema":{"type":"object","properties":{},"required":[]}},
        {"name":"list_templates","description":"List available .vtemplate templates with IDs, names, categories, dimensions and variable fields. Available from Dashboard.","inputSchema":{"type":"object","properties":{},"required":[]}},
        {"name":"create_from_template","description":"Create a new .veltrax project from a template, applying variable values and an optional brand kit. Available from Dashboard; the app switches to Editor after creation.","inputSchema":{"type":"object","properties":{"template_id":{"type":"string"},"variables":{"type":"object","additionalProperties":{"type":"string"}},"project_name":{"type":"string"},"brand_kit_id":{"type":"string"}},"required":["template_id"]}},
        {"name":"save_as_template","description":"Save the active project as a reusable .vtemplate with optional variable fields. Editor gate applies.","inputSchema":{"type":"object","properties":{"name":{"type":"string"},"category":{"type":"string"},"fields":{"type":"array","items":{"type":"object","properties":{"id":{"type":"string"},"label":{"type":"string"},"kind":{"type":"string","enum":["text","image","color"]},"layer_id":{"type":"string"},"variable_name":{"type":"string"},"default_value":{"type":"string"},"image_fit":{"type":"string","enum":["cover","contain"]}},"required":["id","label","kind","layer_id"]}}},"required":["name","category","fields"]}},
        {"name":"batch_render","description":"Batch-render rows of CSV or JSON data into images from a template; alias of render_template_batch and allowed from Dashboard.","inputSchema":{"type":"object","properties":{"template":{"type":"string"},"data":{"type":"string","description":"CSV text with headers or a JSON array of objects."},"output_dir":{"type":"string"},"filename_pattern":{"type":"string"},"format":{"type":"string","enum":["png","jpeg","webp"]},"quality":{"type":"integer","minimum":1,"maximum":100}},"required":["template","data","output_dir"]}},
        {"name":"get_brand_kit","description":"Get brand-kit profiles. Optionally select one profile by ID; otherwise returns the complete store. Available from Dashboard.","inputSchema":{"type":"object","properties":{"brand_kit_id":{"type":"string"}},"required":[]}},
        {"name":"apply_brand_kit","description":"Apply a saved brand kit's primary color to shape layers, its first font to text layers, and its first logo to image layers in the active project. Editor gate applies.","inputSchema":{"type":"object","properties":{"brand_kit_id":{"type":"string"}},"required":[]}},
        {"name":"list_library","description":"List images in the user's Veltrax library. Available from Dashboard.","inputSchema":{"type":"object","properties":{},"required":[]}},
        {"name":"list_fonts","description":"List imported .ttf and .otf fonts in the user's Veltrax font directory. Available from Dashboard.","inputSchema":{"type":"object","properties":{},"required":[]}}
    ])).unwrap_or_default();
    tools.extend(phase_g);
    json!({ "tools": tools })
}

fn forward_tool_call(tool_name: &str, args: &Value) -> Value {
    let (url, body) = if tool_name == "inspect_project" {
        ("http://127.0.0.1:3847/tool/inspect", json!({}))
    } else {
        let mut payload = args.clone();
        payload["tool"] = json!(tool_name);
        ("http://127.0.0.1:3847/tool/execute", payload)
    };

    let resp = match ureq::post(url).send_json(&body) {
        Ok(r) => match r.into_json::<Value>() {
            Ok(v) => v,
            Err(e) => return mcp_err(&e.to_string()),
        },
        Err(e) => {
            return mcp_err(&format!(
                "Could not reach Veltrax Studios GUI (is it running?): {}",
                e
            ))
        }
    };

    if let Some(err_msg) = resp.get("error").and_then(|e| e.as_str()) {
        let detail = resp
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or(err_msg);
        return json!({ "content": [{ "type": "text", "text": detail }], "isError": true });
    }

    if let Some(result) = resp.get("result") {
        let text = serde_json::to_string_pretty(result).unwrap_or_else(|_| result.to_string());
        return json!({ "content": [{ "type": "text", "text": text }] });
    }

    let cw = resp["canvas_width"].as_u64().unwrap_or(0);
    let ch = resp["canvas_height"].as_u64().unwrap_or(0);
    let layers = serde_json::to_string_pretty(&resp["layers"]).unwrap_or_else(|_| "[]".into());
    let metadata = resp.get("project").cloned().unwrap_or(Value::Null);
    let summary = format!(
        "Project properties: {}\nCanvas: {} × {} px\nLayers:\n{}",
        serde_json::to_string_pretty(&metadata).unwrap_or_default(),
        cw,
        ch,
        layers
    );

    let mut content = vec![json!({ "type": "text", "text": summary })];
    if let Some(b64) = resp["image_base64"].as_str() {
        content.push(json!({ "type": "image", "data": b64, "mimeType": "image/jpeg" }));
    }
    json!({ "content": content })
}

fn mcp_err(msg: &str) -> Value {
    json!({ "content": [{ "type": "text", "text": format!("Error: {}", msg) }], "isError": true })
}

#[cfg(test)]
mod phase_g_tests {
    use super::*;

    #[test]
    fn phase_g_tools_are_published_once_with_typed_schemas() {
        let tools = build_tools_description();
        let listed = tools["tools"].as_array().expect("tools array");
        let required = [
            "create_project",
            "export_image",
            "save_project",
            "list_projects",
            "list_templates",
            "create_from_template",
            "save_as_template",
            "batch_render",
            "get_brand_kit",
            "apply_brand_kit",
            "list_library",
            "list_fonts",
        ];
        for name in required {
            let matches: Vec<_> = listed
                .iter()
                .filter(|tool| tool["name"].as_str() == Some(name))
                .collect();
            assert_eq!(matches.len(), 1, "expected one MCP entry for {name}");
            assert_eq!(matches[0]["inputSchema"]["type"], "object");
            assert!(matches[0]["description"]
                .as_str()
                .is_some_and(|d| !d.is_empty()));
        }
        assert!(listed.iter().any(|tool| tool["name"] == "inspect_project"));
        assert!(listed
            .iter()
            .any(|tool| tool["name"] == "render_template_batch"));
    }
}
