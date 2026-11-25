use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tauri::Emitter;

// ── Shared editor state ──────────────────────────────────────────────────

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct EditorState {
    pub is_in_editor: bool,
    pub active_image_path: Option<String>,
}

pub type SharedEditorState = Arc<Mutex<EditorState>>;

// ── Tauri commands ───────────────────────────────────────────────────────

#[tauri::command]
pub fn update_editor_state(
    state: tauri::State<'_, SharedEditorState>,
    is_in_editor: bool,
    active_image_path: Option<String>,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.is_in_editor = is_in_editor;
    s.active_image_path = active_image_path;
    Ok(())
}

#[derive(Serialize)]
pub struct EditorStateInfo {
    pub is_in_editor: bool,
    pub has_image: bool,
}

#[tauri::command]
pub fn get_editor_state_info(
    state: tauri::State<'_, SharedEditorState>,
) -> Result<EditorStateInfo, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    Ok(EditorStateInfo {
        is_in_editor: s.is_in_editor,
        has_image: s.active_image_path.is_some(),
    })
}

#[tauri::command]
pub fn connect_claude_desktop() -> Result<String, String> {
    let exe_path = std::env::current_exe()
        .map_err(|e| format!("Failed to get executable path: {}", e))?;

    let appdata = std::env::var("APPDATA")
        .unwrap_or_else(|_| r"C:\Users\Public\AppData\Roaming".to_string());

    let config_dir  = std::path::Path::new(&appdata).join("Claude");
    let config_path = config_dir.join("claude_desktop_config.json");

    std::fs::create_dir_all(&config_dir)
        .map_err(|e| format!("Failed to create Claude directory: {}", e))?;

    let mut config: Value = if config_path.exists() {
        let raw = std::fs::read_to_string(&config_path).unwrap_or_default();
        serde_json::from_str(&raw).unwrap_or(json!({}))
    } else {
        json!({})
    };

    if config.get("mcpServers").is_none() {
        config["mcpServers"] = json!({});
    }

    config["mcpServers"]["veltrax-studios"] = json!({
        "command": exe_path.to_string_lossy().replace('\\', '/'),
        "args":    ["--mcp"],
        "description": "Veltrax Studios AI-powered image editor by Crafters Apex"
    });

    let pretty = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    std::fs::write(&config_path, pretty)
        .map_err(|e| format!("Failed to write config: {}", e))?;

    Ok(format!(
        "Veltrax Studios successfully connected to Claude Desktop.\nConfig written to: {}",
        config_path.display()
    ))
}

// ── Internal HTTP server (runs inside the Tauri GUI process) ─────────────

pub fn start_internal_server(state: SharedEditorState, app_handle: tauri::AppHandle) {
    std::thread::spawn(move || {
        let server = match tiny_http::Server::http("127.0.0.1:3847") {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[Veltrax Studios MCP] HTTP server failed to start on port 3847: {}", e);
                return;
            }
        };
        eprintln!("[Veltrax Studios MCP] Internal HTTP server listening on 127.0.0.1:3847");

        for mut request in server.incoming_requests() {
            use std::io::Read;
            let mut body = String::new();
            request.as_reader().read_to_string(&mut body).ok();

            let url        = request.url().to_string();
            let response_v = handle_internal_request(&url, &body, &state, &app_handle);
            let response_s = serde_json::to_string(&response_v).unwrap_or_default();

            let resp = tiny_http::Response::from_string(response_s)
                .with_header(
                    tiny_http::Header::from_bytes(b"Content-Type", b"application/json").unwrap(),
                );
            let _ = request.respond(resp);
        }
    });
}

