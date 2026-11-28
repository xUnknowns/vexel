use crate::layers::{
    parse_hex_color, BgGradient, DropShadow, GlowEffect, Layer, LayerEffects, LayerGroup,
    LayerKind, LayerOutline, SharedProjectState,
};
use crate::tools::{internal_get_preview_base64, render_and_respond};
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use tauri::Emitter;

// ── Universal MCP Auto-Connector ────────────────────────────────────────────

#[derive(Clone, serde::Serialize)]
pub struct AiClientStatus {
    pub id: String,
    pub name: String,
    pub installed: bool,
    pub connected: bool,
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
    std::env::var("LOCALAPPDATA")
        .unwrap_or_else(|_| r"C:\Users\Public\AppData\Local".to_string())
}

fn userprofile_dir() -> String {
    std::env::var("USERPROFILE").unwrap_or_else(|_| r"C:\Users\Public".to_string())
}

fn json_has_veltrax(path: &std::path::Path) -> bool {
    if !path.is_file() {
        return false;
    }
    if let Ok(raw) = std::fs::read_to_string(path) {
        if let Ok(v) = serde_json::from_str::<Value>(&raw) {
            return v
                .get("mcpServers")
                .and_then(|s| s.get("veltrax-studios"))
                .is_some();
        }
    }
    false
}

fn toml_has_veltrax(path: &std::path::Path) -> bool {
    if !path.is_file() {
        return false;
    }
    std::fs::read_to_string(path)
        .map(|raw| raw.contains("[mcp_servers.veltrax-studios]"))
        .unwrap_or(false)
}

#[tauri::command]
pub fn scan_desktop_ai_clients() -> Vec<AiClientStatus> {
    let appdata = appdata_dir();
    let local_appdata = local_appdata_dir();
    let userprofile = userprofile_dir();

    let claude_dir = std::path::Path::new(&appdata).join("Claude");
    let claude_local = std::path::Path::new(&local_appdata).join("AnthropicClaude");
    let claude_cfg = claude_dir.join("claude_desktop_config.json");

    let lmstudio_dir = std::path::Path::new(&userprofile).join(".lmstudio");
    let lmstudio_cfg = lmstudio_dir.join("mcp.json");

    let codex_dir = std::path::Path::new(&userprofile).join(".codex");
    let codex_cfg = codex_dir.join("config.toml");

    let cursor_dir = std::path::Path::new(&userprofile).join(".cursor");
    let cursor_cfg = cursor_dir.join("mcp.json");

    vec![
        AiClientStatus {
            id: "claude".to_string(),
            name: "Claude Desktop".to_string(),
            installed: claude_dir.exists() || claude_local.exists(),
            connected: json_has_veltrax(&claude_cfg),
            config_path: claude_cfg.display().to_string(),
        },
        AiClientStatus {
            id: "lmstudio".to_string(),
            name: "LM Studio".to_string(),
            installed: lmstudio_dir.exists(),
            connected: json_has_veltrax(&lmstudio_cfg),
            config_path: lmstudio_cfg.display().to_string(),
        },
        AiClientStatus {
            id: "codex".to_string(),
            name: "ChatGPT / Codex".to_string(),
            installed: codex_dir.exists(),
            connected: toml_has_veltrax(&codex_cfg),
            config_path: codex_cfg.display().to_string(),
        },
        AiClientStatus {
            id: "cursor".to_string(),
            name: "Cursor".to_string(),
            installed: cursor_dir.exists(),
            connected: json_has_veltrax(&cursor_cfg),
            config_path: cursor_cfg.display().to_string(),
        },
    ]
}

