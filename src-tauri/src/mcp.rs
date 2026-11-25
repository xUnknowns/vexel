use crate::layers::{
    parse_hex_color, Layer, LayerEffects, LayerKind, LayerOutline, SharedProjectState,
};
use crate::tools::{internal_get_preview_base64, render_and_respond};
use serde_json::{json, Value};
use std::io::{BufRead, Read, Write};
use tauri::Emitter;

// ── Connect to Claude Desktop ───────────────────────────────────────────────

#[tauri::command]
pub fn connect_claude_desktop() -> Result<String, String> {
    let exe_path = std::env::current_exe()
        .map_err(|e| format!("Failed to get executable path: {}", e))?;

    let appdata = std::env::var("APPDATA")
        .unwrap_or_else(|_| r"C:\Users\Public\AppData\Roaming".to_string());

    let config_dir = std::path::Path::new(&appdata).join("Claude");
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
        "command": exe_path.to_string_lossy().replace('\\', "/"),
        "args": ["--mcp"],
        "description": "Veltrax Studios non-destructive layer editor by Crafters Apex"
    });

    let pretty = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    std::fs::write(&config_path, pretty)
        .map_err(|e| format!("Failed to write config: {}", e))?;

    Ok(format!(
        "Veltrax Studios connected to Claude Desktop.\nConfig written to: {}",
        config_path.display()
    ))
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
                blend_mode: "normal".to_string(),
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
                blend_mode: "normal".to_string(),
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
                },
            });
            render_and_respond(s)
        }
        "add_shape" => {
            let shape_type = p["shape_type"].as_str().unwrap_or("rectangle").to_string();
            let width = p["width"].as_u64().unwrap_or(300) as u32;
            let height = p["height"].as_u64().unwrap_or(160) as u32;
            let fill = parse_hex_color(p["fill_hex"].as_str().unwrap_or("#6366F1"));
            let radius = p["border_radius"].as_u64().unwrap_or(16) as u32;
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
                blend_mode: "normal".to_string(),
                effects: LayerEffects::default(),
                outline: LayerOutline::default(),
                kind: LayerKind::Shape {
                    shape_type,
                    width: width.max(4),
                    height: height.max(4),
                    fill_color: fill,
                    border_radius: radius,
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
        "undo" => {
            if let Some(prev) = s.undo_stack.pop() {
                let curr = std::mem::replace(&mut s.current, prev);
                s.redo_stack.push(curr);
            }
            render_and_respond(s)
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
                "serverInfo": { "name": "veltrax-studios", "version": "0.1.0" }
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
                "name": "add_shape",
                "description": "Add a geometric shape layer (rectangle, circle, or badge).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "shape_type": { "type": "string", "enum": ["rectangle","circle","badge"] },
                        "width": { "type": "integer", "minimum": 4 },
                        "height": { "type": "integer", "minimum": 4 },
                        "fill_hex": { "type": "string" },
                        "border_radius": { "type": "integer", "minimum": 0 },
                        "x": { "type": "integer" },
                        "y": { "type": "integer" }
                    },
                    "required": ["shape_type"]
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
                        "blend_mode": { "type": "string", "enum": ["normal","multiply","screen","overlay"] }
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
                "name": "undo",
                "description": "Undo the last layer action in Veltrax Studios.",
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