fn handle_internal_request(
    url:        &str,
    body:       &str,
    state:      &SharedEditorState,
    app_handle: &tauri::AppHandle,
) -> Value {
    let (is_in_editor, active_path) = {
        let s = state.lock().unwrap();
        (s.is_in_editor, s.active_image_path.clone())
    };

    match url {
        // ── State probe ──────────────────────────────────────────────
        "/state" => json!({
            "is_in_editor": is_in_editor,
            "has_image":    active_path.is_some(),
        }),

        // ── Inspect canvas ───────────────────────────────────────────
        "/tool/inspect" => {
            if !is_in_editor || active_path.is_none() {
                return blocked_response();
            }
            let path = active_path.unwrap();
            match crate::tools::internal_get_preview(&path) {
                Ok((b64, w, h)) => json!({ "success": true, "image_base64": b64, "width": w, "height": h }),
                Err(e)          => json!({ "error": e }),
            }
        }

        // ── Execute a tool ───────────────────────────────────────────
        "/tool/execute" => {
            if !is_in_editor || active_path.is_none() {
                return blocked_response();
            }
            let source = active_path.unwrap();
            let params: Value = serde_json::from_str(body).unwrap_or(json!({}));
            let tool   = params["tool"].as_str().unwrap_or("");

            let dir  = std::env::temp_dir();
            let dest = dir.join(format!(
                "veltrax-mcp-{}.png",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis()
            ));
            let dest_str = dest.to_string_lossy().to_string();

            let result = match tool {
                "apply_filter" => {
                    let f = params["filter"].as_str().unwrap_or("grayscale").to_string();
                    crate::tools::apply_filter(source.clone(), dest_str.clone(), f)
                }
                "adjust" => {
                    let b = params["brightness"].as_f64().unwrap_or(0.0) as i32;
                    let c = params["contrast"].as_f64().unwrap_or(0.0) as f32;
                    let s = params["saturation"].as_f64().unwrap_or(0.0) as f32;
                    crate::tools::adjust_brightness_contrast_saturation(source.clone(), dest_str.clone(), b, c, s)
                }
                "transform" => {
                    let a = params["action"].as_str().unwrap_or("rotate90").to_string();
                    crate::tools::transform_image(source.clone(), dest_str.clone(), a)
                }
                "crop" => {
                    let x = params["x"].as_u64().unwrap_or(0) as u32;
                    let y = params["y"].as_u64().unwrap_or(0) as u32;
                    let w = params["width"].as_u64().unwrap_or(100) as u32;
                    let h = params["height"].as_u64().unwrap_or(100) as u32;
                    crate::tools::crop_image(source.clone(), dest_str.clone(), x, y, w, h)
                }
                "resize" => {
                    let w = params["new_width"].as_u64().unwrap_or(800) as u32;
                    let h = params["new_height"].as_u64().unwrap_or(600) as u32;
                    crate::tools::resize_image(source.clone(), dest_str.clone(), w, h)
                }
                other => Err(format!("Unknown tool: '{}'", other)),
            };

            match result {
                Err(e) => json!({ "error": e }),
                Ok(new_path) => {
                    // Update active path in shared state
                    state.lock().unwrap().active_image_path = Some(new_path.clone());

                    // Notify Svelte to refresh the canvas
                    let _ = app_handle.emit("mcp-image-updated", new_path.clone());

                    // Return preview
                    match crate::tools::internal_get_preview(&new_path) {
                        Ok((b64, w, h)) => json!({
                            "success":      true,
                            "new_path":     new_path,
                            "image_base64": b64,
                            "width":        w,
                            "height":       h,
                        }),
                        Err(_) => json!({ "success": true, "new_path": new_path }),
                    }
                }
            }
        }

        _ => json!({ "error": "Not found" }),
    }
}

fn blocked_response() -> Value {
    json!({
        "error":   "blocked",
        "message": "Action blocked: Veltrax Studios is currently on the Dashboard or has no active image. Ask the user to open an image in the Editor first."
    })
}

// ── MCP stdio server (runs when launched with --mcp flag by Claude Desktop) ─