fn write_json_mcp_entry(config_path: &std::path::Path, exe_str: &str) -> Result<(), String> {
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Failed to create directory: {}", e))?;
    }
    let mut config: Value = if config_path.exists() {
        let raw = std::fs::read_to_string(config_path).unwrap_or_default();
        serde_json::from_str(&raw).unwrap_or_else(|_| json!({}))
    } else {
        json!({})
    };
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
    let exe_path = std::env::current_exe()
        .map_err(|e| format!("Failed to get executable path: {}", e))?;
    let exe_str = exe_path.to_string_lossy().replace('\\', "/");

    let appdata = appdata_dir();
    let userprofile = userprofile_dir();

    match client_id.as_str() {
        "claude" => {
            let cfg = std::path::Path::new(&appdata)
                .join("Claude")
                .join("claude_desktop_config.json");
            write_json_mcp_entry(&cfg, &exe_str)?;
            Ok(format!("Connected to Claude Desktop ({})", cfg.display()))
        }
        "lmstudio" => {
            let cfg = std::path::Path::new(&userprofile)
                .join(".lmstudio")
                .join("mcp.json");
            write_json_mcp_entry(&cfg, &exe_str)?;
            Ok(format!("Connected to LM Studio ({})", cfg.display()))
        }
        "cursor" => {
            let cfg = std::path::Path::new(&userprofile)
                .join(".cursor")
                .join("mcp.json");
            write_json_mcp_entry(&cfg, &exe_str)?;
            Ok(format!("Connected to Cursor ({})", cfg.display()))
        }
        "codex" => {
            let dir = std::path::Path::new(&userprofile).join(".codex");
            std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create .codex dir: {}", e))?;
            let cfg = dir.join("config.toml");
            let mut raw = if cfg.exists() {
                std::fs::read_to_string(&cfg).unwrap_or_default()
            } else {
                String::new()
            };
            if !raw.contains("[mcp_servers.veltrax-studios]") {
                if !raw.is_empty() && !raw.ends_with('\n') {
                    raw.push('\n');
                }
                raw.push_str(&format!(
                    "\n[mcp_servers.veltrax-studios]\ncommand = '{}'\nargs = [\"--mcp\"]\n",
                    exe_str
                ));
                std::fs::write(&cfg, raw).map_err(|e| format!("Failed to write config.toml: {}", e))?;
            }
            Ok(format!("Connected to ChatGPT / Codex ({})", cfg.display()))
        }
        other => Err(format!("Unknown AI client '{}'", other)),
    }
}

