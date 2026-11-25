use crate::layers::{
    parse_hex_color, render_project_composite, Layer, LayerEffects, LayerKind, LayerOutline,
    ProjectResponse, ProjectSnapshot, ProjectState, SharedProjectState,
};
use image::imageops::FilterType;
use std::path::Path;

fn next_preview_path() -> String {
    let dir = std::env::temp_dir();
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    dir.join(format!("veltrax-preview-{}.png", ms))
        .to_string_lossy()
        .to_string()
}

pub fn render_and_respond(state: &mut ProjectState) -> Result<ProjectResponse, String> {
    let dest = next_preview_path();
    render_project_composite(&state.current, &dest)?;
    state.preview_path = dest;
    Ok(state.to_response())
}

pub fn internal_get_preview_base64(source_path: &str) -> Result<(String, u32, u32), String> {
    let img = image::open(source_path)
        .map_err(|e| format!("Failed to open '{}': {}", source_path, e))?;
    let (w, h) = (img.width(), img.height());
    let preview = if w > 800 {
        let scale = 800.0 / w as f32;
        let new_h = ((h as f32 * scale) as u32).max(1);
        img.resize_exact(800, new_h, FilterType::Triangle)
    } else {
        img
    };
    let mut buf: Vec<u8> = Vec::new();
    {
        let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 75);
        enc.encode_image(&preview.to_rgb8())
            .map_err(|e| format!("Preview encode error: {}", e))?;
    }
    use base64::Engine;
    Ok((base64::engine::general_purpose::STANDARD.encode(&buf), w, h))
}

// ── Project & State Commands ────────────────────────────────────────────────

#[derive(serde::Serialize)]
pub struct Dimensions {
    pub width: u32,
    pub height: u32,
}

#[tauri::command]
pub fn get_image_dimensions(path: String) -> Result<Dimensions, String> {
    let img = image::open(&path).map_err(|e| format!("Failed to open '{}': {}", path, e))?;
    Ok(Dimensions {
        width: img.width(),
        height: img.height(),
    })
}

#[tauri::command]
pub fn set_editor_active(
    state: tauri::State<'_, SharedProjectState>,
    is_in_editor: bool,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.is_in_editor = is_in_editor;
    Ok(())
}

#[tauri::command]
pub fn get_project_state(
    state: tauri::State<'_, SharedProjectState>,
) -> Result<ProjectResponse, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    Ok(s.to_response())
}

#[tauri::command]
pub fn rename_project(
    state: tauri::State<'_, SharedProjectState>,
    name: String,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.current.project_name = name;
    Ok(())
}