pub fn run_mcp_stdio() {
    use std::io::{BufRead, Write};

    let stdin     = std::io::stdin();
    let stdout    = std::io::stdout();
    let mut out   = stdout.lock();

    let tools_desc = build_tools_description();

    for line in stdin.lock().lines() {
        let line = match line { Ok(l) => l, Err(_) => break };
        if line.trim().is_empty() { continue; }

        let req: Value = match serde_json::from_str(&line) {
            Ok(v)  => v,
            Err(e) => {
                let r = json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":format!("Parse error: {}",e)}});
                writeln!(out, "{}", r).ok();
                continue;
            }
        };

        let id     = req.get("id").cloned().unwrap_or(json!(null));
        let method = req["method"].as_str().unwrap_or("");
        let params = req.get("params").cloned().unwrap_or(json!({}));

        let result: Option<Value> = match method {
            "initialize" => Some(json!({
                "protocolVersion": "2024-11-05",
                "capabilities":    { "tools": {} },
                "serverInfo":      { "name": "veltrax-studios", "version": "0.1.0" }
            })),
            "notifications/initialized" | "ping" => {
                if method == "ping" { Some(json!({})) } else { continue; }
            }
            "tools/list" => Some(tools_desc.clone()),
            "tools/call" => {
                let tool_name = params["name"].as_str().unwrap_or("");
                let args      = params.get("arguments").cloned().unwrap_or(json!({}));
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
    json!({ "tools": [
        {
            "name": "inspect_canvas",
            "description": "Get a visual preview (JPEG image) and pixel dimensions of the active canvas in Veltrax Studios. Call this first to see what you are working with.",
            "inputSchema": { "type": "object", "properties": {}, "required": [] }
        },
        {
            "name": "apply_filter",
            "description": "Apply a visual filter to the current image.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "filter": { "type": "string", "enum": ["grayscale","invert","blur","noir","warm","cool"], "description": "Name of the filter to apply." }
                },
                "required": ["filter"]
            }
        },
        {
            "name": "adjust_image",
            "description": "Adjust brightness, contrast and/or saturation. Values range from -100 to 100.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "brightness": { "type": "integer",  "minimum": -100, "maximum": 100 },
                    "contrast":   { "type": "number",   "minimum": -100, "maximum": 100 },
                    "saturation": { "type": "number",   "minimum": -100, "maximum": 100 }
                }
            }
        },
        {
            "name": "transform_image",
            "description": "Rotate or flip the current image.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["rotate90","rotate180","flip_h","flip_v"] }
                },
                "required": ["action"]
            }
        },
        {
            "name": "crop_image",
            "description": "Crop the image to a rectangular region (pixel coordinates).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "x":      { "type": "integer", "minimum": 0 },
                    "y":      { "type": "integer", "minimum": 0 },
                    "width":  { "type": "integer", "minimum": 1 },
                    "height": { "type": "integer", "minimum": 1 }
                },
                "required": ["x","y","width","height"]
            }
        },
        {
            "name": "resize_image",
            "description": "Resize the image to exact pixel dimensions using Lanczos3 resampling.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "new_width":  { "type": "integer", "minimum": 1 },
                    "new_height": { "type": "integer", "minimum": 1 }
                },
                "required": ["new_width","new_height"]
            }
        }
    ]})
}

fn forward_tool_call(tool_name: &str, args: &Value) -> Value {
    let (url, body) = match tool_name {
        "inspect_canvas" => (
            "http://127.0.0.1:3847/tool/inspect".to_string(),
            json!({}),
        ),
        "apply_filter" => (
            "http://127.0.0.1:3847/tool/execute".to_string(),
            json!({ "tool": "apply_filter", "filter": args["filter"] }),
        ),
        "adjust_image" => (
            "http://127.0.0.1:3847/tool/execute".to_string(),
            json!({ "tool": "adjust", "brightness": args.get("brightness").cloned().unwrap_or(json!(0)), "contrast": args.get("contrast").cloned().unwrap_or(json!(0)), "saturation": args.get("saturation").cloned().unwrap_or(json!(0)) }),
        ),
        "transform_image" => (
            "http://127.0.0.1:3847/tool/execute".to_string(),
            json!({ "tool": "transform", "action": args["action"] }),
        ),
        "crop_image" => (
            "http://127.0.0.1:3847/tool/execute".to_string(),
            json!({ "tool": "crop", "x": args["x"], "y": args["y"], "width": args["width"], "height": args["height"] }),
        ),
        "resize_image" => (
            "http://127.0.0.1:3847/tool/execute".to_string(),
            json!({ "tool": "resize", "new_width": args["new_width"], "new_height": args["new_height"] }),
        ),
        other => return mcp_err(&format!("Unknown tool: '{}'", other)),
    };

    let resp = match ureq::post(&url).send_json(&body) {
        Ok(r)  => match r.into_json::<Value>() { Ok(v) => v, Err(e) => return mcp_err(&e.to_string()) },
        Err(e) => return mcp_err(&format!("Could not reach Veltrax Studios GUI (is it running?): {}", e)),
    };

    if let Some(err_msg) = resp.get("error").and_then(|e| e.as_str()) {
        let detail = resp.get("message").and_then(|m| m.as_str()).unwrap_or(err_msg);
        return json!({ "content": [{ "type": "text", "text": detail }], "isError": true });
    }

    let w = resp["width"].as_u64().unwrap_or(0);
    let h = resp["height"].as_u64().unwrap_or(0);
    let desc = if w > 0 {
        format!("Done — canvas is now {} × {} px.", w, h)
    } else {
        format!("'{}' completed successfully.", tool_name)
    };

    let mut content = vec![json!({ "type": "text", "text": desc })];
    if let Some(b64) = resp["image_base64"].as_str() {
        content.push(json!({ "type": "image", "data": b64, "mimeType": "image/jpeg" }));
    }

    json!({ "content": content })
}

fn mcp_err(msg: &str) -> Value {
    json!({ "content": [{ "type": "text", "text": format!("Error: {}", msg) }], "isError": true })
}