#[tauri::command]
pub fn disconnect_ai_client(client_id: String) -> Result<String, String> {
    let appdata = appdata_dir();
    let userprofile = userprofile_dir();

    match client_id.as_str() {
        "claude" | "lmstudio" | "cursor" => {
            let cfg = match client_id.as_str() {
                "claude" => std::path::Path::new(&appdata).join("Claude").join("claude_desktop_config.json"),
                "lmstudio" => std::path::Path::new(&userprofile).join(".lmstudio").join("mcp.json"),
                _ => std::path::Path::new(&userprofile).join(".cursor").join("mcp.json"),
            };
            if cfg.is_file() {
                let raw = std::fs::read_to_string(&cfg).unwrap_or_default();
                if let Ok(mut v) = serde_json::from_str::<Value>(&raw) {
                    if let Some(obj) = v.get_mut("mcpServers").and_then(|s| s.as_object_mut()) {
                        obj.remove("veltrax-studios");
                        let pretty = serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?;
                        std::fs::write(&cfg, pretty).map_err(|e| e.to_string())?;
                    }
                }
            }
            Ok(format!("Disconnected {}", client_id))
        }
        "codex" => {
            let cfg = std::path::Path::new(&userprofile).join(".codex").join("config.toml");
            if cfg.is_file() {
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
    let exe_path = std::env::current_exe()
        .map_err(|e| format!("Failed to get executable path: {}", e))?;
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

    let appdata = std::env::var("APPDATA")
        .unwrap_or_else(|_| r"C:\Users\Public\AppData\Roaming".to_string());
    let config_path = std::path::Path::new(&appdata)
        .join("Claude")
        .join("claude_desktop_config.json");
    let config_path_str = config_path.display().to_string();

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
            "version": "1.0.0",
            "engine": "veltrax-studios"
        }),
        "/tool/inspect" => {
            let s = state.lock().unwrap();
            if !s.is_in_editor || !s.has_project || s.preview_path.is_empty() {
                return blocked_response();
            }
            let layers_json = serde_json::to_value(&s.current.layers).unwrap_or(json!([]));
            let (w, h) = (s.current.canvas_width, s.current.canvas_height);
            let preview_path = s.preview_path.clone();
            let transparent = s.current.transparent_bg;
            drop(s);

            match internal_get_preview_base64(&preview_path) {
                Ok((b64, _, _)) => json!({
                    "success": true,
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
            let content = p["content"].as_str().ok_or("Missing 'content'")?.to_string();
            let font_style = p["font_style"].as_str().unwrap_or("bold_display").to_string();
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
                },
            });
            render_and_respond(s)
        }
        "update_text_layer" => {
            let lid = p["layer_id"].as_str().ok_or("Missing 'layer_id'")?;
            let content = p["content"].as_str().ok_or("Missing 'content'")?.to_string();
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
                kind: LayerKind::Shape {
                    shape_type,
                    width: width.max(4),
                    height: height.max(4),
                    fill_color: fill,
                    border_radius: radius,
                    border_color,
                    border_width,
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
            let font_style = p["font_style"].as_str().unwrap_or("classic_serif").to_string();
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
            if let Some(v) = p["blur"].as_u64() {
                layer.shadow.blur = v as u32;
            }
            if let Some(v) = p["distance"].as_u64() {
                layer.shadow.distance = v as u32;
            }
            if let Some(v) = p["angle"].as_f64() {
                layer.shadow.angle = v as f32;
            }
            if let Some(v) = p["opacity"].as_u64() {
                layer.shadow.opacity = v.min(100) as u8;
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
                glow.intensity = (v as u32).clamp(1, 60);
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
                    } else if p["gradient_from_hex"].is_string() || p["gradient_to_hex"].is_string() {
                        let cur = gradient_colors.unwrap_or([[255, 255, 255, 255], [56, 189, 248, 255]]);
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
                    from: parse_hex_color(a),
                    to: parse_hex_color(b),
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
        other => Err(format!("Unknown MCP tool '{}'", other)),
    }
}

// ── MCP stdio Server (--mcp) ────────────────────────────────────────────────

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
                "serverInfo": { "name": "veltrax-studios", "version": "1.0.0" }
            })),
            "notifications/initialized" => continue,
            "ping" => Some(json!({})),
            "tools/list" => Some(tools_desc.clone()),
            "tools/call" => {
                let tool_name = params["name"].as_str().unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                Some(forward_tool_call(tool_name, &args))
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
    json!({
        "tools": [
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
            },
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
                        "filter": { "type": "string", "enum": ["original","grayscale","noir","warm","cool","invert"] }
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
                "name": "undo",
                "description": "Undo the last layer action in Veltrax Studios.",
                "inputSchema": { "type": "object", "properties": {}, "required": [] }
            },
            {
                "name": "redo",
                "description": "Redo the previously undone layer action in Veltrax Studios.",
                "inputSchema": { "type": "object", "properties": {}, "required": [] }
            }
        ]
    })
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

    let cw = resp["canvas_width"].as_u64().unwrap_or(0);
    let ch = resp["canvas_height"].as_u64().unwrap_or(0);
    let layers = serde_json::to_string_pretty(&resp["layers"]).unwrap_or_else(|_| "[]".into());
    let summary = format!("Canvas: {} × {} px\nLayers:\n{}", cw, ch, layers);

    let mut content = vec![json!({ "type": "text", "text": summary })];
    if let Some(b64) = resp["image_base64"].as_str() {
        content.push(json!({ "type": "image", "data": b64, "mimeType": "image/jpeg" }));
    }
    json!({ "content": content })
}

fn mcp_err(msg: &str) -> Value {
    json!({ "content": [{ "type": "text", "text": format!("Error: {}", msg) }], "isError": true })
}