#[tauri::command]
pub fn create_project(
    state: tauri::State<'_, SharedProjectState>,
    width: u32,
    height: u32,
    transparent_bg: bool,
    project_name: Option<String>,
) -> Result<ProjectResponse, String> {
    if width == 0 || height == 0 {
        return Err("Canvas width and height must be greater than zero.".to_string());
    }
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.undo_stack.clear();
    s.redo_stack.clear();
    s.has_project = true;
    s.is_in_editor = true;
    s.current = ProjectSnapshot {
        project_name: project_name.unwrap_or_else(|| "Untitled Design".to_string()),
        canvas_width: width,
        canvas_height: height,
        transparent_bg,
        bg_color: if transparent_bg {
            [0, 0, 0, 0]
        } else {
            [255, 255, 255, 255]
        },
        layers: Vec::new(),
    };
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn open_base_image(
    state: tauri::State<'_, SharedProjectState>,
    image_path: String,
) -> Result<ProjectResponse, String> {
    let img =
        image::open(&image_path).map_err(|e| format!("Failed to open '{}': {}", image_path, e))?;
    let (w, h) = (img.width(), img.height());
    let file_stem = Path::new(&image_path)
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("Untitled Design")
        .to_string();

    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.undo_stack.clear();
    s.redo_stack.clear();
    s.has_project = true;
    s.is_in_editor = true;
    let id = s.next_id();
    s.current = ProjectSnapshot {
        project_name: file_stem.clone(),
        canvas_width: w,
        canvas_height: h,
        transparent_bg: true,
        bg_color: [0, 0, 0, 0],
        layers: vec![Layer {
            id,
            name: file_stem,
            visible: true,
            locked: false,
            opacity: 1.0,
            x: 0,
            y: 0,
            scale: 1.0,
            rotation: 0.0,
            blend_mode: "normal".to_string(),
            effects: LayerEffects::default(),
            outline: LayerOutline::default(),
            kind: LayerKind::Image {
                source_path: image_path,
                bg_remove_color: None,
                bg_remove_tolerance: 30,
            },
        }],
    };
    render_and_respond(&mut s)
}

// ── Layer Creation & Manipulation ───────────────────────────────────────────

#[tauri::command]
pub fn add_image_overlay_layer(
    state: tauri::State<'_, SharedProjectState>,
    image_path: String,
    x: Option<i64>,
    y: Option<i64>,
    scale: Option<f32>,
) -> Result<ProjectResponse, String> {
    let _ =
        image::open(&image_path).map_err(|e| format!("Failed to open '{}': {}", image_path, e))?;
    let name = Path::new(&image_path)
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("Image Layer")
        .to_string();

    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let id = s.next_id();
    s.current.layers.push(Layer {
        id,
        name,
        visible: true,
        locked: false,
        opacity: 1.0,
        x: x.unwrap_or(40),
        y: y.unwrap_or(40),
        scale: scale.unwrap_or(1.0).max(0.05),
        rotation: 0.0,
        blend_mode: "normal".to_string(),
        effects: LayerEffects::default(),
        outline: LayerOutline::default(),
        kind: LayerKind::Image {
            source_path: image_path,
            bg_remove_color: None,
            bg_remove_tolerance: 30,
        },
    });
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn remove_layer_background_color(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    target_hex: String,
    tolerance: u8,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let target = parse_hex_color(&target_hex);
    let layer = s
        .current
        .layers
        .iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;

    match &mut layer.kind {
        LayerKind::Image {
            bg_remove_color,
            bg_remove_tolerance,
            ..
        } => {
            *bg_remove_color = Some(target);
            *bg_remove_tolerance = tolerance;
        }
        _ => return Err("Background removal only applies to Image layers.".to_string()),
    }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn add_text_layer(
    state: tauri::State<'_, SharedProjectState>,
    content: String,
    font_style: String,
    size: f32,
    color_hex: String,
    depth_3d: u32,
    stroke_hex: String,
    stroke_width: Option<u32>,
    x: i64,
    y: i64,
) -> Result<ProjectResponse, String> {
    if content.trim().is_empty() {
        return Err("Text content cannot be empty.".to_string());
    }
    let mut s = state.lock().map_err(|e| e.to_string())?;
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
            color: parse_hex_color(&color_hex),
            depth: depth_3d,
            shadow_color: [0, 0, 0, 180],
            stroke_color: parse_hex_color(&stroke_hex),
            stroke_width: stroke_width.unwrap_or(2),
            size: size.max(8.0),
        },
    });
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn add_shape_layer(
    state: tauri::State<'_, SharedProjectState>,
    shape_type: String,
    width: u32,
    height: u32,
    fill_hex: String,
    border_radius: u32,
    x: i64,
    y: i64,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let id = s.next_id();
    let title = match shape_type.as_str() {
        "circle" => "Circle",
        "badge" => "Badge",
        _ => "Rectangle",
    };
    s.current.layers.push(Layer {
        id,
        name: title.to_string(),
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
            fill_color: parse_hex_color(&fill_hex),
            border_radius,
        },
    });
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_transform(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    x: i64,
    y: i64,
    scale: f32,
    rotation: f32,
    opacity: f32,
    blend_mode: String,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let layer = s
        .current
        .layers
        .iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    if layer.locked {
        return Err("Layer is locked.".to_string());
    }
    layer.x = x;
    layer.y = y;
    layer.scale = scale.clamp(0.05, 8.0);
    layer.rotation = rotation;
    layer.opacity = opacity.clamp(0.0, 1.0);
    layer.blend_mode = blend_mode;
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_effects(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    brightness: i32,
    contrast: f32,
    saturation: f32,
    blur: f32,
    filter: String,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let layer = s
        .current
        .layers
        .iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.effects = LayerEffects {
        brightness,
        contrast,
        saturation,
        blur: blur.max(0.0),
        filter,
    };
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn set_layer_outline(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    color_hex: String,
    thickness: u32,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let layer = s
        .current
        .layers
        .iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.outline = LayerOutline {
        enabled,
        color: parse_hex_color(&color_hex),
        thickness: thickness.min(24),
    };
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn reorder_layer(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    new_index: usize,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let idx = s
        .current
        .layers
        .iter()
        .position(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    s.push_undo();
    let layer = s.current.layers.remove(idx);
    let target = new_index.min(s.current.layers.len());
    s.current.layers.insert(target, layer);
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn toggle_layer_visibility(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let layer = s
        .current
        .layers
        .iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.visible = !layer.visible;
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn toggle_layer_lock(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let layer = s
        .current
        .layers
        .iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.locked = !layer.locked;
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn duplicate_layer(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let idx = s
        .current
        .layers
        .iter()
        .position(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    s.push_undo();
    let mut copy = s.current.layers[idx].clone();
    copy.id = s.next_id();
    copy.name = format!("{} Copy", copy.name);
    copy.x += 20;
    copy.y += 20;
    s.current.layers.insert(idx + 1, copy);
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn delete_layer(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let idx = s
        .current
        .layers
        .iter()
        .position(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    s.push_undo();
    s.current.layers.remove(idx);
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn undo_action(
    state: tauri::State<'_, SharedProjectState>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(prev) = s.undo_stack.pop() {
        let curr = std::mem::replace(&mut s.current, prev);
        s.redo_stack.push(curr);
    }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn redo_action(
    state: tauri::State<'_, SharedProjectState>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(next) = s.redo_stack.pop() {
        let curr = std::mem::replace(&mut s.current, next);
        s.undo_stack.push(curr);
    }
    render_and_respond(&mut s)
}

// ── Export & Project Save/Load ──────────────────────────────────────────────

#[tauri::command]
pub fn export_configured(
    state: tauri::State<'_, SharedProjectState>,
    dest_path: String,
    format: String,
    quality: u8,
    scale: f32,
) -> Result<String, String> {
    let snapshot = {
        let s = state.lock().map_err(|e| e.to_string())?;
        if !s.has_project {
            return Err("No active project to export.".to_string());
        }
        s.current.clone()
    };

    let img = render_project_composite(&snapshot, "")?;
    let img = if (scale - 1.0).abs() > 0.01 {
        let nw = ((img.width() as f32 * scale) as u32).max(1);
        let nh = ((img.height() as f32 * scale) as u32).max(1);
        img.resize_exact(nw, nh, FilterType::Lanczos3)
    } else {
        img
    };

    let dest = Path::new(&dest_path);
    if let Some(parent) = dest.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }

    match format.as_str() {
        "jpeg" | "jpg" => {
            let file = std::fs::File::create(dest).map_err(|e| e.to_string())?;
            let mut enc =
                image::codecs::jpeg::JpegEncoder::new_with_quality(file, quality.clamp(1, 100));
            enc.encode_image(&img.to_rgb8()).map_err(|e| e.to_string())?;
        }
        _ => {
            img.save(dest).map_err(|e| e.to_string())?;
        }
    }
    Ok(dest_path)
}

#[tauri::command]
pub fn save_project_file(
    state: tauri::State<'_, SharedProjectState>,
    dest_path: String,
) -> Result<String, String> {
    let snapshot = {
        let s = state.lock().map_err(|e| e.to_string())?;
        s.current.clone()
    };
    let json = serde_json::to_string_pretty(&snapshot).map_err(|e| e.to_string())?;
    std::fs::write(&dest_path, json).map_err(|e| format!("Failed to save project: {}", e))?;
    Ok(dest_path)
}

#[tauri::command]
pub fn load_project_file(
    state: tauri::State<'_, SharedProjectState>,
    source_path: String,
) -> Result<ProjectResponse, String> {
    let raw = std::fs::read_to_string(&source_path)
        .map_err(|e| format!("Failed to read project '{}': {}", source_path, e))?;
    let snapshot: ProjectSnapshot =
        serde_json::from_str(&raw).map_err(|e| format!("Invalid .veltrax project: {}", e))?;

    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.undo_stack.clear();
    s.redo_stack.clear();
    s.has_project = true;
    s.is_in_editor = true;
    s.current = snapshot;
    render_and_respond(&mut s)
}
