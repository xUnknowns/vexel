use crate::layers::{
    parse_hex_color, render_project_composite, BgGradient, DropShadow, GlowEffect, Layer,
    LayerEffects, LayerGroup, LayerKind, LayerOutline, ProjectResponse, ProjectSnapshot,
    ProjectState, SharedProjectState,
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
    render_project_composite(&mut state.current, &dest)?;
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
        bg_gradient: None,
        layers: Vec::new(),
        groups: Vec::new(),
    };
    render_and_respond(&mut s)
}

// ── Phase 3: Font management ─────────────────────────────────────────────────

#[tauri::command]
pub fn import_font(source_path: String) -> Result<String, String> {
    let p = std::path::Path::new(&source_path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext != "ttf" && ext != "otf" {
        return Err("Only .ttf and .otf font files are supported.".into());
    }
    let filename = p.file_name().and_then(|n| n.to_str())
        .ok_or("Invalid file name")?
        .to_string();
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    std::fs::create_dir_all(&fonts_dir).map_err(|e| e.to_string())?;
    let dest = fonts_dir.join(&filename);
    std::fs::copy(&source_path, &dest).map_err(|e| format!("Copy failed: {}", e))?;
    Ok(filename)
}

#[tauri::command]
pub fn list_imported_fonts() -> Result<Vec<String>, String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    if !fonts_dir.exists() {
        return Ok(vec![]);
    }
    let mut names = vec![];
    for entry in std::fs::read_dir(&fonts_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let ext = std::path::Path::new(&name)
            .extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext == "ttf" || ext == "otf" {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

#[tauri::command]
pub fn delete_imported_font(filename: String) -> Result<(), String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    let path = fonts_dir.join(&filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Phase 3: Drop shadow / glow / mask / clip ────────────────────────────────

fn hex_to_rgba8(hex: &str) -> [u8; 4] {
    let h = hex.trim_start_matches('#');
    if h.len() == 6 {
        let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(0);
        [r, g, b, 255]
    } else {
        [0, 0, 0, 255]
    }
}

#[tauri::command]
pub fn update_layer_shadow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    color_hex: Option<String>,
    blur: Option<u32>,
    distance: Option<u32>,
    angle: Option<f32>,
    opacity: Option<u8>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.shadow.enabled = enabled;
    if let Some(c) = color_hex { layer.shadow.color = hex_to_rgba8(&c); }
    if let Some(v) = blur { layer.shadow.blur = v; }
    if let Some(v) = distance { layer.shadow.distance = v; }
    if let Some(v) = angle { layer.shadow.angle = v; }
    if let Some(v) = opacity { layer.shadow.opacity = v; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_glow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    kind: String, // "outer" | "inner"
    enabled: bool,
    color_hex: Option<String>,
    intensity: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    let glow = if kind == "inner" { &mut layer.inner_glow } else { &mut layer.outer_glow };
    glow.enabled = enabled;
    if let Some(c) = color_hex { glow.color = hex_to_rgba8(&c); }
    if let Some(v) = intensity { glow.intensity = v.clamp(1, 60); }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_mask(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    mask_shape: Option<String>, // None | "circle" | "rounded" | "star"
    mask_radius: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.mask_shape = mask_shape;
    if let Some(r) = mask_radius { layer.mask_radius = r; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn set_clip_to_below(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.clip_to_below = enabled;
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced text ───────────────────────────────────────────────────

#[tauri::command]
pub fn update_text_advanced(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    underline: Option<bool>,
    gradient_from_hex: Option<String>,
    gradient_to_hex: Option<String>,
    gradient_angle: Option<f32>,
    curve_amount: Option<f32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    match &mut layer.kind {
        LayerKind::Text3D {
            underline: ul,
            gradient_colors: gc,
            gradient_angle: ga,
            curve_amount: ca,
            ..
        } => {
            if let Some(v) = underline { *ul = v; }
            if let Some(v) = curve_amount { *ca = v.clamp(-180.0, 180.0); }
            if let Some(v) = gradient_angle { *ga = v; }
            // If either gradient color given, build/update gradient_colors pair
            if gradient_from_hex.is_some() || gradient_to_hex.is_some() {
                let cur = gc.unwrap_or([[255,255,255,255],[255,255,0,255]]);
                let from = gradient_from_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[0]);
                let to   = gradient_to_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[1]);
                *gc = Some([from, to]);
            } else if gradient_from_hex.is_none() && gradient_to_hex.is_none()
                && underline.is_none() && curve_amount.is_none() && gradient_angle.is_none()
            {
                // Clear gradient
                *gc = None;
            }
        }
        _ => return Err("Layer is not a Text layer.".into()),
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced shapes ─────────────────────────────────────────────────

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn add_shape_advanced(
    state: tauri::State<'_, SharedProjectState>,
    shape_type: String,
    width: u32,
    height: u32,
    fill_hex: String,
    border_radius: Option<u32>,
    border_hex: Option<String>,
    border_width: Option<u32>,
    sides: Option<u32>,
    fill_gradient_from: Option<String>,
    fill_gradient_to: Option<String>,
    fill_gradient_angle: Option<f32>,
    x: Option<i64>,
    y: Option<i64>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let cw = s.current.canvas_width as i64;
    let ch = s.current.canvas_height as i64;
    let lx = x.unwrap_or((cw - width as i64) / 2);
    let ly = y.unwrap_or((ch - height as i64) / 2);
    let fill_color = parse_hex_color(&fill_hex);
    let b_color = border_hex.as_deref().map(parse_hex_color).unwrap_or([255,255,255,255]);
    let fill_gradient = match (fill_gradient_from.as_deref(), fill_gradient_to.as_deref()) {
        (Some(a), Some(b)) => Some(BgGradient {
            from: hex_to_rgba8(a),
            to: hex_to_rgba8(b),
            angle: fill_gradient_angle.unwrap_or(0.0),
        }),
        _ => None,
    };
    let id = format!("layer_{}", uuid_v4());
    let name = match shape_type.as_str() {
        "line"     => "Line",
        "arrow"    => "Arrow",
        "triangle" => "Triangle",
        "star"     => "Star",
        "polygon"  => "Polygon",
        "circle"   => "Circle",
        "badge"    => "Badge",
        _          => "Rectangle",
    }.to_string();
    let layer = Layer {
        id: id.clone(),
        name,
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
            shape_type: shape_type.clone(),
            width,
            height,
            fill_color,
            border_radius: border_radius.unwrap_or(0),
            border_color: b_color,
            border_width: border_width.unwrap_or(0),
            sides: sides.unwrap_or(5),
            corner_radii: None,
            fill_gradient,
        },
    };
    s.current.layers.push(layer);
    render_and_respond(&mut s)
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_micros();
    format!("{:x}", ms ^ (ms >> 16))
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
        bg_gradient: None,
        groups: Vec::new(),
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
            flip_h: false,
            flip_v: false,
            blend_mode: "normal".to_string(),
            bounds_width: w,
            bounds_height: h,
            group_id: None,
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

// ── Phase 3: Font management ─────────────────────────────────────────────────

#[tauri::command]
pub fn import_font(source_path: String) -> Result<String, String> {
    let p = std::path::Path::new(&source_path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext != "ttf" && ext != "otf" {
        return Err("Only .ttf and .otf font files are supported.".into());
    }
    let filename = p.file_name().and_then(|n| n.to_str())
        .ok_or("Invalid file name")?
        .to_string();
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    std::fs::create_dir_all(&fonts_dir).map_err(|e| e.to_string())?;
    let dest = fonts_dir.join(&filename);
    std::fs::copy(&source_path, &dest).map_err(|e| format!("Copy failed: {}", e))?;
    Ok(filename)
}

#[tauri::command]
pub fn list_imported_fonts() -> Result<Vec<String>, String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    if !fonts_dir.exists() {
        return Ok(vec![]);
    }
    let mut names = vec![];
    for entry in std::fs::read_dir(&fonts_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let ext = std::path::Path::new(&name)
            .extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext == "ttf" || ext == "otf" {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

#[tauri::command]
pub fn delete_imported_font(filename: String) -> Result<(), String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    let path = fonts_dir.join(&filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Phase 3: Drop shadow / glow / mask / clip ────────────────────────────────

fn hex_to_rgba8(hex: &str) -> [u8; 4] {
    let h = hex.trim_start_matches('#');
    if h.len() == 6 {
        let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(0);
        [r, g, b, 255]
    } else {
        [0, 0, 0, 255]
    }
}

#[tauri::command]
pub fn update_layer_shadow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    color_hex: Option<String>,
    blur: Option<u32>,
    distance: Option<u32>,
    angle: Option<f32>,
    opacity: Option<u8>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.shadow.enabled = enabled;
    if let Some(c) = color_hex { layer.shadow.color = hex_to_rgba8(&c); }
    if let Some(v) = blur { layer.shadow.blur = v; }
    if let Some(v) = distance { layer.shadow.distance = v; }
    if let Some(v) = angle { layer.shadow.angle = v; }
    if let Some(v) = opacity { layer.shadow.opacity = v; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_glow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    kind: String, // "outer" | "inner"
    enabled: bool,
    color_hex: Option<String>,
    intensity: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    let glow = if kind == "inner" { &mut layer.inner_glow } else { &mut layer.outer_glow };
    glow.enabled = enabled;
    if let Some(c) = color_hex { glow.color = hex_to_rgba8(&c); }
    if let Some(v) = intensity { glow.intensity = v.clamp(1, 60); }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_mask(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    mask_shape: Option<String>, // None | "circle" | "rounded" | "star"
    mask_radius: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.mask_shape = mask_shape;
    if let Some(r) = mask_radius { layer.mask_radius = r; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn set_clip_to_below(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.clip_to_below = enabled;
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced text ───────────────────────────────────────────────────

#[tauri::command]
pub fn update_text_advanced(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    underline: Option<bool>,
    gradient_from_hex: Option<String>,
    gradient_to_hex: Option<String>,
    gradient_angle: Option<f32>,
    curve_amount: Option<f32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    match &mut layer.kind {
        LayerKind::Text3D {
            underline: ul,
            gradient_colors: gc,
            gradient_angle: ga,
            curve_amount: ca,
            ..
        } => {
            if let Some(v) = underline { *ul = v; }
            if let Some(v) = curve_amount { *ca = v.clamp(-180.0, 180.0); }
            if let Some(v) = gradient_angle { *ga = v; }
            // If either gradient color given, build/update gradient_colors pair
            if gradient_from_hex.is_some() || gradient_to_hex.is_some() {
                let cur = gc.unwrap_or([[255,255,255,255],[255,255,0,255]]);
                let from = gradient_from_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[0]);
                let to   = gradient_to_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[1]);
                *gc = Some([from, to]);
            } else if gradient_from_hex.is_none() && gradient_to_hex.is_none()
                && underline.is_none() && curve_amount.is_none() && gradient_angle.is_none()
            {
                // Clear gradient
                *gc = None;
            }
        }
        _ => return Err("Layer is not a Text layer.".into()),
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced shapes ─────────────────────────────────────────────────

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn add_shape_advanced(
    state: tauri::State<'_, SharedProjectState>,
    shape_type: String,
    width: u32,
    height: u32,
    fill_hex: String,
    border_radius: Option<u32>,
    border_hex: Option<String>,
    border_width: Option<u32>,
    sides: Option<u32>,
    fill_gradient_from: Option<String>,
    fill_gradient_to: Option<String>,
    fill_gradient_angle: Option<f32>,
    x: Option<i64>,
    y: Option<i64>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let cw = s.current.canvas_width as i64;
    let ch = s.current.canvas_height as i64;
    let lx = x.unwrap_or((cw - width as i64) / 2);
    let ly = y.unwrap_or((ch - height as i64) / 2);
    let fill_color = parse_hex_color(&fill_hex);
    let b_color = border_hex.as_deref().map(parse_hex_color).unwrap_or([255,255,255,255]);
    let fill_gradient = match (fill_gradient_from.as_deref(), fill_gradient_to.as_deref()) {
        (Some(a), Some(b)) => Some(BgGradient {
            from: hex_to_rgba8(a),
            to: hex_to_rgba8(b),
            angle: fill_gradient_angle.unwrap_or(0.0),
        }),
        _ => None,
    };
    let id = format!("layer_{}", uuid_v4());
    let name = match shape_type.as_str() {
        "line"     => "Line",
        "arrow"    => "Arrow",
        "triangle" => "Triangle",
        "star"     => "Star",
        "polygon"  => "Polygon",
        "circle"   => "Circle",
        "badge"    => "Badge",
        _          => "Rectangle",
    }.to_string();
    let layer = Layer {
        id: id.clone(),
        name,
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
            shape_type: shape_type.clone(),
            width,
            height,
            fill_color,
            border_radius: border_radius.unwrap_or(0),
            border_color: b_color,
            border_width: border_width.unwrap_or(0),
            sides: sides.unwrap_or(5),
            corner_radii: None,
            fill_gradient,
        },
    };
    s.current.layers.push(layer);
    render_and_respond(&mut s)
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_micros();
    format!("{:x}", ms ^ (ms >> 16))
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
        flip_h: false,
        flip_v: false,
        blend_mode: "normal".to_string(),
        bounds_width: 0,
        bounds_height: 0,
        group_id: None,
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

// ── Phase 3: Font management ─────────────────────────────────────────────────

#[tauri::command]
pub fn import_font(source_path: String) -> Result<String, String> {
    let p = std::path::Path::new(&source_path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext != "ttf" && ext != "otf" {
        return Err("Only .ttf and .otf font files are supported.".into());
    }
    let filename = p.file_name().and_then(|n| n.to_str())
        .ok_or("Invalid file name")?
        .to_string();
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    std::fs::create_dir_all(&fonts_dir).map_err(|e| e.to_string())?;
    let dest = fonts_dir.join(&filename);
    std::fs::copy(&source_path, &dest).map_err(|e| format!("Copy failed: {}", e))?;
    Ok(filename)
}

#[tauri::command]
pub fn list_imported_fonts() -> Result<Vec<String>, String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    if !fonts_dir.exists() {
        return Ok(vec![]);
    }
    let mut names = vec![];
    for entry in std::fs::read_dir(&fonts_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let ext = std::path::Path::new(&name)
            .extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext == "ttf" || ext == "otf" {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

#[tauri::command]
pub fn delete_imported_font(filename: String) -> Result<(), String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    let path = fonts_dir.join(&filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Phase 3: Drop shadow / glow / mask / clip ────────────────────────────────

fn hex_to_rgba8(hex: &str) -> [u8; 4] {
    let h = hex.trim_start_matches('#');
    if h.len() == 6 {
        let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(0);
        [r, g, b, 255]
    } else {
        [0, 0, 0, 255]
    }
}

#[tauri::command]
pub fn update_layer_shadow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    color_hex: Option<String>,
    blur: Option<u32>,
    distance: Option<u32>,
    angle: Option<f32>,
    opacity: Option<u8>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.shadow.enabled = enabled;
    if let Some(c) = color_hex { layer.shadow.color = hex_to_rgba8(&c); }
    if let Some(v) = blur { layer.shadow.blur = v; }
    if let Some(v) = distance { layer.shadow.distance = v; }
    if let Some(v) = angle { layer.shadow.angle = v; }
    if let Some(v) = opacity { layer.shadow.opacity = v; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_glow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    kind: String, // "outer" | "inner"
    enabled: bool,
    color_hex: Option<String>,
    intensity: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    let glow = if kind == "inner" { &mut layer.inner_glow } else { &mut layer.outer_glow };
    glow.enabled = enabled;
    if let Some(c) = color_hex { glow.color = hex_to_rgba8(&c); }
    if let Some(v) = intensity { glow.intensity = v.clamp(1, 60); }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_mask(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    mask_shape: Option<String>, // None | "circle" | "rounded" | "star"
    mask_radius: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.mask_shape = mask_shape;
    if let Some(r) = mask_radius { layer.mask_radius = r; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn set_clip_to_below(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.clip_to_below = enabled;
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced text ───────────────────────────────────────────────────

#[tauri::command]
pub fn update_text_advanced(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    underline: Option<bool>,
    gradient_from_hex: Option<String>,
    gradient_to_hex: Option<String>,
    gradient_angle: Option<f32>,
    curve_amount: Option<f32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    match &mut layer.kind {
        LayerKind::Text3D {
            underline: ul,
            gradient_colors: gc,
            gradient_angle: ga,
            curve_amount: ca,
            ..
        } => {
            if let Some(v) = underline { *ul = v; }
            if let Some(v) = curve_amount { *ca = v.clamp(-180.0, 180.0); }
            if let Some(v) = gradient_angle { *ga = v; }
            // If either gradient color given, build/update gradient_colors pair
            if gradient_from_hex.is_some() || gradient_to_hex.is_some() {
                let cur = gc.unwrap_or([[255,255,255,255],[255,255,0,255]]);
                let from = gradient_from_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[0]);
                let to   = gradient_to_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[1]);
                *gc = Some([from, to]);
            } else if gradient_from_hex.is_none() && gradient_to_hex.is_none()
                && underline.is_none() && curve_amount.is_none() && gradient_angle.is_none()
            {
                // Clear gradient
                *gc = None;
            }
        }
        _ => return Err("Layer is not a Text layer.".into()),
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced shapes ─────────────────────────────────────────────────

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn add_shape_advanced(
    state: tauri::State<'_, SharedProjectState>,
    shape_type: String,
    width: u32,
    height: u32,
    fill_hex: String,
    border_radius: Option<u32>,
    border_hex: Option<String>,
    border_width: Option<u32>,
    sides: Option<u32>,
    fill_gradient_from: Option<String>,
    fill_gradient_to: Option<String>,
    fill_gradient_angle: Option<f32>,
    x: Option<i64>,
    y: Option<i64>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let cw = s.current.canvas_width as i64;
    let ch = s.current.canvas_height as i64;
    let lx = x.unwrap_or((cw - width as i64) / 2);
    let ly = y.unwrap_or((ch - height as i64) / 2);
    let fill_color = parse_hex_color(&fill_hex);
    let b_color = border_hex.as_deref().map(parse_hex_color).unwrap_or([255,255,255,255]);
    let fill_gradient = match (fill_gradient_from.as_deref(), fill_gradient_to.as_deref()) {
        (Some(a), Some(b)) => Some(BgGradient {
            from: hex_to_rgba8(a),
            to: hex_to_rgba8(b),
            angle: fill_gradient_angle.unwrap_or(0.0),
        }),
        _ => None,
    };
    let id = format!("layer_{}", uuid_v4());
    let name = match shape_type.as_str() {
        "line"     => "Line",
        "arrow"    => "Arrow",
        "triangle" => "Triangle",
        "star"     => "Star",
        "polygon"  => "Polygon",
        "circle"   => "Circle",
        "badge"    => "Badge",
        _          => "Rectangle",
    }.to_string();
    let layer = Layer {
        id: id.clone(),
        name,
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
            shape_type: shape_type.clone(),
            width,
            height,
            fill_color,
            border_radius: border_radius.unwrap_or(0),
            border_color: b_color,
            border_width: border_width.unwrap_or(0),
            sides: sides.unwrap_or(5),
            corner_radii: None,
            fill_gradient,
        },
    };
    s.current.layers.push(layer);
    render_and_respond(&mut s)
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_micros();
    format!("{:x}", ms ^ (ms >> 16))
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

// ── Phase 3: Font management ─────────────────────────────────────────────────

#[tauri::command]
pub fn import_font(source_path: String) -> Result<String, String> {
    let p = std::path::Path::new(&source_path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext != "ttf" && ext != "otf" {
        return Err("Only .ttf and .otf font files are supported.".into());
    }
    let filename = p.file_name().and_then(|n| n.to_str())
        .ok_or("Invalid file name")?
        .to_string();
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    std::fs::create_dir_all(&fonts_dir).map_err(|e| e.to_string())?;
    let dest = fonts_dir.join(&filename);
    std::fs::copy(&source_path, &dest).map_err(|e| format!("Copy failed: {}", e))?;
    Ok(filename)
}

#[tauri::command]
pub fn list_imported_fonts() -> Result<Vec<String>, String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    if !fonts_dir.exists() {
        return Ok(vec![]);
    }
    let mut names = vec![];
    for entry in std::fs::read_dir(&fonts_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let ext = std::path::Path::new(&name)
            .extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext == "ttf" || ext == "otf" {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

#[tauri::command]
pub fn delete_imported_font(filename: String) -> Result<(), String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    let path = fonts_dir.join(&filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Phase 3: Drop shadow / glow / mask / clip ────────────────────────────────

fn hex_to_rgba8(hex: &str) -> [u8; 4] {
    let h = hex.trim_start_matches('#');
    if h.len() == 6 {
        let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(0);
        [r, g, b, 255]
    } else {
        [0, 0, 0, 255]
    }
}

#[tauri::command]
pub fn update_layer_shadow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    color_hex: Option<String>,
    blur: Option<u32>,
    distance: Option<u32>,
    angle: Option<f32>,
    opacity: Option<u8>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.shadow.enabled = enabled;
    if let Some(c) = color_hex { layer.shadow.color = hex_to_rgba8(&c); }
    if let Some(v) = blur { layer.shadow.blur = v; }
    if let Some(v) = distance { layer.shadow.distance = v; }
    if let Some(v) = angle { layer.shadow.angle = v; }
    if let Some(v) = opacity { layer.shadow.opacity = v; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_glow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    kind: String, // "outer" | "inner"
    enabled: bool,
    color_hex: Option<String>,
    intensity: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    let glow = if kind == "inner" { &mut layer.inner_glow } else { &mut layer.outer_glow };
    glow.enabled = enabled;
    if let Some(c) = color_hex { glow.color = hex_to_rgba8(&c); }
    if let Some(v) = intensity { glow.intensity = v.clamp(1, 60); }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_mask(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    mask_shape: Option<String>, // None | "circle" | "rounded" | "star"
    mask_radius: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.mask_shape = mask_shape;
    if let Some(r) = mask_radius { layer.mask_radius = r; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn set_clip_to_below(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.clip_to_below = enabled;
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced text ───────────────────────────────────────────────────

#[tauri::command]
pub fn update_text_advanced(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    underline: Option<bool>,
    gradient_from_hex: Option<String>,
    gradient_to_hex: Option<String>,
    gradient_angle: Option<f32>,
    curve_amount: Option<f32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    match &mut layer.kind {
        LayerKind::Text3D {
            underline: ul,
            gradient_colors: gc,
            gradient_angle: ga,
            curve_amount: ca,
            ..
        } => {
            if let Some(v) = underline { *ul = v; }
            if let Some(v) = curve_amount { *ca = v.clamp(-180.0, 180.0); }
            if let Some(v) = gradient_angle { *ga = v; }
            // If either gradient color given, build/update gradient_colors pair
            if gradient_from_hex.is_some() || gradient_to_hex.is_some() {
                let cur = gc.unwrap_or([[255,255,255,255],[255,255,0,255]]);
                let from = gradient_from_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[0]);
                let to   = gradient_to_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[1]);
                *gc = Some([from, to]);
            } else if gradient_from_hex.is_none() && gradient_to_hex.is_none()
                && underline.is_none() && curve_amount.is_none() && gradient_angle.is_none()
            {
                // Clear gradient
                *gc = None;
            }
        }
        _ => return Err("Layer is not a Text layer.".into()),
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced shapes ─────────────────────────────────────────────────

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn add_shape_advanced(
    state: tauri::State<'_, SharedProjectState>,
    shape_type: String,
    width: u32,
    height: u32,
    fill_hex: String,
    border_radius: Option<u32>,
    border_hex: Option<String>,
    border_width: Option<u32>,
    sides: Option<u32>,
    fill_gradient_from: Option<String>,
    fill_gradient_to: Option<String>,
    fill_gradient_angle: Option<f32>,
    x: Option<i64>,
    y: Option<i64>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let cw = s.current.canvas_width as i64;
    let ch = s.current.canvas_height as i64;
    let lx = x.unwrap_or((cw - width as i64) / 2);
    let ly = y.unwrap_or((ch - height as i64) / 2);
    let fill_color = parse_hex_color(&fill_hex);
    let b_color = border_hex.as_deref().map(parse_hex_color).unwrap_or([255,255,255,255]);
    let fill_gradient = match (fill_gradient_from.as_deref(), fill_gradient_to.as_deref()) {
        (Some(a), Some(b)) => Some(BgGradient {
            from: hex_to_rgba8(a),
            to: hex_to_rgba8(b),
            angle: fill_gradient_angle.unwrap_or(0.0),
        }),
        _ => None,
    };
    let id = format!("layer_{}", uuid_v4());
    let name = match shape_type.as_str() {
        "line"     => "Line",
        "arrow"    => "Arrow",
        "triangle" => "Triangle",
        "star"     => "Star",
        "polygon"  => "Polygon",
        "circle"   => "Circle",
        "badge"    => "Badge",
        _          => "Rectangle",
    }.to_string();
    let layer = Layer {
        id: id.clone(),
        name,
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
            shape_type: shape_type.clone(),
            width,
            height,
            fill_color,
            border_radius: border_radius.unwrap_or(0),
            border_color: b_color,
            border_width: border_width.unwrap_or(0),
            sides: sides.unwrap_or(5),
            corner_radii: None,
            fill_gradient,
        },
    };
    s.current.layers.push(layer);
    render_and_respond(&mut s)
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_micros();
    format!("{:x}", ms ^ (ms >> 16))
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
    bold: Option<bool>,
    italic: Option<bool>,
    align: Option<String>,
    line_height: Option<f32>,
    letter_spacing: Option<f32>,
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
            color: parse_hex_color(&color_hex),
            depth: depth_3d,
            shadow_color: [0, 0, 0, 180],
            stroke_color: parse_hex_color(&stroke_hex),
            stroke_width: stroke_width.unwrap_or(2),
            size: size.max(8.0),
            bold: bold.unwrap_or(false),
            italic: italic.unwrap_or(false),
            align: align.unwrap_or_else(|| "left".to_string()),
            line_height: line_height.unwrap_or(1.2),
            letter_spacing: letter_spacing.unwrap_or(0.0),
        },
    });
    render_and_respond(&mut s)
}

// ── Phase 3: Font management ─────────────────────────────────────────────────

#[tauri::command]
pub fn import_font(source_path: String) -> Result<String, String> {
    let p = std::path::Path::new(&source_path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext != "ttf" && ext != "otf" {
        return Err("Only .ttf and .otf font files are supported.".into());
    }
    let filename = p.file_name().and_then(|n| n.to_str())
        .ok_or("Invalid file name")?
        .to_string();
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    std::fs::create_dir_all(&fonts_dir).map_err(|e| e.to_string())?;
    let dest = fonts_dir.join(&filename);
    std::fs::copy(&source_path, &dest).map_err(|e| format!("Copy failed: {}", e))?;
    Ok(filename)
}

#[tauri::command]
pub fn list_imported_fonts() -> Result<Vec<String>, String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    if !fonts_dir.exists() {
        return Ok(vec![]);
    }
    let mut names = vec![];
    for entry in std::fs::read_dir(&fonts_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let ext = std::path::Path::new(&name)
            .extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext == "ttf" || ext == "otf" {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

#[tauri::command]
pub fn delete_imported_font(filename: String) -> Result<(), String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    let path = fonts_dir.join(&filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Phase 3: Drop shadow / glow / mask / clip ────────────────────────────────

fn hex_to_rgba8(hex: &str) -> [u8; 4] {
    let h = hex.trim_start_matches('#');
    if h.len() == 6 {
        let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(0);
        [r, g, b, 255]
    } else {
        [0, 0, 0, 255]
    }
}

#[tauri::command]
pub fn update_layer_shadow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    color_hex: Option<String>,
    blur: Option<u32>,
    distance: Option<u32>,
    angle: Option<f32>,
    opacity: Option<u8>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.shadow.enabled = enabled;
    if let Some(c) = color_hex { layer.shadow.color = hex_to_rgba8(&c); }
    if let Some(v) = blur { layer.shadow.blur = v; }
    if let Some(v) = distance { layer.shadow.distance = v; }
    if let Some(v) = angle { layer.shadow.angle = v; }
    if let Some(v) = opacity { layer.shadow.opacity = v; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_glow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    kind: String, // "outer" | "inner"
    enabled: bool,
    color_hex: Option<String>,
    intensity: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    let glow = if kind == "inner" { &mut layer.inner_glow } else { &mut layer.outer_glow };
    glow.enabled = enabled;
    if let Some(c) = color_hex { glow.color = hex_to_rgba8(&c); }
    if let Some(v) = intensity { glow.intensity = v.clamp(1, 60); }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_mask(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    mask_shape: Option<String>, // None | "circle" | "rounded" | "star"
    mask_radius: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.mask_shape = mask_shape;
    if let Some(r) = mask_radius { layer.mask_radius = r; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn set_clip_to_below(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.clip_to_below = enabled;
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced text ───────────────────────────────────────────────────

#[tauri::command]
pub fn update_text_advanced(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    underline: Option<bool>,
    gradient_from_hex: Option<String>,
    gradient_to_hex: Option<String>,
    gradient_angle: Option<f32>,
    curve_amount: Option<f32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    match &mut layer.kind {
        LayerKind::Text3D {
            underline: ul,
            gradient_colors: gc,
            gradient_angle: ga,
            curve_amount: ca,
            ..
        } => {
            if let Some(v) = underline { *ul = v; }
            if let Some(v) = curve_amount { *ca = v.clamp(-180.0, 180.0); }
            if let Some(v) = gradient_angle { *ga = v; }
            // If either gradient color given, build/update gradient_colors pair
            if gradient_from_hex.is_some() || gradient_to_hex.is_some() {
                let cur = gc.unwrap_or([[255,255,255,255],[255,255,0,255]]);
                let from = gradient_from_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[0]);
                let to   = gradient_to_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[1]);
                *gc = Some([from, to]);
            } else if gradient_from_hex.is_none() && gradient_to_hex.is_none()
                && underline.is_none() && curve_amount.is_none() && gradient_angle.is_none()
            {
                // Clear gradient
                *gc = None;
            }
        }
        _ => return Err("Layer is not a Text layer.".into()),
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced shapes ─────────────────────────────────────────────────

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn add_shape_advanced(
    state: tauri::State<'_, SharedProjectState>,
    shape_type: String,
    width: u32,
    height: u32,
    fill_hex: String,
    border_radius: Option<u32>,
    border_hex: Option<String>,
    border_width: Option<u32>,
    sides: Option<u32>,
    fill_gradient_from: Option<String>,
    fill_gradient_to: Option<String>,
    fill_gradient_angle: Option<f32>,
    x: Option<i64>,
    y: Option<i64>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let cw = s.current.canvas_width as i64;
    let ch = s.current.canvas_height as i64;
    let lx = x.unwrap_or((cw - width as i64) / 2);
    let ly = y.unwrap_or((ch - height as i64) / 2);
    let fill_color = parse_hex_color(&fill_hex);
    let b_color = border_hex.as_deref().map(parse_hex_color).unwrap_or([255,255,255,255]);
    let fill_gradient = match (fill_gradient_from.as_deref(), fill_gradient_to.as_deref()) {
        (Some(a), Some(b)) => Some(BgGradient {
            from: hex_to_rgba8(a),
            to: hex_to_rgba8(b),
            angle: fill_gradient_angle.unwrap_or(0.0),
        }),
        _ => None,
    };
    let id = format!("layer_{}", uuid_v4());
    let name = match shape_type.as_str() {
        "line"     => "Line",
        "arrow"    => "Arrow",
        "triangle" => "Triangle",
        "star"     => "Star",
        "polygon"  => "Polygon",
        "circle"   => "Circle",
        "badge"    => "Badge",
        _          => "Rectangle",
    }.to_string();
    let layer = Layer {
        id: id.clone(),
        name,
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
            shape_type: shape_type.clone(),
            width,
            height,
            fill_color,
            border_radius: border_radius.unwrap_or(0),
            border_color: b_color,
            border_width: border_width.unwrap_or(0),
            sides: sides.unwrap_or(5),
            corner_radii: None,
            fill_gradient,
        },
    };
    s.current.layers.push(layer);
    render_and_respond(&mut s)
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_micros();
    format!("{:x}", ms ^ (ms >> 16))
}

#[tauri::command]
pub fn push_undo_checkpoint(
    state: tauri::State<'_, SharedProjectState>,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if s.has_project {
        s.push_undo();
    }
    Ok(())
}

#[tauri::command]
pub fn update_text_layer(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    content: String,
    font_style: String,
    size: f32,
    color_hex: String,
    depth_3d: u32,
    stroke_hex: String,
    stroke_width: Option<u32>,
    bold: Option<bool>,
    italic: Option<bool>,
    align: Option<String>,
    line_height: Option<f32>,
    letter_spacing: Option<f32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) {
        s.push_undo();
    }
    let layer = s
        .current
        .layers
        .iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;

    let label: String = if content.trim().is_empty() {
        "Empty".to_string()
    } else {
        content.chars().take(18).collect()
    };
    layer.name = format!("Text: {}", label);
    match &mut layer.kind {
        LayerKind::Text3D {
            content: c,
            font_style: fs,
            color,
            depth,
            stroke_color,
            stroke_width: sw,
            size: sz,
            bold: b,
            italic: it,
            align: al,
            line_height: lh,
            letter_spacing: ls,
            ..
        } => {
            *c = content;
            *fs = font_style;
            *color = parse_hex_color(&color_hex);
            *depth = depth_3d;
            *stroke_color = parse_hex_color(&stroke_hex);
            *sw = stroke_width.unwrap_or(*sw);
            *sz = size.max(8.0);
            if let Some(v) = bold {
                *b = v;
            }
            if let Some(v) = italic {
                *it = v;
            }
            if let Some(v) = align {
                *al = v;
            }
            if let Some(v) = line_height {
                *lh = v.clamp(0.7, 3.0);
            }
            if let Some(v) = letter_spacing {
                *ls = v.clamp(-20.0, 80.0);
            }
        }
        _ => return Err("Selected layer is not a Text layer.".to_string()),
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Font management ─────────────────────────────────────────────────

#[tauri::command]
pub fn import_font(source_path: String) -> Result<String, String> {
    let p = std::path::Path::new(&source_path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext != "ttf" && ext != "otf" {
        return Err("Only .ttf and .otf font files are supported.".into());
    }
    let filename = p.file_name().and_then(|n| n.to_str())
        .ok_or("Invalid file name")?
        .to_string();
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    std::fs::create_dir_all(&fonts_dir).map_err(|e| e.to_string())?;
    let dest = fonts_dir.join(&filename);
    std::fs::copy(&source_path, &dest).map_err(|e| format!("Copy failed: {}", e))?;
    Ok(filename)
}

#[tauri::command]
pub fn list_imported_fonts() -> Result<Vec<String>, String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    if !fonts_dir.exists() {
        return Ok(vec![]);
    }
    let mut names = vec![];
    for entry in std::fs::read_dir(&fonts_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let ext = std::path::Path::new(&name)
            .extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext == "ttf" || ext == "otf" {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

#[tauri::command]
pub fn delete_imported_font(filename: String) -> Result<(), String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    let path = fonts_dir.join(&filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Phase 3: Drop shadow / glow / mask / clip ────────────────────────────────

fn hex_to_rgba8(hex: &str) -> [u8; 4] {
    let h = hex.trim_start_matches('#');
    if h.len() == 6 {
        let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(0);
        [r, g, b, 255]
    } else {
        [0, 0, 0, 255]
    }
}

#[tauri::command]
pub fn update_layer_shadow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    color_hex: Option<String>,
    blur: Option<u32>,
    distance: Option<u32>,
    angle: Option<f32>,
    opacity: Option<u8>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.shadow.enabled = enabled;
    if let Some(c) = color_hex { layer.shadow.color = hex_to_rgba8(&c); }
    if let Some(v) = blur { layer.shadow.blur = v; }
    if let Some(v) = distance { layer.shadow.distance = v; }
    if let Some(v) = angle { layer.shadow.angle = v; }
    if let Some(v) = opacity { layer.shadow.opacity = v; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_glow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    kind: String, // "outer" | "inner"
    enabled: bool,
    color_hex: Option<String>,
    intensity: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    let glow = if kind == "inner" { &mut layer.inner_glow } else { &mut layer.outer_glow };
    glow.enabled = enabled;
    if let Some(c) = color_hex { glow.color = hex_to_rgba8(&c); }
    if let Some(v) = intensity { glow.intensity = v.clamp(1, 60); }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_mask(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    mask_shape: Option<String>, // None | "circle" | "rounded" | "star"
    mask_radius: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.mask_shape = mask_shape;
    if let Some(r) = mask_radius { layer.mask_radius = r; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn set_clip_to_below(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.clip_to_below = enabled;
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced text ───────────────────────────────────────────────────

#[tauri::command]
pub fn update_text_advanced(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    underline: Option<bool>,
    gradient_from_hex: Option<String>,
    gradient_to_hex: Option<String>,
    gradient_angle: Option<f32>,
    curve_amount: Option<f32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    match &mut layer.kind {
        LayerKind::Text3D {
            underline: ul,
            gradient_colors: gc,
            gradient_angle: ga,
            curve_amount: ca,
            ..
        } => {
            if let Some(v) = underline { *ul = v; }
            if let Some(v) = curve_amount { *ca = v.clamp(-180.0, 180.0); }
            if let Some(v) = gradient_angle { *ga = v; }
            // If either gradient color given, build/update gradient_colors pair
            if gradient_from_hex.is_some() || gradient_to_hex.is_some() {
                let cur = gc.unwrap_or([[255,255,255,255],[255,255,0,255]]);
                let from = gradient_from_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[0]);
                let to   = gradient_to_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[1]);
                *gc = Some([from, to]);
            } else if gradient_from_hex.is_none() && gradient_to_hex.is_none()
                && underline.is_none() && curve_amount.is_none() && gradient_angle.is_none()
            {
                // Clear gradient
                *gc = None;
            }
        }
        _ => return Err("Layer is not a Text layer.".into()),
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced shapes ─────────────────────────────────────────────────

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn add_shape_advanced(
    state: tauri::State<'_, SharedProjectState>,
    shape_type: String,
    width: u32,
    height: u32,
    fill_hex: String,
    border_radius: Option<u32>,
    border_hex: Option<String>,
    border_width: Option<u32>,
    sides: Option<u32>,
    fill_gradient_from: Option<String>,
    fill_gradient_to: Option<String>,
    fill_gradient_angle: Option<f32>,
    x: Option<i64>,
    y: Option<i64>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let cw = s.current.canvas_width as i64;
    let ch = s.current.canvas_height as i64;
    let lx = x.unwrap_or((cw - width as i64) / 2);
    let ly = y.unwrap_or((ch - height as i64) / 2);
    let fill_color = parse_hex_color(&fill_hex);
    let b_color = border_hex.as_deref().map(parse_hex_color).unwrap_or([255,255,255,255]);
    let fill_gradient = match (fill_gradient_from.as_deref(), fill_gradient_to.as_deref()) {
        (Some(a), Some(b)) => Some(BgGradient {
            from: hex_to_rgba8(a),
            to: hex_to_rgba8(b),
            angle: fill_gradient_angle.unwrap_or(0.0),
        }),
        _ => None,
    };
    let id = format!("layer_{}", uuid_v4());
    let name = match shape_type.as_str() {
        "line"     => "Line",
        "arrow"    => "Arrow",
        "triangle" => "Triangle",
        "star"     => "Star",
        "polygon"  => "Polygon",
        "circle"   => "Circle",
        "badge"    => "Badge",
        _          => "Rectangle",
    }.to_string();
    let layer = Layer {
        id: id.clone(),
        name,
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
            shape_type: shape_type.clone(),
            width,
            height,
            fill_color,
            border_radius: border_radius.unwrap_or(0),
            border_color: b_color,
            border_width: border_width.unwrap_or(0),
            sides: sides.unwrap_or(5),
            corner_radii: None,
            fill_gradient,
        },
    };
    s.current.layers.push(layer);
    render_and_respond(&mut s)
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_micros();
    format!("{:x}", ms ^ (ms >> 16))
}

#[tauri::command]
pub fn update_shape_layer(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    width: u32,
    height: u32,
    fill_hex: String,
    border_radius: u32,
    border_hex: Option<String>,
    border_width: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) {
        s.push_undo();
    }
    let layer = s
        .current
        .layers
        .iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;

    match &mut layer.kind {
        LayerKind::Shape {
            width: w,
            height: h,
            fill_color,
            border_radius: br,
            border_color,
            border_width: bw,
            ..
        } => {
            *w = width.max(4);
            *h = height.max(4);
            *fill_color = parse_hex_color(&fill_hex);
            *br = border_radius;
            if let Some(bh) = border_hex.as_deref() {
                *border_color = parse_hex_color(bh);
            }
            if let Some(bwidth) = border_width {
                *bw = bwidth;
            }
        }
        _ => return Err("Selected layer is not a Shape layer.".to_string()),
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Font management ─────────────────────────────────────────────────

#[tauri::command]
pub fn import_font(source_path: String) -> Result<String, String> {
    let p = std::path::Path::new(&source_path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext != "ttf" && ext != "otf" {
        return Err("Only .ttf and .otf font files are supported.".into());
    }
    let filename = p.file_name().and_then(|n| n.to_str())
        .ok_or("Invalid file name")?
        .to_string();
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    std::fs::create_dir_all(&fonts_dir).map_err(|e| e.to_string())?;
    let dest = fonts_dir.join(&filename);
    std::fs::copy(&source_path, &dest).map_err(|e| format!("Copy failed: {}", e))?;
    Ok(filename)
}

#[tauri::command]
pub fn list_imported_fonts() -> Result<Vec<String>, String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    if !fonts_dir.exists() {
        return Ok(vec![]);
    }
    let mut names = vec![];
    for entry in std::fs::read_dir(&fonts_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let ext = std::path::Path::new(&name)
            .extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext == "ttf" || ext == "otf" {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

#[tauri::command]
pub fn delete_imported_font(filename: String) -> Result<(), String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    let path = fonts_dir.join(&filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Phase 3: Drop shadow / glow / mask / clip ────────────────────────────────

fn hex_to_rgba8(hex: &str) -> [u8; 4] {
    let h = hex.trim_start_matches('#');
    if h.len() == 6 {
        let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(0);
        [r, g, b, 255]
    } else {
        [0, 0, 0, 255]
    }
}

#[tauri::command]
pub fn update_layer_shadow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    color_hex: Option<String>,
    blur: Option<u32>,
    distance: Option<u32>,
    angle: Option<f32>,
    opacity: Option<u8>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.shadow.enabled = enabled;
    if let Some(c) = color_hex { layer.shadow.color = hex_to_rgba8(&c); }
    if let Some(v) = blur { layer.shadow.blur = v; }
    if let Some(v) = distance { layer.shadow.distance = v; }
    if let Some(v) = angle { layer.shadow.angle = v; }
    if let Some(v) = opacity { layer.shadow.opacity = v; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_glow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    kind: String, // "outer" | "inner"
    enabled: bool,
    color_hex: Option<String>,
    intensity: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    let glow = if kind == "inner" { &mut layer.inner_glow } else { &mut layer.outer_glow };
    glow.enabled = enabled;
    if let Some(c) = color_hex { glow.color = hex_to_rgba8(&c); }
    if let Some(v) = intensity { glow.intensity = v.clamp(1, 60); }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_mask(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    mask_shape: Option<String>, // None | "circle" | "rounded" | "star"
    mask_radius: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.mask_shape = mask_shape;
    if let Some(r) = mask_radius { layer.mask_radius = r; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn set_clip_to_below(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.clip_to_below = enabled;
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced text ───────────────────────────────────────────────────

#[tauri::command]
pub fn update_text_advanced(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    underline: Option<bool>,
    gradient_from_hex: Option<String>,
    gradient_to_hex: Option<String>,
    gradient_angle: Option<f32>,
    curve_amount: Option<f32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    match &mut layer.kind {
        LayerKind::Text3D {
            underline: ul,
            gradient_colors: gc,
            gradient_angle: ga,
            curve_amount: ca,
            ..
        } => {
            if let Some(v) = underline { *ul = v; }
            if let Some(v) = curve_amount { *ca = v.clamp(-180.0, 180.0); }
            if let Some(v) = gradient_angle { *ga = v; }
            // If either gradient color given, build/update gradient_colors pair
            if gradient_from_hex.is_some() || gradient_to_hex.is_some() {
                let cur = gc.unwrap_or([[255,255,255,255],[255,255,0,255]]);
                let from = gradient_from_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[0]);
                let to   = gradient_to_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[1]);
                *gc = Some([from, to]);
            } else if gradient_from_hex.is_none() && gradient_to_hex.is_none()
                && underline.is_none() && curve_amount.is_none() && gradient_angle.is_none()
            {
                // Clear gradient
                *gc = None;
            }
        }
        _ => return Err("Layer is not a Text layer.".into()),
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced shapes ─────────────────────────────────────────────────

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn add_shape_advanced(
    state: tauri::State<'_, SharedProjectState>,
    shape_type: String,
    width: u32,
    height: u32,
    fill_hex: String,
    border_radius: Option<u32>,
    border_hex: Option<String>,
    border_width: Option<u32>,
    sides: Option<u32>,
    fill_gradient_from: Option<String>,
    fill_gradient_to: Option<String>,
    fill_gradient_angle: Option<f32>,
    x: Option<i64>,
    y: Option<i64>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let cw = s.current.canvas_width as i64;
    let ch = s.current.canvas_height as i64;
    let lx = x.unwrap_or((cw - width as i64) / 2);
    let ly = y.unwrap_or((ch - height as i64) / 2);
    let fill_color = parse_hex_color(&fill_hex);
    let b_color = border_hex.as_deref().map(parse_hex_color).unwrap_or([255,255,255,255]);
    let fill_gradient = match (fill_gradient_from.as_deref(), fill_gradient_to.as_deref()) {
        (Some(a), Some(b)) => Some(BgGradient {
            from: hex_to_rgba8(a),
            to: hex_to_rgba8(b),
            angle: fill_gradient_angle.unwrap_or(0.0),
        }),
        _ => None,
    };
    let id = format!("layer_{}", uuid_v4());
    let name = match shape_type.as_str() {
        "line"     => "Line",
        "arrow"    => "Arrow",
        "triangle" => "Triangle",
        "star"     => "Star",
        "polygon"  => "Polygon",
        "circle"   => "Circle",
        "badge"    => "Badge",
        _          => "Rectangle",
    }.to_string();
    let layer = Layer {
        id: id.clone(),
        name,
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
            shape_type: shape_type.clone(),
            width,
            height,
            fill_color,
            border_radius: border_radius.unwrap_or(0),
            border_color: b_color,
            border_width: border_width.unwrap_or(0),
            sides: sides.unwrap_or(5),
            corner_radii: None,
            fill_gradient,
        },
    };
    s.current.layers.push(layer);
    render_and_respond(&mut s)
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_micros();
    format!("{:x}", ms ^ (ms >> 16))
}

#[tauri::command]
pub fn replace_image_layer(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    image_path: String,
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
    let layer = s
        .current
        .layers
        .iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;

    layer.name = name;
    match &mut layer.kind {
        LayerKind::Image { source_path, .. } => {
            *source_path = image_path;
        }
        _ => return Err("Selected layer is not an Image layer.".to_string()),
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Font management ─────────────────────────────────────────────────

#[tauri::command]
pub fn import_font(source_path: String) -> Result<String, String> {
    let p = std::path::Path::new(&source_path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext != "ttf" && ext != "otf" {
        return Err("Only .ttf and .otf font files are supported.".into());
    }
    let filename = p.file_name().and_then(|n| n.to_str())
        .ok_or("Invalid file name")?
        .to_string();
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    std::fs::create_dir_all(&fonts_dir).map_err(|e| e.to_string())?;
    let dest = fonts_dir.join(&filename);
    std::fs::copy(&source_path, &dest).map_err(|e| format!("Copy failed: {}", e))?;
    Ok(filename)
}

#[tauri::command]
pub fn list_imported_fonts() -> Result<Vec<String>, String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    if !fonts_dir.exists() {
        return Ok(vec![]);
    }
    let mut names = vec![];
    for entry in std::fs::read_dir(&fonts_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let ext = std::path::Path::new(&name)
            .extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext == "ttf" || ext == "otf" {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

#[tauri::command]
pub fn delete_imported_font(filename: String) -> Result<(), String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    let path = fonts_dir.join(&filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Phase 3: Drop shadow / glow / mask / clip ────────────────────────────────

fn hex_to_rgba8(hex: &str) -> [u8; 4] {
    let h = hex.trim_start_matches('#');
    if h.len() == 6 {
        let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(0);
        [r, g, b, 255]
    } else {
        [0, 0, 0, 255]
    }
}

#[tauri::command]
pub fn update_layer_shadow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    color_hex: Option<String>,
    blur: Option<u32>,
    distance: Option<u32>,
    angle: Option<f32>,
    opacity: Option<u8>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.shadow.enabled = enabled;
    if let Some(c) = color_hex { layer.shadow.color = hex_to_rgba8(&c); }
    if let Some(v) = blur { layer.shadow.blur = v; }
    if let Some(v) = distance { layer.shadow.distance = v; }
    if let Some(v) = angle { layer.shadow.angle = v; }
    if let Some(v) = opacity { layer.shadow.opacity = v; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_glow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    kind: String, // "outer" | "inner"
    enabled: bool,
    color_hex: Option<String>,
    intensity: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    let glow = if kind == "inner" { &mut layer.inner_glow } else { &mut layer.outer_glow };
    glow.enabled = enabled;
    if let Some(c) = color_hex { glow.color = hex_to_rgba8(&c); }
    if let Some(v) = intensity { glow.intensity = v.clamp(1, 60); }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_mask(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    mask_shape: Option<String>, // None | "circle" | "rounded" | "star"
    mask_radius: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.mask_shape = mask_shape;
    if let Some(r) = mask_radius { layer.mask_radius = r; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn set_clip_to_below(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.clip_to_below = enabled;
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced text ───────────────────────────────────────────────────

#[tauri::command]
pub fn update_text_advanced(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    underline: Option<bool>,
    gradient_from_hex: Option<String>,
    gradient_to_hex: Option<String>,
    gradient_angle: Option<f32>,
    curve_amount: Option<f32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    match &mut layer.kind {
        LayerKind::Text3D {
            underline: ul,
            gradient_colors: gc,
            gradient_angle: ga,
            curve_amount: ca,
            ..
        } => {
            if let Some(v) = underline { *ul = v; }
            if let Some(v) = curve_amount { *ca = v.clamp(-180.0, 180.0); }
            if let Some(v) = gradient_angle { *ga = v; }
            // If either gradient color given, build/update gradient_colors pair
            if gradient_from_hex.is_some() || gradient_to_hex.is_some() {
                let cur = gc.unwrap_or([[255,255,255,255],[255,255,0,255]]);
                let from = gradient_from_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[0]);
                let to   = gradient_to_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[1]);
                *gc = Some([from, to]);
            } else if gradient_from_hex.is_none() && gradient_to_hex.is_none()
                && underline.is_none() && curve_amount.is_none() && gradient_angle.is_none()
            {
                // Clear gradient
                *gc = None;
            }
        }
        _ => return Err("Layer is not a Text layer.".into()),
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced shapes ─────────────────────────────────────────────────

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn add_shape_advanced(
    state: tauri::State<'_, SharedProjectState>,
    shape_type: String,
    width: u32,
    height: u32,
    fill_hex: String,
    border_radius: Option<u32>,
    border_hex: Option<String>,
    border_width: Option<u32>,
    sides: Option<u32>,
    fill_gradient_from: Option<String>,
    fill_gradient_to: Option<String>,
    fill_gradient_angle: Option<f32>,
    x: Option<i64>,
    y: Option<i64>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let cw = s.current.canvas_width as i64;
    let ch = s.current.canvas_height as i64;
    let lx = x.unwrap_or((cw - width as i64) / 2);
    let ly = y.unwrap_or((ch - height as i64) / 2);
    let fill_color = parse_hex_color(&fill_hex);
    let b_color = border_hex.as_deref().map(parse_hex_color).unwrap_or([255,255,255,255]);
    let fill_gradient = match (fill_gradient_from.as_deref(), fill_gradient_to.as_deref()) {
        (Some(a), Some(b)) => Some(BgGradient {
            from: hex_to_rgba8(a),
            to: hex_to_rgba8(b),
            angle: fill_gradient_angle.unwrap_or(0.0),
        }),
        _ => None,
    };
    let id = format!("layer_{}", uuid_v4());
    let name = match shape_type.as_str() {
        "line"     => "Line",
        "arrow"    => "Arrow",
        "triangle" => "Triangle",
        "star"     => "Star",
        "polygon"  => "Polygon",
        "circle"   => "Circle",
        "badge"    => "Badge",
        _          => "Rectangle",
    }.to_string();
    let layer = Layer {
        id: id.clone(),
        name,
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
            shape_type: shape_type.clone(),
            width,
            height,
            fill_color,
            border_radius: border_radius.unwrap_or(0),
            border_color: b_color,
            border_width: border_width.unwrap_or(0),
            sides: sides.unwrap_or(5),
            corner_radii: None,
            fill_gradient,
        },
    };
    s.current.layers.push(layer);
    render_and_respond(&mut s)
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_micros();
    format!("{:x}", ms ^ (ms >> 16))
}

#[tauri::command]
pub fn crop_canvas(
    state: tauri::State<'_, SharedProjectState>,
    x: i64,
    y: i64,
    width: u32,
    height: u32,
) -> Result<ProjectResponse, String> {
    if width == 0 || height == 0 {
        return Err("Crop width and height must be greater than zero.".to_string());
    }
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if !s.has_project {
        return Err("No active project to crop.".to_string());
    }
    s.push_undo();
    s.current.canvas_width = width;
    s.current.canvas_height = height;
    for layer in &mut s.current.layers {
        layer.x -= x;
        layer.y -= y;
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Font management ─────────────────────────────────────────────────

#[tauri::command]
pub fn import_font(source_path: String) -> Result<String, String> {
    let p = std::path::Path::new(&source_path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext != "ttf" && ext != "otf" {
        return Err("Only .ttf and .otf font files are supported.".into());
    }
    let filename = p.file_name().and_then(|n| n.to_str())
        .ok_or("Invalid file name")?
        .to_string();
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    std::fs::create_dir_all(&fonts_dir).map_err(|e| e.to_string())?;
    let dest = fonts_dir.join(&filename);
    std::fs::copy(&source_path, &dest).map_err(|e| format!("Copy failed: {}", e))?;
    Ok(filename)
}

#[tauri::command]
pub fn list_imported_fonts() -> Result<Vec<String>, String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    if !fonts_dir.exists() {
        return Ok(vec![]);
    }
    let mut names = vec![];
    for entry in std::fs::read_dir(&fonts_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let ext = std::path::Path::new(&name)
            .extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext == "ttf" || ext == "otf" {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

#[tauri::command]
pub fn delete_imported_font(filename: String) -> Result<(), String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    let path = fonts_dir.join(&filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Phase 3: Drop shadow / glow / mask / clip ────────────────────────────────

fn hex_to_rgba8(hex: &str) -> [u8; 4] {
    let h = hex.trim_start_matches('#');
    if h.len() == 6 {
        let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(0);
        [r, g, b, 255]
    } else {
        [0, 0, 0, 255]
    }
}

#[tauri::command]
pub fn update_layer_shadow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    color_hex: Option<String>,
    blur: Option<u32>,
    distance: Option<u32>,
    angle: Option<f32>,
    opacity: Option<u8>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.shadow.enabled = enabled;
    if let Some(c) = color_hex { layer.shadow.color = hex_to_rgba8(&c); }
    if let Some(v) = blur { layer.shadow.blur = v; }
    if let Some(v) = distance { layer.shadow.distance = v; }
    if let Some(v) = angle { layer.shadow.angle = v; }
    if let Some(v) = opacity { layer.shadow.opacity = v; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_glow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    kind: String, // "outer" | "inner"
    enabled: bool,
    color_hex: Option<String>,
    intensity: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    let glow = if kind == "inner" { &mut layer.inner_glow } else { &mut layer.outer_glow };
    glow.enabled = enabled;
    if let Some(c) = color_hex { glow.color = hex_to_rgba8(&c); }
    if let Some(v) = intensity { glow.intensity = v.clamp(1, 60); }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_mask(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    mask_shape: Option<String>, // None | "circle" | "rounded" | "star"
    mask_radius: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.mask_shape = mask_shape;
    if let Some(r) = mask_radius { layer.mask_radius = r; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn set_clip_to_below(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.clip_to_below = enabled;
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced text ───────────────────────────────────────────────────

#[tauri::command]
pub fn update_text_advanced(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    underline: Option<bool>,
    gradient_from_hex: Option<String>,
    gradient_to_hex: Option<String>,
    gradient_angle: Option<f32>,
    curve_amount: Option<f32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    match &mut layer.kind {
        LayerKind::Text3D {
            underline: ul,
            gradient_colors: gc,
            gradient_angle: ga,
            curve_amount: ca,
            ..
        } => {
            if let Some(v) = underline { *ul = v; }
            if let Some(v) = curve_amount { *ca = v.clamp(-180.0, 180.0); }
            if let Some(v) = gradient_angle { *ga = v; }
            // If either gradient color given, build/update gradient_colors pair
            if gradient_from_hex.is_some() || gradient_to_hex.is_some() {
                let cur = gc.unwrap_or([[255,255,255,255],[255,255,0,255]]);
                let from = gradient_from_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[0]);
                let to   = gradient_to_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[1]);
                *gc = Some([from, to]);
            } else if gradient_from_hex.is_none() && gradient_to_hex.is_none()
                && underline.is_none() && curve_amount.is_none() && gradient_angle.is_none()
            {
                // Clear gradient
                *gc = None;
            }
        }
        _ => return Err("Layer is not a Text layer.".into()),
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced shapes ─────────────────────────────────────────────────

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn add_shape_advanced(
    state: tauri::State<'_, SharedProjectState>,
    shape_type: String,
    width: u32,
    height: u32,
    fill_hex: String,
    border_radius: Option<u32>,
    border_hex: Option<String>,
    border_width: Option<u32>,
    sides: Option<u32>,
    fill_gradient_from: Option<String>,
    fill_gradient_to: Option<String>,
    fill_gradient_angle: Option<f32>,
    x: Option<i64>,
    y: Option<i64>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let cw = s.current.canvas_width as i64;
    let ch = s.current.canvas_height as i64;
    let lx = x.unwrap_or((cw - width as i64) / 2);
    let ly = y.unwrap_or((ch - height as i64) / 2);
    let fill_color = parse_hex_color(&fill_hex);
    let b_color = border_hex.as_deref().map(parse_hex_color).unwrap_or([255,255,255,255]);
    let fill_gradient = match (fill_gradient_from.as_deref(), fill_gradient_to.as_deref()) {
        (Some(a), Some(b)) => Some(BgGradient {
            from: hex_to_rgba8(a),
            to: hex_to_rgba8(b),
            angle: fill_gradient_angle.unwrap_or(0.0),
        }),
        _ => None,
    };
    let id = format!("layer_{}", uuid_v4());
    let name = match shape_type.as_str() {
        "line"     => "Line",
        "arrow"    => "Arrow",
        "triangle" => "Triangle",
        "star"     => "Star",
        "polygon"  => "Polygon",
        "circle"   => "Circle",
        "badge"    => "Badge",
        _          => "Rectangle",
    }.to_string();
    let layer = Layer {
        id: id.clone(),
        name,
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
            shape_type: shape_type.clone(),
            width,
            height,
            fill_color,
            border_radius: border_radius.unwrap_or(0),
            border_color: b_color,
            border_width: border_width.unwrap_or(0),
            sides: sides.unwrap_or(5),
            corner_radii: None,
            fill_gradient,
        },
    };
    s.current.layers.push(layer);
    render_and_respond(&mut s)
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_micros();
    format!("{:x}", ms ^ (ms >> 16))
}

#[tauri::command]
pub fn add_shape_layer(
    state: tauri::State<'_, SharedProjectState>,
    shape_type: String,
    width: u32,
    height: u32,
    fill_hex: String,
    border_radius: u32,
    border_hex: Option<String>,
    border_width: Option<u32>,
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
            fill_color: parse_hex_color(&fill_hex),
            border_radius,
            border_color: border_hex.as_deref().map(parse_hex_color).unwrap_or([0, 0, 0, 0]),
            border_width: border_width.unwrap_or(0),
        },
    });
    render_and_respond(&mut s)
}

// ── Phase 3: Font management ─────────────────────────────────────────────────

#[tauri::command]
pub fn import_font(source_path: String) -> Result<String, String> {
    let p = std::path::Path::new(&source_path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext != "ttf" && ext != "otf" {
        return Err("Only .ttf and .otf font files are supported.".into());
    }
    let filename = p.file_name().and_then(|n| n.to_str())
        .ok_or("Invalid file name")?
        .to_string();
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    std::fs::create_dir_all(&fonts_dir).map_err(|e| e.to_string())?;
    let dest = fonts_dir.join(&filename);
    std::fs::copy(&source_path, &dest).map_err(|e| format!("Copy failed: {}", e))?;
    Ok(filename)
}

#[tauri::command]
pub fn list_imported_fonts() -> Result<Vec<String>, String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    if !fonts_dir.exists() {
        return Ok(vec![]);
    }
    let mut names = vec![];
    for entry in std::fs::read_dir(&fonts_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let ext = std::path::Path::new(&name)
            .extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext == "ttf" || ext == "otf" {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

#[tauri::command]
pub fn delete_imported_font(filename: String) -> Result<(), String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    let path = fonts_dir.join(&filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Phase 3: Drop shadow / glow / mask / clip ────────────────────────────────

fn hex_to_rgba8(hex: &str) -> [u8; 4] {
    let h = hex.trim_start_matches('#');
    if h.len() == 6 {
        let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(0);
        [r, g, b, 255]
    } else {
        [0, 0, 0, 255]
    }
}

#[tauri::command]
pub fn update_layer_shadow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    color_hex: Option<String>,
    blur: Option<u32>,
    distance: Option<u32>,
    angle: Option<f32>,
    opacity: Option<u8>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.shadow.enabled = enabled;
    if let Some(c) = color_hex { layer.shadow.color = hex_to_rgba8(&c); }
    if let Some(v) = blur { layer.shadow.blur = v; }
    if let Some(v) = distance { layer.shadow.distance = v; }
    if let Some(v) = angle { layer.shadow.angle = v; }
    if let Some(v) = opacity { layer.shadow.opacity = v; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_glow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    kind: String, // "outer" | "inner"
    enabled: bool,
    color_hex: Option<String>,
    intensity: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    let glow = if kind == "inner" { &mut layer.inner_glow } else { &mut layer.outer_glow };
    glow.enabled = enabled;
    if let Some(c) = color_hex { glow.color = hex_to_rgba8(&c); }
    if let Some(v) = intensity { glow.intensity = v.clamp(1, 60); }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_mask(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    mask_shape: Option<String>, // None | "circle" | "rounded" | "star"
    mask_radius: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.mask_shape = mask_shape;
    if let Some(r) = mask_radius { layer.mask_radius = r; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn set_clip_to_below(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.clip_to_below = enabled;
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced text ───────────────────────────────────────────────────

#[tauri::command]
pub fn update_text_advanced(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    underline: Option<bool>,
    gradient_from_hex: Option<String>,
    gradient_to_hex: Option<String>,
    gradient_angle: Option<f32>,
    curve_amount: Option<f32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    match &mut layer.kind {
        LayerKind::Text3D {
            underline: ul,
            gradient_colors: gc,
            gradient_angle: ga,
            curve_amount: ca,
            ..
        } => {
            if let Some(v) = underline { *ul = v; }
            if let Some(v) = curve_amount { *ca = v.clamp(-180.0, 180.0); }
            if let Some(v) = gradient_angle { *ga = v; }
            // If either gradient color given, build/update gradient_colors pair
            if gradient_from_hex.is_some() || gradient_to_hex.is_some() {
                let cur = gc.unwrap_or([[255,255,255,255],[255,255,0,255]]);
                let from = gradient_from_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[0]);
                let to   = gradient_to_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[1]);
                *gc = Some([from, to]);
            } else if gradient_from_hex.is_none() && gradient_to_hex.is_none()
                && underline.is_none() && curve_amount.is_none() && gradient_angle.is_none()
            {
                // Clear gradient
                *gc = None;
            }
        }
        _ => return Err("Layer is not a Text layer.".into()),
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced shapes ─────────────────────────────────────────────────

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn add_shape_advanced(
    state: tauri::State<'_, SharedProjectState>,
    shape_type: String,
    width: u32,
    height: u32,
    fill_hex: String,
    border_radius: Option<u32>,
    border_hex: Option<String>,
    border_width: Option<u32>,
    sides: Option<u32>,
    fill_gradient_from: Option<String>,
    fill_gradient_to: Option<String>,
    fill_gradient_angle: Option<f32>,
    x: Option<i64>,
    y: Option<i64>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let cw = s.current.canvas_width as i64;
    let ch = s.current.canvas_height as i64;
    let lx = x.unwrap_or((cw - width as i64) / 2);
    let ly = y.unwrap_or((ch - height as i64) / 2);
    let fill_color = parse_hex_color(&fill_hex);
    let b_color = border_hex.as_deref().map(parse_hex_color).unwrap_or([255,255,255,255]);
    let fill_gradient = match (fill_gradient_from.as_deref(), fill_gradient_to.as_deref()) {
        (Some(a), Some(b)) => Some(BgGradient {
            from: hex_to_rgba8(a),
            to: hex_to_rgba8(b),
            angle: fill_gradient_angle.unwrap_or(0.0),
        }),
        _ => None,
    };
    let id = format!("layer_{}", uuid_v4());
    let name = match shape_type.as_str() {
        "line"     => "Line",
        "arrow"    => "Arrow",
        "triangle" => "Triangle",
        "star"     => "Star",
        "polygon"  => "Polygon",
        "circle"   => "Circle",
        "badge"    => "Badge",
        _          => "Rectangle",
    }.to_string();
    let layer = Layer {
        id: id.clone(),
        name,
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
            shape_type: shape_type.clone(),
            width,
            height,
            fill_color,
            border_radius: border_radius.unwrap_or(0),
            border_color: b_color,
            border_width: border_width.unwrap_or(0),
            sides: sides.unwrap_or(5),
            corner_radii: None,
            fill_gradient,
        },
    };
    s.current.layers.push(layer);
    render_and_respond(&mut s)
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_micros();
    format!("{:x}", ms ^ (ms >> 16))
}

#[tauri::command]
pub fn add_gradient_layer(
    state: tauri::State<'_, SharedProjectState>,
    width: u32,
    height: u32,
    from_hex: String,
    to_hex: String,
    angle: f32,
    x: i64,
    y: i64,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
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
            from_color: parse_hex_color(&from_hex),
            to_color: parse_hex_color(&to_hex),
            angle,
        },
    });
    render_and_respond(&mut s)
}

// ── Phase 3: Font management ─────────────────────────────────────────────────

#[tauri::command]
pub fn import_font(source_path: String) -> Result<String, String> {
    let p = std::path::Path::new(&source_path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext != "ttf" && ext != "otf" {
        return Err("Only .ttf and .otf font files are supported.".into());
    }
    let filename = p.file_name().and_then(|n| n.to_str())
        .ok_or("Invalid file name")?
        .to_string();
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    std::fs::create_dir_all(&fonts_dir).map_err(|e| e.to_string())?;
    let dest = fonts_dir.join(&filename);
    std::fs::copy(&source_path, &dest).map_err(|e| format!("Copy failed: {}", e))?;
    Ok(filename)
}

#[tauri::command]
pub fn list_imported_fonts() -> Result<Vec<String>, String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    if !fonts_dir.exists() {
        return Ok(vec![]);
    }
    let mut names = vec![];
    for entry in std::fs::read_dir(&fonts_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let ext = std::path::Path::new(&name)
            .extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext == "ttf" || ext == "otf" {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

#[tauri::command]
pub fn delete_imported_font(filename: String) -> Result<(), String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    let path = fonts_dir.join(&filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Phase 3: Drop shadow / glow / mask / clip ────────────────────────────────

fn hex_to_rgba8(hex: &str) -> [u8; 4] {
    let h = hex.trim_start_matches('#');
    if h.len() == 6 {
        let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(0);
        [r, g, b, 255]
    } else {
        [0, 0, 0, 255]
    }
}

#[tauri::command]
pub fn update_layer_shadow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    color_hex: Option<String>,
    blur: Option<u32>,
    distance: Option<u32>,
    angle: Option<f32>,
    opacity: Option<u8>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.shadow.enabled = enabled;
    if let Some(c) = color_hex { layer.shadow.color = hex_to_rgba8(&c); }
    if let Some(v) = blur { layer.shadow.blur = v; }
    if let Some(v) = distance { layer.shadow.distance = v; }
    if let Some(v) = angle { layer.shadow.angle = v; }
    if let Some(v) = opacity { layer.shadow.opacity = v; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_glow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    kind: String, // "outer" | "inner"
    enabled: bool,
    color_hex: Option<String>,
    intensity: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    let glow = if kind == "inner" { &mut layer.inner_glow } else { &mut layer.outer_glow };
    glow.enabled = enabled;
    if let Some(c) = color_hex { glow.color = hex_to_rgba8(&c); }
    if let Some(v) = intensity { glow.intensity = v.clamp(1, 60); }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_mask(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    mask_shape: Option<String>, // None | "circle" | "rounded" | "star"
    mask_radius: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.mask_shape = mask_shape;
    if let Some(r) = mask_radius { layer.mask_radius = r; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn set_clip_to_below(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.clip_to_below = enabled;
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced text ───────────────────────────────────────────────────

#[tauri::command]
pub fn update_text_advanced(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    underline: Option<bool>,
    gradient_from_hex: Option<String>,
    gradient_to_hex: Option<String>,
    gradient_angle: Option<f32>,
    curve_amount: Option<f32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    match &mut layer.kind {
        LayerKind::Text3D {
            underline: ul,
            gradient_colors: gc,
            gradient_angle: ga,
            curve_amount: ca,
            ..
        } => {
            if let Some(v) = underline { *ul = v; }
            if let Some(v) = curve_amount { *ca = v.clamp(-180.0, 180.0); }
            if let Some(v) = gradient_angle { *ga = v; }
            // If either gradient color given, build/update gradient_colors pair
            if gradient_from_hex.is_some() || gradient_to_hex.is_some() {
                let cur = gc.unwrap_or([[255,255,255,255],[255,255,0,255]]);
                let from = gradient_from_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[0]);
                let to   = gradient_to_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[1]);
                *gc = Some([from, to]);
            } else if gradient_from_hex.is_none() && gradient_to_hex.is_none()
                && underline.is_none() && curve_amount.is_none() && gradient_angle.is_none()
            {
                // Clear gradient
                *gc = None;
            }
        }
        _ => return Err("Layer is not a Text layer.".into()),
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced shapes ─────────────────────────────────────────────────

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn add_shape_advanced(
    state: tauri::State<'_, SharedProjectState>,
    shape_type: String,
    width: u32,
    height: u32,
    fill_hex: String,
    border_radius: Option<u32>,
    border_hex: Option<String>,
    border_width: Option<u32>,
    sides: Option<u32>,
    fill_gradient_from: Option<String>,
    fill_gradient_to: Option<String>,
    fill_gradient_angle: Option<f32>,
    x: Option<i64>,
    y: Option<i64>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let cw = s.current.canvas_width as i64;
    let ch = s.current.canvas_height as i64;
    let lx = x.unwrap_or((cw - width as i64) / 2);
    let ly = y.unwrap_or((ch - height as i64) / 2);
    let fill_color = parse_hex_color(&fill_hex);
    let b_color = border_hex.as_deref().map(parse_hex_color).unwrap_or([255,255,255,255]);
    let fill_gradient = match (fill_gradient_from.as_deref(), fill_gradient_to.as_deref()) {
        (Some(a), Some(b)) => Some(BgGradient {
            from: hex_to_rgba8(a),
            to: hex_to_rgba8(b),
            angle: fill_gradient_angle.unwrap_or(0.0),
        }),
        _ => None,
    };
    let id = format!("layer_{}", uuid_v4());
    let name = match shape_type.as_str() {
        "line"     => "Line",
        "arrow"    => "Arrow",
        "triangle" => "Triangle",
        "star"     => "Star",
        "polygon"  => "Polygon",
        "circle"   => "Circle",
        "badge"    => "Badge",
        _          => "Rectangle",
    }.to_string();
    let layer = Layer {
        id: id.clone(),
        name,
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
            shape_type: shape_type.clone(),
            width,
            height,
            fill_color,
            border_radius: border_radius.unwrap_or(0),
            border_color: b_color,
            border_width: border_width.unwrap_or(0),
            sides: sides.unwrap_or(5),
            corner_radii: None,
            fill_gradient,
        },
    };
    s.current.layers.push(layer);
    render_and_respond(&mut s)
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_micros();
    format!("{:x}", ms ^ (ms >> 16))
}

#[tauri::command]
pub fn add_watermark_layer(
    state: tauri::State<'_, SharedProjectState>,
    text: String,
    opacity: f32,
    position: String,
    font_style: Option<String>,
    color_hex: Option<String>,
    size: Option<f32>,
) -> Result<ProjectResponse, String> {
    if text.trim().is_empty() {
        return Err("Watermark text cannot be empty.".to_string());
    }
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let id = s.next_id();
    let cw = s.current.canvas_width;
    let ch = s.current.canvas_height;
    let font = font_style.unwrap_or_else(|| "classic_serif".to_string());
    let color = parse_hex_color(color_hex.as_deref().unwrap_or("#FFFFFF80"));
    let sz = size.unwrap_or(28.0).max(8.0);
    s.current.layers.push(Layer {
        id,
        name: format!("Watermark: {}", &text.chars().take(14).collect::<String>()),
        visible: true,
        locked: false,
        opacity: opacity.clamp(0.05, 1.0),
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
            font_style: font,
            color,
            size: sz,
            position,
            opacity: opacity.clamp(0.05, 1.0),
            canvas_width: cw,
            canvas_height: ch,
        },
    });
    render_and_respond(&mut s)
}

// ── Phase 3: Font management ─────────────────────────────────────────────────

#[tauri::command]
pub fn import_font(source_path: String) -> Result<String, String> {
    let p = std::path::Path::new(&source_path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext != "ttf" && ext != "otf" {
        return Err("Only .ttf and .otf font files are supported.".into());
    }
    let filename = p.file_name().and_then(|n| n.to_str())
        .ok_or("Invalid file name")?
        .to_string();
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    std::fs::create_dir_all(&fonts_dir).map_err(|e| e.to_string())?;
    let dest = fonts_dir.join(&filename);
    std::fs::copy(&source_path, &dest).map_err(|e| format!("Copy failed: {}", e))?;
    Ok(filename)
}

#[tauri::command]
pub fn list_imported_fonts() -> Result<Vec<String>, String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    if !fonts_dir.exists() {
        return Ok(vec![]);
    }
    let mut names = vec![];
    for entry in std::fs::read_dir(&fonts_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let ext = std::path::Path::new(&name)
            .extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext == "ttf" || ext == "otf" {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

#[tauri::command]
pub fn delete_imported_font(filename: String) -> Result<(), String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    let path = fonts_dir.join(&filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Phase 3: Drop shadow / glow / mask / clip ────────────────────────────────

fn hex_to_rgba8(hex: &str) -> [u8; 4] {
    let h = hex.trim_start_matches('#');
    if h.len() == 6 {
        let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(0);
        [r, g, b, 255]
    } else {
        [0, 0, 0, 255]
    }
}

#[tauri::command]
pub fn update_layer_shadow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    color_hex: Option<String>,
    blur: Option<u32>,
    distance: Option<u32>,
    angle: Option<f32>,
    opacity: Option<u8>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.shadow.enabled = enabled;
    if let Some(c) = color_hex { layer.shadow.color = hex_to_rgba8(&c); }
    if let Some(v) = blur { layer.shadow.blur = v; }
    if let Some(v) = distance { layer.shadow.distance = v; }
    if let Some(v) = angle { layer.shadow.angle = v; }
    if let Some(v) = opacity { layer.shadow.opacity = v; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_glow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    kind: String, // "outer" | "inner"
    enabled: bool,
    color_hex: Option<String>,
    intensity: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    let glow = if kind == "inner" { &mut layer.inner_glow } else { &mut layer.outer_glow };
    glow.enabled = enabled;
    if let Some(c) = color_hex { glow.color = hex_to_rgba8(&c); }
    if let Some(v) = intensity { glow.intensity = v.clamp(1, 60); }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_mask(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    mask_shape: Option<String>, // None | "circle" | "rounded" | "star"
    mask_radius: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.mask_shape = mask_shape;
    if let Some(r) = mask_radius { layer.mask_radius = r; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn set_clip_to_below(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.clip_to_below = enabled;
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced text ───────────────────────────────────────────────────

#[tauri::command]
pub fn update_text_advanced(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    underline: Option<bool>,
    gradient_from_hex: Option<String>,
    gradient_to_hex: Option<String>,
    gradient_angle: Option<f32>,
    curve_amount: Option<f32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    match &mut layer.kind {
        LayerKind::Text3D {
            underline: ul,
            gradient_colors: gc,
            gradient_angle: ga,
            curve_amount: ca,
            ..
        } => {
            if let Some(v) = underline { *ul = v; }
            if let Some(v) = curve_amount { *ca = v.clamp(-180.0, 180.0); }
            if let Some(v) = gradient_angle { *ga = v; }
            // If either gradient color given, build/update gradient_colors pair
            if gradient_from_hex.is_some() || gradient_to_hex.is_some() {
                let cur = gc.unwrap_or([[255,255,255,255],[255,255,0,255]]);
                let from = gradient_from_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[0]);
                let to   = gradient_to_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[1]);
                *gc = Some([from, to]);
            } else if gradient_from_hex.is_none() && gradient_to_hex.is_none()
                && underline.is_none() && curve_amount.is_none() && gradient_angle.is_none()
            {
                // Clear gradient
                *gc = None;
            }
        }
        _ => return Err("Layer is not a Text layer.".into()),
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced shapes ─────────────────────────────────────────────────

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn add_shape_advanced(
    state: tauri::State<'_, SharedProjectState>,
    shape_type: String,
    width: u32,
    height: u32,
    fill_hex: String,
    border_radius: Option<u32>,
    border_hex: Option<String>,
    border_width: Option<u32>,
    sides: Option<u32>,
    fill_gradient_from: Option<String>,
    fill_gradient_to: Option<String>,
    fill_gradient_angle: Option<f32>,
    x: Option<i64>,
    y: Option<i64>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let cw = s.current.canvas_width as i64;
    let ch = s.current.canvas_height as i64;
    let lx = x.unwrap_or((cw - width as i64) / 2);
    let ly = y.unwrap_or((ch - height as i64) / 2);
    let fill_color = parse_hex_color(&fill_hex);
    let b_color = border_hex.as_deref().map(parse_hex_color).unwrap_or([255,255,255,255]);
    let fill_gradient = match (fill_gradient_from.as_deref(), fill_gradient_to.as_deref()) {
        (Some(a), Some(b)) => Some(BgGradient {
            from: hex_to_rgba8(a),
            to: hex_to_rgba8(b),
            angle: fill_gradient_angle.unwrap_or(0.0),
        }),
        _ => None,
    };
    let id = format!("layer_{}", uuid_v4());
    let name = match shape_type.as_str() {
        "line"     => "Line",
        "arrow"    => "Arrow",
        "triangle" => "Triangle",
        "star"     => "Star",
        "polygon"  => "Polygon",
        "circle"   => "Circle",
        "badge"    => "Badge",
        _          => "Rectangle",
    }.to_string();
    let layer = Layer {
        id: id.clone(),
        name,
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
            shape_type: shape_type.clone(),
            width,
            height,
            fill_color,
            border_radius: border_radius.unwrap_or(0),
            border_color: b_color,
            border_width: border_width.unwrap_or(0),
            sides: sides.unwrap_or(5),
            corner_radii: None,
            fill_gradient,
        },
    };
    s.current.layers.push(layer);
    render_and_respond(&mut s)
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_micros();
    format!("{:x}", ms ^ (ms >> 16))
}

#[tauri::command]
pub fn set_canvas_background(
    state: tauri::State<'_, SharedProjectState>,
    color_hex: Option<String>,
    from_hex: Option<String>,
    to_hex: Option<String>,
    angle: Option<f32>,
    transparent: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    if transparent == Some(true) {
        s.current.transparent_bg = true;
        s.current.bg_gradient = None;
    } else if let (Some(fh), Some(th)) = (from_hex.as_deref(), to_hex.as_deref()) {
        s.current.transparent_bg = false;
        s.current.bg_gradient = Some(BgGradient {
            from_color: parse_hex_color(fh),
            to_color: parse_hex_color(th),
            angle: angle.unwrap_or(0.0),
        });
    } else if let Some(hex) = color_hex.as_deref() {
        s.current.transparent_bg = false;
        s.current.bg_color = parse_hex_color(hex);
        s.current.bg_gradient = None;
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Font management ─────────────────────────────────────────────────

#[tauri::command]
pub fn import_font(source_path: String) -> Result<String, String> {
    let p = std::path::Path::new(&source_path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext != "ttf" && ext != "otf" {
        return Err("Only .ttf and .otf font files are supported.".into());
    }
    let filename = p.file_name().and_then(|n| n.to_str())
        .ok_or("Invalid file name")?
        .to_string();
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    std::fs::create_dir_all(&fonts_dir).map_err(|e| e.to_string())?;
    let dest = fonts_dir.join(&filename);
    std::fs::copy(&source_path, &dest).map_err(|e| format!("Copy failed: {}", e))?;
    Ok(filename)
}

#[tauri::command]
pub fn list_imported_fonts() -> Result<Vec<String>, String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    if !fonts_dir.exists() {
        return Ok(vec![]);
    }
    let mut names = vec![];
    for entry in std::fs::read_dir(&fonts_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let ext = std::path::Path::new(&name)
            .extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext == "ttf" || ext == "otf" {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

#[tauri::command]
pub fn delete_imported_font(filename: String) -> Result<(), String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    let path = fonts_dir.join(&filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Phase 3: Drop shadow / glow / mask / clip ────────────────────────────────

fn hex_to_rgba8(hex: &str) -> [u8; 4] {
    let h = hex.trim_start_matches('#');
    if h.len() == 6 {
        let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(0);
        [r, g, b, 255]
    } else {
        [0, 0, 0, 255]
    }
}

#[tauri::command]
pub fn update_layer_shadow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    color_hex: Option<String>,
    blur: Option<u32>,
    distance: Option<u32>,
    angle: Option<f32>,
    opacity: Option<u8>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.shadow.enabled = enabled;
    if let Some(c) = color_hex { layer.shadow.color = hex_to_rgba8(&c); }
    if let Some(v) = blur { layer.shadow.blur = v; }
    if let Some(v) = distance { layer.shadow.distance = v; }
    if let Some(v) = angle { layer.shadow.angle = v; }
    if let Some(v) = opacity { layer.shadow.opacity = v; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_glow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    kind: String, // "outer" | "inner"
    enabled: bool,
    color_hex: Option<String>,
    intensity: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    let glow = if kind == "inner" { &mut layer.inner_glow } else { &mut layer.outer_glow };
    glow.enabled = enabled;
    if let Some(c) = color_hex { glow.color = hex_to_rgba8(&c); }
    if let Some(v) = intensity { glow.intensity = v.clamp(1, 60); }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_mask(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    mask_shape: Option<String>, // None | "circle" | "rounded" | "star"
    mask_radius: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.mask_shape = mask_shape;
    if let Some(r) = mask_radius { layer.mask_radius = r; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn set_clip_to_below(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.clip_to_below = enabled;
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced text ───────────────────────────────────────────────────

#[tauri::command]
pub fn update_text_advanced(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    underline: Option<bool>,
    gradient_from_hex: Option<String>,
    gradient_to_hex: Option<String>,
    gradient_angle: Option<f32>,
    curve_amount: Option<f32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    match &mut layer.kind {
        LayerKind::Text3D {
            underline: ul,
            gradient_colors: gc,
            gradient_angle: ga,
            curve_amount: ca,
            ..
        } => {
            if let Some(v) = underline { *ul = v; }
            if let Some(v) = curve_amount { *ca = v.clamp(-180.0, 180.0); }
            if let Some(v) = gradient_angle { *ga = v; }
            // If either gradient color given, build/update gradient_colors pair
            if gradient_from_hex.is_some() || gradient_to_hex.is_some() {
                let cur = gc.unwrap_or([[255,255,255,255],[255,255,0,255]]);
                let from = gradient_from_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[0]);
                let to   = gradient_to_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[1]);
                *gc = Some([from, to]);
            } else if gradient_from_hex.is_none() && gradient_to_hex.is_none()
                && underline.is_none() && curve_amount.is_none() && gradient_angle.is_none()
            {
                // Clear gradient
                *gc = None;
            }
        }
        _ => return Err("Layer is not a Text layer.".into()),
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced shapes ─────────────────────────────────────────────────

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn add_shape_advanced(
    state: tauri::State<'_, SharedProjectState>,
    shape_type: String,
    width: u32,
    height: u32,
    fill_hex: String,
    border_radius: Option<u32>,
    border_hex: Option<String>,
    border_width: Option<u32>,
    sides: Option<u32>,
    fill_gradient_from: Option<String>,
    fill_gradient_to: Option<String>,
    fill_gradient_angle: Option<f32>,
    x: Option<i64>,
    y: Option<i64>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let cw = s.current.canvas_width as i64;
    let ch = s.current.canvas_height as i64;
    let lx = x.unwrap_or((cw - width as i64) / 2);
    let ly = y.unwrap_or((ch - height as i64) / 2);
    let fill_color = parse_hex_color(&fill_hex);
    let b_color = border_hex.as_deref().map(parse_hex_color).unwrap_or([255,255,255,255]);
    let fill_gradient = match (fill_gradient_from.as_deref(), fill_gradient_to.as_deref()) {
        (Some(a), Some(b)) => Some(BgGradient {
            from: hex_to_rgba8(a),
            to: hex_to_rgba8(b),
            angle: fill_gradient_angle.unwrap_or(0.0),
        }),
        _ => None,
    };
    let id = format!("layer_{}", uuid_v4());
    let name = match shape_type.as_str() {
        "line"     => "Line",
        "arrow"    => "Arrow",
        "triangle" => "Triangle",
        "star"     => "Star",
        "polygon"  => "Polygon",
        "circle"   => "Circle",
        "badge"    => "Badge",
        _          => "Rectangle",
    }.to_string();
    let layer = Layer {
        id: id.clone(),
        name,
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
            shape_type: shape_type.clone(),
            width,
            height,
            fill_color,
            border_radius: border_radius.unwrap_or(0),
            border_color: b_color,
            border_width: border_width.unwrap_or(0),
            sides: sides.unwrap_or(5),
            corner_radii: None,
            fill_gradient,
        },
    };
    s.current.layers.push(layer);
    render_and_respond(&mut s)
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_micros();
    format!("{:x}", ms ^ (ms >> 16))
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
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) {
        s.push_undo();
    }
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

// ── Phase 3: Font management ─────────────────────────────────────────────────

#[tauri::command]
pub fn import_font(source_path: String) -> Result<String, String> {
    let p = std::path::Path::new(&source_path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext != "ttf" && ext != "otf" {
        return Err("Only .ttf and .otf font files are supported.".into());
    }
    let filename = p.file_name().and_then(|n| n.to_str())
        .ok_or("Invalid file name")?
        .to_string();
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    std::fs::create_dir_all(&fonts_dir).map_err(|e| e.to_string())?;
    let dest = fonts_dir.join(&filename);
    std::fs::copy(&source_path, &dest).map_err(|e| format!("Copy failed: {}", e))?;
    Ok(filename)
}

#[tauri::command]
pub fn list_imported_fonts() -> Result<Vec<String>, String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    if !fonts_dir.exists() {
        return Ok(vec![]);
    }
    let mut names = vec![];
    for entry in std::fs::read_dir(&fonts_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let ext = std::path::Path::new(&name)
            .extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext == "ttf" || ext == "otf" {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

#[tauri::command]
pub fn delete_imported_font(filename: String) -> Result<(), String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    let path = fonts_dir.join(&filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Phase 3: Drop shadow / glow / mask / clip ────────────────────────────────

fn hex_to_rgba8(hex: &str) -> [u8; 4] {
    let h = hex.trim_start_matches('#');
    if h.len() == 6 {
        let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(0);
        [r, g, b, 255]
    } else {
        [0, 0, 0, 255]
    }
}

#[tauri::command]
pub fn update_layer_shadow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    color_hex: Option<String>,
    blur: Option<u32>,
    distance: Option<u32>,
    angle: Option<f32>,
    opacity: Option<u8>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.shadow.enabled = enabled;
    if let Some(c) = color_hex { layer.shadow.color = hex_to_rgba8(&c); }
    if let Some(v) = blur { layer.shadow.blur = v; }
    if let Some(v) = distance { layer.shadow.distance = v; }
    if let Some(v) = angle { layer.shadow.angle = v; }
    if let Some(v) = opacity { layer.shadow.opacity = v; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_glow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    kind: String, // "outer" | "inner"
    enabled: bool,
    color_hex: Option<String>,
    intensity: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    let glow = if kind == "inner" { &mut layer.inner_glow } else { &mut layer.outer_glow };
    glow.enabled = enabled;
    if let Some(c) = color_hex { glow.color = hex_to_rgba8(&c); }
    if let Some(v) = intensity { glow.intensity = v.clamp(1, 60); }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_mask(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    mask_shape: Option<String>, // None | "circle" | "rounded" | "star"
    mask_radius: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.mask_shape = mask_shape;
    if let Some(r) = mask_radius { layer.mask_radius = r; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn set_clip_to_below(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.clip_to_below = enabled;
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced text ───────────────────────────────────────────────────

#[tauri::command]
pub fn update_text_advanced(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    underline: Option<bool>,
    gradient_from_hex: Option<String>,
    gradient_to_hex: Option<String>,
    gradient_angle: Option<f32>,
    curve_amount: Option<f32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    match &mut layer.kind {
        LayerKind::Text3D {
            underline: ul,
            gradient_colors: gc,
            gradient_angle: ga,
            curve_amount: ca,
            ..
        } => {
            if let Some(v) = underline { *ul = v; }
            if let Some(v) = curve_amount { *ca = v.clamp(-180.0, 180.0); }
            if let Some(v) = gradient_angle { *ga = v; }
            // If either gradient color given, build/update gradient_colors pair
            if gradient_from_hex.is_some() || gradient_to_hex.is_some() {
                let cur = gc.unwrap_or([[255,255,255,255],[255,255,0,255]]);
                let from = gradient_from_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[0]);
                let to   = gradient_to_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[1]);
                *gc = Some([from, to]);
            } else if gradient_from_hex.is_none() && gradient_to_hex.is_none()
                && underline.is_none() && curve_amount.is_none() && gradient_angle.is_none()
            {
                // Clear gradient
                *gc = None;
            }
        }
        _ => return Err("Layer is not a Text layer.".into()),
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced shapes ─────────────────────────────────────────────────

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn add_shape_advanced(
    state: tauri::State<'_, SharedProjectState>,
    shape_type: String,
    width: u32,
    height: u32,
    fill_hex: String,
    border_radius: Option<u32>,
    border_hex: Option<String>,
    border_width: Option<u32>,
    sides: Option<u32>,
    fill_gradient_from: Option<String>,
    fill_gradient_to: Option<String>,
    fill_gradient_angle: Option<f32>,
    x: Option<i64>,
    y: Option<i64>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let cw = s.current.canvas_width as i64;
    let ch = s.current.canvas_height as i64;
    let lx = x.unwrap_or((cw - width as i64) / 2);
    let ly = y.unwrap_or((ch - height as i64) / 2);
    let fill_color = parse_hex_color(&fill_hex);
    let b_color = border_hex.as_deref().map(parse_hex_color).unwrap_or([255,255,255,255]);
    let fill_gradient = match (fill_gradient_from.as_deref(), fill_gradient_to.as_deref()) {
        (Some(a), Some(b)) => Some(BgGradient {
            from: hex_to_rgba8(a),
            to: hex_to_rgba8(b),
            angle: fill_gradient_angle.unwrap_or(0.0),
        }),
        _ => None,
    };
    let id = format!("layer_{}", uuid_v4());
    let name = match shape_type.as_str() {
        "line"     => "Line",
        "arrow"    => "Arrow",
        "triangle" => "Triangle",
        "star"     => "Star",
        "polygon"  => "Polygon",
        "circle"   => "Circle",
        "badge"    => "Badge",
        _          => "Rectangle",
    }.to_string();
    let layer = Layer {
        id: id.clone(),
        name,
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
            shape_type: shape_type.clone(),
            width,
            height,
            fill_color,
            border_radius: border_radius.unwrap_or(0),
            border_color: b_color,
            border_width: border_width.unwrap_or(0),
            sides: sides.unwrap_or(5),
            corner_radii: None,
            fill_gradient,
        },
    };
    s.current.layers.push(layer);
    render_and_respond(&mut s)
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_micros();
    format!("{:x}", ms ^ (ms >> 16))
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
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) {
        s.push_undo();
    }
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

// ── Phase 3: Font management ─────────────────────────────────────────────────

#[tauri::command]
pub fn import_font(source_path: String) -> Result<String, String> {
    let p = std::path::Path::new(&source_path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext != "ttf" && ext != "otf" {
        return Err("Only .ttf and .otf font files are supported.".into());
    }
    let filename = p.file_name().and_then(|n| n.to_str())
        .ok_or("Invalid file name")?
        .to_string();
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    std::fs::create_dir_all(&fonts_dir).map_err(|e| e.to_string())?;
    let dest = fonts_dir.join(&filename);
    std::fs::copy(&source_path, &dest).map_err(|e| format!("Copy failed: {}", e))?;
    Ok(filename)
}

#[tauri::command]
pub fn list_imported_fonts() -> Result<Vec<String>, String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    if !fonts_dir.exists() {
        return Ok(vec![]);
    }
    let mut names = vec![];
    for entry in std::fs::read_dir(&fonts_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let ext = std::path::Path::new(&name)
            .extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext == "ttf" || ext == "otf" {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

#[tauri::command]
pub fn delete_imported_font(filename: String) -> Result<(), String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    let path = fonts_dir.join(&filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Phase 3: Drop shadow / glow / mask / clip ────────────────────────────────

fn hex_to_rgba8(hex: &str) -> [u8; 4] {
    let h = hex.trim_start_matches('#');
    if h.len() == 6 {
        let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(0);
        [r, g, b, 255]
    } else {
        [0, 0, 0, 255]
    }
}

#[tauri::command]
pub fn update_layer_shadow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    color_hex: Option<String>,
    blur: Option<u32>,
    distance: Option<u32>,
    angle: Option<f32>,
    opacity: Option<u8>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.shadow.enabled = enabled;
    if let Some(c) = color_hex { layer.shadow.color = hex_to_rgba8(&c); }
    if let Some(v) = blur { layer.shadow.blur = v; }
    if let Some(v) = distance { layer.shadow.distance = v; }
    if let Some(v) = angle { layer.shadow.angle = v; }
    if let Some(v) = opacity { layer.shadow.opacity = v; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_glow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    kind: String, // "outer" | "inner"
    enabled: bool,
    color_hex: Option<String>,
    intensity: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    let glow = if kind == "inner" { &mut layer.inner_glow } else { &mut layer.outer_glow };
    glow.enabled = enabled;
    if let Some(c) = color_hex { glow.color = hex_to_rgba8(&c); }
    if let Some(v) = intensity { glow.intensity = v.clamp(1, 60); }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_mask(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    mask_shape: Option<String>, // None | "circle" | "rounded" | "star"
    mask_radius: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.mask_shape = mask_shape;
    if let Some(r) = mask_radius { layer.mask_radius = r; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn set_clip_to_below(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.clip_to_below = enabled;
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced text ───────────────────────────────────────────────────

#[tauri::command]
pub fn update_text_advanced(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    underline: Option<bool>,
    gradient_from_hex: Option<String>,
    gradient_to_hex: Option<String>,
    gradient_angle: Option<f32>,
    curve_amount: Option<f32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    match &mut layer.kind {
        LayerKind::Text3D {
            underline: ul,
            gradient_colors: gc,
            gradient_angle: ga,
            curve_amount: ca,
            ..
        } => {
            if let Some(v) = underline { *ul = v; }
            if let Some(v) = curve_amount { *ca = v.clamp(-180.0, 180.0); }
            if let Some(v) = gradient_angle { *ga = v; }
            // If either gradient color given, build/update gradient_colors pair
            if gradient_from_hex.is_some() || gradient_to_hex.is_some() {
                let cur = gc.unwrap_or([[255,255,255,255],[255,255,0,255]]);
                let from = gradient_from_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[0]);
                let to   = gradient_to_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[1]);
                *gc = Some([from, to]);
            } else if gradient_from_hex.is_none() && gradient_to_hex.is_none()
                && underline.is_none() && curve_amount.is_none() && gradient_angle.is_none()
            {
                // Clear gradient
                *gc = None;
            }
        }
        _ => return Err("Layer is not a Text layer.".into()),
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced shapes ─────────────────────────────────────────────────

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn add_shape_advanced(
    state: tauri::State<'_, SharedProjectState>,
    shape_type: String,
    width: u32,
    height: u32,
    fill_hex: String,
    border_radius: Option<u32>,
    border_hex: Option<String>,
    border_width: Option<u32>,
    sides: Option<u32>,
    fill_gradient_from: Option<String>,
    fill_gradient_to: Option<String>,
    fill_gradient_angle: Option<f32>,
    x: Option<i64>,
    y: Option<i64>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let cw = s.current.canvas_width as i64;
    let ch = s.current.canvas_height as i64;
    let lx = x.unwrap_or((cw - width as i64) / 2);
    let ly = y.unwrap_or((ch - height as i64) / 2);
    let fill_color = parse_hex_color(&fill_hex);
    let b_color = border_hex.as_deref().map(parse_hex_color).unwrap_or([255,255,255,255]);
    let fill_gradient = match (fill_gradient_from.as_deref(), fill_gradient_to.as_deref()) {
        (Some(a), Some(b)) => Some(BgGradient {
            from: hex_to_rgba8(a),
            to: hex_to_rgba8(b),
            angle: fill_gradient_angle.unwrap_or(0.0),
        }),
        _ => None,
    };
    let id = format!("layer_{}", uuid_v4());
    let name = match shape_type.as_str() {
        "line"     => "Line",
        "arrow"    => "Arrow",
        "triangle" => "Triangle",
        "star"     => "Star",
        "polygon"  => "Polygon",
        "circle"   => "Circle",
        "badge"    => "Badge",
        _          => "Rectangle",
    }.to_string();
    let layer = Layer {
        id: id.clone(),
        name,
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
            shape_type: shape_type.clone(),
            width,
            height,
            fill_color,
            border_radius: border_radius.unwrap_or(0),
            border_color: b_color,
            border_width: border_width.unwrap_or(0),
            sides: sides.unwrap_or(5),
            corner_radii: None,
            fill_gradient,
        },
    };
    s.current.layers.push(layer);
    render_and_respond(&mut s)
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_micros();
    format!("{:x}", ms ^ (ms >> 16))
}

#[tauri::command]
pub fn set_layer_outline(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    color_hex: String,
    thickness: u32,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) {
        s.push_undo();
    }
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

// ── Phase 3: Font management ─────────────────────────────────────────────────

#[tauri::command]
pub fn import_font(source_path: String) -> Result<String, String> {
    let p = std::path::Path::new(&source_path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext != "ttf" && ext != "otf" {
        return Err("Only .ttf and .otf font files are supported.".into());
    }
    let filename = p.file_name().and_then(|n| n.to_str())
        .ok_or("Invalid file name")?
        .to_string();
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    std::fs::create_dir_all(&fonts_dir).map_err(|e| e.to_string())?;
    let dest = fonts_dir.join(&filename);
    std::fs::copy(&source_path, &dest).map_err(|e| format!("Copy failed: {}", e))?;
    Ok(filename)
}

#[tauri::command]
pub fn list_imported_fonts() -> Result<Vec<String>, String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    if !fonts_dir.exists() {
        return Ok(vec![]);
    }
    let mut names = vec![];
    for entry in std::fs::read_dir(&fonts_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let ext = std::path::Path::new(&name)
            .extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext == "ttf" || ext == "otf" {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

#[tauri::command]
pub fn delete_imported_font(filename: String) -> Result<(), String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    let path = fonts_dir.join(&filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Phase 3: Drop shadow / glow / mask / clip ────────────────────────────────

fn hex_to_rgba8(hex: &str) -> [u8; 4] {
    let h = hex.trim_start_matches('#');
    if h.len() == 6 {
        let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(0);
        [r, g, b, 255]
    } else {
        [0, 0, 0, 255]
    }
}

#[tauri::command]
pub fn update_layer_shadow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    color_hex: Option<String>,
    blur: Option<u32>,
    distance: Option<u32>,
    angle: Option<f32>,
    opacity: Option<u8>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.shadow.enabled = enabled;
    if let Some(c) = color_hex { layer.shadow.color = hex_to_rgba8(&c); }
    if let Some(v) = blur { layer.shadow.blur = v; }
    if let Some(v) = distance { layer.shadow.distance = v; }
    if let Some(v) = angle { layer.shadow.angle = v; }
    if let Some(v) = opacity { layer.shadow.opacity = v; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_glow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    kind: String, // "outer" | "inner"
    enabled: bool,
    color_hex: Option<String>,
    intensity: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    let glow = if kind == "inner" { &mut layer.inner_glow } else { &mut layer.outer_glow };
    glow.enabled = enabled;
    if let Some(c) = color_hex { glow.color = hex_to_rgba8(&c); }
    if let Some(v) = intensity { glow.intensity = v.clamp(1, 60); }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_mask(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    mask_shape: Option<String>, // None | "circle" | "rounded" | "star"
    mask_radius: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.mask_shape = mask_shape;
    if let Some(r) = mask_radius { layer.mask_radius = r; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn set_clip_to_below(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.clip_to_below = enabled;
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced text ───────────────────────────────────────────────────

#[tauri::command]
pub fn update_text_advanced(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    underline: Option<bool>,
    gradient_from_hex: Option<String>,
    gradient_to_hex: Option<String>,
    gradient_angle: Option<f32>,
    curve_amount: Option<f32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    match &mut layer.kind {
        LayerKind::Text3D {
            underline: ul,
            gradient_colors: gc,
            gradient_angle: ga,
            curve_amount: ca,
            ..
        } => {
            if let Some(v) = underline { *ul = v; }
            if let Some(v) = curve_amount { *ca = v.clamp(-180.0, 180.0); }
            if let Some(v) = gradient_angle { *ga = v; }
            // If either gradient color given, build/update gradient_colors pair
            if gradient_from_hex.is_some() || gradient_to_hex.is_some() {
                let cur = gc.unwrap_or([[255,255,255,255],[255,255,0,255]]);
                let from = gradient_from_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[0]);
                let to   = gradient_to_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[1]);
                *gc = Some([from, to]);
            } else if gradient_from_hex.is_none() && gradient_to_hex.is_none()
                && underline.is_none() && curve_amount.is_none() && gradient_angle.is_none()
            {
                // Clear gradient
                *gc = None;
            }
        }
        _ => return Err("Layer is not a Text layer.".into()),
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced shapes ─────────────────────────────────────────────────

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn add_shape_advanced(
    state: tauri::State<'_, SharedProjectState>,
    shape_type: String,
    width: u32,
    height: u32,
    fill_hex: String,
    border_radius: Option<u32>,
    border_hex: Option<String>,
    border_width: Option<u32>,
    sides: Option<u32>,
    fill_gradient_from: Option<String>,
    fill_gradient_to: Option<String>,
    fill_gradient_angle: Option<f32>,
    x: Option<i64>,
    y: Option<i64>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let cw = s.current.canvas_width as i64;
    let ch = s.current.canvas_height as i64;
    let lx = x.unwrap_or((cw - width as i64) / 2);
    let ly = y.unwrap_or((ch - height as i64) / 2);
    let fill_color = parse_hex_color(&fill_hex);
    let b_color = border_hex.as_deref().map(parse_hex_color).unwrap_or([255,255,255,255]);
    let fill_gradient = match (fill_gradient_from.as_deref(), fill_gradient_to.as_deref()) {
        (Some(a), Some(b)) => Some(BgGradient {
            from: hex_to_rgba8(a),
            to: hex_to_rgba8(b),
            angle: fill_gradient_angle.unwrap_or(0.0),
        }),
        _ => None,
    };
    let id = format!("layer_{}", uuid_v4());
    let name = match shape_type.as_str() {
        "line"     => "Line",
        "arrow"    => "Arrow",
        "triangle" => "Triangle",
        "star"     => "Star",
        "polygon"  => "Polygon",
        "circle"   => "Circle",
        "badge"    => "Badge",
        _          => "Rectangle",
    }.to_string();
    let layer = Layer {
        id: id.clone(),
        name,
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
            shape_type: shape_type.clone(),
            width,
            height,
            fill_color,
            border_radius: border_radius.unwrap_or(0),
            border_color: b_color,
            border_width: border_width.unwrap_or(0),
            sides: sides.unwrap_or(5),
            corner_radii: None,
            fill_gradient,
        },
    };
    s.current.layers.push(layer);
    render_and_respond(&mut s)
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_micros();
    format!("{:x}", ms ^ (ms >> 16))
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

// ── Phase 3: Font management ─────────────────────────────────────────────────

#[tauri::command]
pub fn import_font(source_path: String) -> Result<String, String> {
    let p = std::path::Path::new(&source_path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext != "ttf" && ext != "otf" {
        return Err("Only .ttf and .otf font files are supported.".into());
    }
    let filename = p.file_name().and_then(|n| n.to_str())
        .ok_or("Invalid file name")?
        .to_string();
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    std::fs::create_dir_all(&fonts_dir).map_err(|e| e.to_string())?;
    let dest = fonts_dir.join(&filename);
    std::fs::copy(&source_path, &dest).map_err(|e| format!("Copy failed: {}", e))?;
    Ok(filename)
}

#[tauri::command]
pub fn list_imported_fonts() -> Result<Vec<String>, String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    if !fonts_dir.exists() {
        return Ok(vec![]);
    }
    let mut names = vec![];
    for entry in std::fs::read_dir(&fonts_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let ext = std::path::Path::new(&name)
            .extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext == "ttf" || ext == "otf" {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

#[tauri::command]
pub fn delete_imported_font(filename: String) -> Result<(), String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    let path = fonts_dir.join(&filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Phase 3: Drop shadow / glow / mask / clip ────────────────────────────────

fn hex_to_rgba8(hex: &str) -> [u8; 4] {
    let h = hex.trim_start_matches('#');
    if h.len() == 6 {
        let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(0);
        [r, g, b, 255]
    } else {
        [0, 0, 0, 255]
    }
}

#[tauri::command]
pub fn update_layer_shadow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    color_hex: Option<String>,
    blur: Option<u32>,
    distance: Option<u32>,
    angle: Option<f32>,
    opacity: Option<u8>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.shadow.enabled = enabled;
    if let Some(c) = color_hex { layer.shadow.color = hex_to_rgba8(&c); }
    if let Some(v) = blur { layer.shadow.blur = v; }
    if let Some(v) = distance { layer.shadow.distance = v; }
    if let Some(v) = angle { layer.shadow.angle = v; }
    if let Some(v) = opacity { layer.shadow.opacity = v; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_glow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    kind: String, // "outer" | "inner"
    enabled: bool,
    color_hex: Option<String>,
    intensity: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    let glow = if kind == "inner" { &mut layer.inner_glow } else { &mut layer.outer_glow };
    glow.enabled = enabled;
    if let Some(c) = color_hex { glow.color = hex_to_rgba8(&c); }
    if let Some(v) = intensity { glow.intensity = v.clamp(1, 60); }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_mask(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    mask_shape: Option<String>, // None | "circle" | "rounded" | "star"
    mask_radius: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.mask_shape = mask_shape;
    if let Some(r) = mask_radius { layer.mask_radius = r; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn set_clip_to_below(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.clip_to_below = enabled;
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced text ───────────────────────────────────────────────────

#[tauri::command]
pub fn update_text_advanced(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    underline: Option<bool>,
    gradient_from_hex: Option<String>,
    gradient_to_hex: Option<String>,
    gradient_angle: Option<f32>,
    curve_amount: Option<f32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    match &mut layer.kind {
        LayerKind::Text3D {
            underline: ul,
            gradient_colors: gc,
            gradient_angle: ga,
            curve_amount: ca,
            ..
        } => {
            if let Some(v) = underline { *ul = v; }
            if let Some(v) = curve_amount { *ca = v.clamp(-180.0, 180.0); }
            if let Some(v) = gradient_angle { *ga = v; }
            // If either gradient color given, build/update gradient_colors pair
            if gradient_from_hex.is_some() || gradient_to_hex.is_some() {
                let cur = gc.unwrap_or([[255,255,255,255],[255,255,0,255]]);
                let from = gradient_from_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[0]);
                let to   = gradient_to_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[1]);
                *gc = Some([from, to]);
            } else if gradient_from_hex.is_none() && gradient_to_hex.is_none()
                && underline.is_none() && curve_amount.is_none() && gradient_angle.is_none()
            {
                // Clear gradient
                *gc = None;
            }
        }
        _ => return Err("Layer is not a Text layer.".into()),
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced shapes ─────────────────────────────────────────────────

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn add_shape_advanced(
    state: tauri::State<'_, SharedProjectState>,
    shape_type: String,
    width: u32,
    height: u32,
    fill_hex: String,
    border_radius: Option<u32>,
    border_hex: Option<String>,
    border_width: Option<u32>,
    sides: Option<u32>,
    fill_gradient_from: Option<String>,
    fill_gradient_to: Option<String>,
    fill_gradient_angle: Option<f32>,
    x: Option<i64>,
    y: Option<i64>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let cw = s.current.canvas_width as i64;
    let ch = s.current.canvas_height as i64;
    let lx = x.unwrap_or((cw - width as i64) / 2);
    let ly = y.unwrap_or((ch - height as i64) / 2);
    let fill_color = parse_hex_color(&fill_hex);
    let b_color = border_hex.as_deref().map(parse_hex_color).unwrap_or([255,255,255,255]);
    let fill_gradient = match (fill_gradient_from.as_deref(), fill_gradient_to.as_deref()) {
        (Some(a), Some(b)) => Some(BgGradient {
            from: hex_to_rgba8(a),
            to: hex_to_rgba8(b),
            angle: fill_gradient_angle.unwrap_or(0.0),
        }),
        _ => None,
    };
    let id = format!("layer_{}", uuid_v4());
    let name = match shape_type.as_str() {
        "line"     => "Line",
        "arrow"    => "Arrow",
        "triangle" => "Triangle",
        "star"     => "Star",
        "polygon"  => "Polygon",
        "circle"   => "Circle",
        "badge"    => "Badge",
        _          => "Rectangle",
    }.to_string();
    let layer = Layer {
        id: id.clone(),
        name,
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
            shape_type: shape_type.clone(),
            width,
            height,
            fill_color,
            border_radius: border_radius.unwrap_or(0),
            border_color: b_color,
            border_width: border_width.unwrap_or(0),
            sides: sides.unwrap_or(5),
            corner_radii: None,
            fill_gradient,
        },
    };
    s.current.layers.push(layer);
    render_and_respond(&mut s)
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_micros();
    format!("{:x}", ms ^ (ms >> 16))
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

// ── Phase 3: Font management ─────────────────────────────────────────────────

#[tauri::command]
pub fn import_font(source_path: String) -> Result<String, String> {
    let p = std::path::Path::new(&source_path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext != "ttf" && ext != "otf" {
        return Err("Only .ttf and .otf font files are supported.".into());
    }
    let filename = p.file_name().and_then(|n| n.to_str())
        .ok_or("Invalid file name")?
        .to_string();
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    std::fs::create_dir_all(&fonts_dir).map_err(|e| e.to_string())?;
    let dest = fonts_dir.join(&filename);
    std::fs::copy(&source_path, &dest).map_err(|e| format!("Copy failed: {}", e))?;
    Ok(filename)
}

#[tauri::command]
pub fn list_imported_fonts() -> Result<Vec<String>, String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    if !fonts_dir.exists() {
        return Ok(vec![]);
    }
    let mut names = vec![];
    for entry in std::fs::read_dir(&fonts_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let ext = std::path::Path::new(&name)
            .extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext == "ttf" || ext == "otf" {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

#[tauri::command]
pub fn delete_imported_font(filename: String) -> Result<(), String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    let path = fonts_dir.join(&filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Phase 3: Drop shadow / glow / mask / clip ────────────────────────────────

fn hex_to_rgba8(hex: &str) -> [u8; 4] {
    let h = hex.trim_start_matches('#');
    if h.len() == 6 {
        let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(0);
        [r, g, b, 255]
    } else {
        [0, 0, 0, 255]
    }
}

#[tauri::command]
pub fn update_layer_shadow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    color_hex: Option<String>,
    blur: Option<u32>,
    distance: Option<u32>,
    angle: Option<f32>,
    opacity: Option<u8>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.shadow.enabled = enabled;
    if let Some(c) = color_hex { layer.shadow.color = hex_to_rgba8(&c); }
    if let Some(v) = blur { layer.shadow.blur = v; }
    if let Some(v) = distance { layer.shadow.distance = v; }
    if let Some(v) = angle { layer.shadow.angle = v; }
    if let Some(v) = opacity { layer.shadow.opacity = v; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_glow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    kind: String, // "outer" | "inner"
    enabled: bool,
    color_hex: Option<String>,
    intensity: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    let glow = if kind == "inner" { &mut layer.inner_glow } else { &mut layer.outer_glow };
    glow.enabled = enabled;
    if let Some(c) = color_hex { glow.color = hex_to_rgba8(&c); }
    if let Some(v) = intensity { glow.intensity = v.clamp(1, 60); }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_mask(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    mask_shape: Option<String>, // None | "circle" | "rounded" | "star"
    mask_radius: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.mask_shape = mask_shape;
    if let Some(r) = mask_radius { layer.mask_radius = r; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn set_clip_to_below(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.clip_to_below = enabled;
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced text ───────────────────────────────────────────────────

#[tauri::command]
pub fn update_text_advanced(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    underline: Option<bool>,
    gradient_from_hex: Option<String>,
    gradient_to_hex: Option<String>,
    gradient_angle: Option<f32>,
    curve_amount: Option<f32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    match &mut layer.kind {
        LayerKind::Text3D {
            underline: ul,
            gradient_colors: gc,
            gradient_angle: ga,
            curve_amount: ca,
            ..
        } => {
            if let Some(v) = underline { *ul = v; }
            if let Some(v) = curve_amount { *ca = v.clamp(-180.0, 180.0); }
            if let Some(v) = gradient_angle { *ga = v; }
            // If either gradient color given, build/update gradient_colors pair
            if gradient_from_hex.is_some() || gradient_to_hex.is_some() {
                let cur = gc.unwrap_or([[255,255,255,255],[255,255,0,255]]);
                let from = gradient_from_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[0]);
                let to   = gradient_to_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[1]);
                *gc = Some([from, to]);
            } else if gradient_from_hex.is_none() && gradient_to_hex.is_none()
                && underline.is_none() && curve_amount.is_none() && gradient_angle.is_none()
            {
                // Clear gradient
                *gc = None;
            }
        }
        _ => return Err("Layer is not a Text layer.".into()),
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced shapes ─────────────────────────────────────────────────

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn add_shape_advanced(
    state: tauri::State<'_, SharedProjectState>,
    shape_type: String,
    width: u32,
    height: u32,
    fill_hex: String,
    border_radius: Option<u32>,
    border_hex: Option<String>,
    border_width: Option<u32>,
    sides: Option<u32>,
    fill_gradient_from: Option<String>,
    fill_gradient_to: Option<String>,
    fill_gradient_angle: Option<f32>,
    x: Option<i64>,
    y: Option<i64>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let cw = s.current.canvas_width as i64;
    let ch = s.current.canvas_height as i64;
    let lx = x.unwrap_or((cw - width as i64) / 2);
    let ly = y.unwrap_or((ch - height as i64) / 2);
    let fill_color = parse_hex_color(&fill_hex);
    let b_color = border_hex.as_deref().map(parse_hex_color).unwrap_or([255,255,255,255]);
    let fill_gradient = match (fill_gradient_from.as_deref(), fill_gradient_to.as_deref()) {
        (Some(a), Some(b)) => Some(BgGradient {
            from: hex_to_rgba8(a),
            to: hex_to_rgba8(b),
            angle: fill_gradient_angle.unwrap_or(0.0),
        }),
        _ => None,
    };
    let id = format!("layer_{}", uuid_v4());
    let name = match shape_type.as_str() {
        "line"     => "Line",
        "arrow"    => "Arrow",
        "triangle" => "Triangle",
        "star"     => "Star",
        "polygon"  => "Polygon",
        "circle"   => "Circle",
        "badge"    => "Badge",
        _          => "Rectangle",
    }.to_string();
    let layer = Layer {
        id: id.clone(),
        name,
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
            shape_type: shape_type.clone(),
            width,
            height,
            fill_color,
            border_radius: border_radius.unwrap_or(0),
            border_color: b_color,
            border_width: border_width.unwrap_or(0),
            sides: sides.unwrap_or(5),
            corner_radii: None,
            fill_gradient,
        },
    };
    s.current.layers.push(layer);
    render_and_respond(&mut s)
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_micros();
    format!("{:x}", ms ^ (ms >> 16))
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

// ── Phase 3: Font management ─────────────────────────────────────────────────

#[tauri::command]
pub fn import_font(source_path: String) -> Result<String, String> {
    let p = std::path::Path::new(&source_path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext != "ttf" && ext != "otf" {
        return Err("Only .ttf and .otf font files are supported.".into());
    }
    let filename = p.file_name().and_then(|n| n.to_str())
        .ok_or("Invalid file name")?
        .to_string();
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    std::fs::create_dir_all(&fonts_dir).map_err(|e| e.to_string())?;
    let dest = fonts_dir.join(&filename);
    std::fs::copy(&source_path, &dest).map_err(|e| format!("Copy failed: {}", e))?;
    Ok(filename)
}

#[tauri::command]
pub fn list_imported_fonts() -> Result<Vec<String>, String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    if !fonts_dir.exists() {
        return Ok(vec![]);
    }
    let mut names = vec![];
    for entry in std::fs::read_dir(&fonts_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let ext = std::path::Path::new(&name)
            .extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext == "ttf" || ext == "otf" {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

#[tauri::command]
pub fn delete_imported_font(filename: String) -> Result<(), String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    let path = fonts_dir.join(&filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Phase 3: Drop shadow / glow / mask / clip ────────────────────────────────

fn hex_to_rgba8(hex: &str) -> [u8; 4] {
    let h = hex.trim_start_matches('#');
    if h.len() == 6 {
        let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(0);
        [r, g, b, 255]
    } else {
        [0, 0, 0, 255]
    }
}

#[tauri::command]
pub fn update_layer_shadow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    color_hex: Option<String>,
    blur: Option<u32>,
    distance: Option<u32>,
    angle: Option<f32>,
    opacity: Option<u8>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.shadow.enabled = enabled;
    if let Some(c) = color_hex { layer.shadow.color = hex_to_rgba8(&c); }
    if let Some(v) = blur { layer.shadow.blur = v; }
    if let Some(v) = distance { layer.shadow.distance = v; }
    if let Some(v) = angle { layer.shadow.angle = v; }
    if let Some(v) = opacity { layer.shadow.opacity = v; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_glow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    kind: String, // "outer" | "inner"
    enabled: bool,
    color_hex: Option<String>,
    intensity: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    let glow = if kind == "inner" { &mut layer.inner_glow } else { &mut layer.outer_glow };
    glow.enabled = enabled;
    if let Some(c) = color_hex { glow.color = hex_to_rgba8(&c); }
    if let Some(v) = intensity { glow.intensity = v.clamp(1, 60); }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_mask(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    mask_shape: Option<String>, // None | "circle" | "rounded" | "star"
    mask_radius: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.mask_shape = mask_shape;
    if let Some(r) = mask_radius { layer.mask_radius = r; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn set_clip_to_below(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.clip_to_below = enabled;
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced text ───────────────────────────────────────────────────

#[tauri::command]
pub fn update_text_advanced(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    underline: Option<bool>,
    gradient_from_hex: Option<String>,
    gradient_to_hex: Option<String>,
    gradient_angle: Option<f32>,
    curve_amount: Option<f32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    match &mut layer.kind {
        LayerKind::Text3D {
            underline: ul,
            gradient_colors: gc,
            gradient_angle: ga,
            curve_amount: ca,
            ..
        } => {
            if let Some(v) = underline { *ul = v; }
            if let Some(v) = curve_amount { *ca = v.clamp(-180.0, 180.0); }
            if let Some(v) = gradient_angle { *ga = v; }
            // If either gradient color given, build/update gradient_colors pair
            if gradient_from_hex.is_some() || gradient_to_hex.is_some() {
                let cur = gc.unwrap_or([[255,255,255,255],[255,255,0,255]]);
                let from = gradient_from_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[0]);
                let to   = gradient_to_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[1]);
                *gc = Some([from, to]);
            } else if gradient_from_hex.is_none() && gradient_to_hex.is_none()
                && underline.is_none() && curve_amount.is_none() && gradient_angle.is_none()
            {
                // Clear gradient
                *gc = None;
            }
        }
        _ => return Err("Layer is not a Text layer.".into()),
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced shapes ─────────────────────────────────────────────────

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn add_shape_advanced(
    state: tauri::State<'_, SharedProjectState>,
    shape_type: String,
    width: u32,
    height: u32,
    fill_hex: String,
    border_radius: Option<u32>,
    border_hex: Option<String>,
    border_width: Option<u32>,
    sides: Option<u32>,
    fill_gradient_from: Option<String>,
    fill_gradient_to: Option<String>,
    fill_gradient_angle: Option<f32>,
    x: Option<i64>,
    y: Option<i64>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let cw = s.current.canvas_width as i64;
    let ch = s.current.canvas_height as i64;
    let lx = x.unwrap_or((cw - width as i64) / 2);
    let ly = y.unwrap_or((ch - height as i64) / 2);
    let fill_color = parse_hex_color(&fill_hex);
    let b_color = border_hex.as_deref().map(parse_hex_color).unwrap_or([255,255,255,255]);
    let fill_gradient = match (fill_gradient_from.as_deref(), fill_gradient_to.as_deref()) {
        (Some(a), Some(b)) => Some(BgGradient {
            from: hex_to_rgba8(a),
            to: hex_to_rgba8(b),
            angle: fill_gradient_angle.unwrap_or(0.0),
        }),
        _ => None,
    };
    let id = format!("layer_{}", uuid_v4());
    let name = match shape_type.as_str() {
        "line"     => "Line",
        "arrow"    => "Arrow",
        "triangle" => "Triangle",
        "star"     => "Star",
        "polygon"  => "Polygon",
        "circle"   => "Circle",
        "badge"    => "Badge",
        _          => "Rectangle",
    }.to_string();
    let layer = Layer {
        id: id.clone(),
        name,
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
            shape_type: shape_type.clone(),
            width,
            height,
            fill_color,
            border_radius: border_radius.unwrap_or(0),
            border_color: b_color,
            border_width: border_width.unwrap_or(0),
            sides: sides.unwrap_or(5),
            corner_radii: None,
            fill_gradient,
        },
    };
    s.current.layers.push(layer);
    render_and_respond(&mut s)
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_micros();
    format!("{:x}", ms ^ (ms >> 16))
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

// ── Phase 3: Font management ─────────────────────────────────────────────────

#[tauri::command]
pub fn import_font(source_path: String) -> Result<String, String> {
    let p = std::path::Path::new(&source_path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext != "ttf" && ext != "otf" {
        return Err("Only .ttf and .otf font files are supported.".into());
    }
    let filename = p.file_name().and_then(|n| n.to_str())
        .ok_or("Invalid file name")?
        .to_string();
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    std::fs::create_dir_all(&fonts_dir).map_err(|e| e.to_string())?;
    let dest = fonts_dir.join(&filename);
    std::fs::copy(&source_path, &dest).map_err(|e| format!("Copy failed: {}", e))?;
    Ok(filename)
}

#[tauri::command]
pub fn list_imported_fonts() -> Result<Vec<String>, String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    if !fonts_dir.exists() {
        return Ok(vec![]);
    }
    let mut names = vec![];
    for entry in std::fs::read_dir(&fonts_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let ext = std::path::Path::new(&name)
            .extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext == "ttf" || ext == "otf" {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

#[tauri::command]
pub fn delete_imported_font(filename: String) -> Result<(), String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    let path = fonts_dir.join(&filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Phase 3: Drop shadow / glow / mask / clip ────────────────────────────────

fn hex_to_rgba8(hex: &str) -> [u8; 4] {
    let h = hex.trim_start_matches('#');
    if h.len() == 6 {
        let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(0);
        [r, g, b, 255]
    } else {
        [0, 0, 0, 255]
    }
}

#[tauri::command]
pub fn update_layer_shadow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    color_hex: Option<String>,
    blur: Option<u32>,
    distance: Option<u32>,
    angle: Option<f32>,
    opacity: Option<u8>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.shadow.enabled = enabled;
    if let Some(c) = color_hex { layer.shadow.color = hex_to_rgba8(&c); }
    if let Some(v) = blur { layer.shadow.blur = v; }
    if let Some(v) = distance { layer.shadow.distance = v; }
    if let Some(v) = angle { layer.shadow.angle = v; }
    if let Some(v) = opacity { layer.shadow.opacity = v; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_glow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    kind: String, // "outer" | "inner"
    enabled: bool,
    color_hex: Option<String>,
    intensity: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    let glow = if kind == "inner" { &mut layer.inner_glow } else { &mut layer.outer_glow };
    glow.enabled = enabled;
    if let Some(c) = color_hex { glow.color = hex_to_rgba8(&c); }
    if let Some(v) = intensity { glow.intensity = v.clamp(1, 60); }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_mask(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    mask_shape: Option<String>, // None | "circle" | "rounded" | "star"
    mask_radius: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.mask_shape = mask_shape;
    if let Some(r) = mask_radius { layer.mask_radius = r; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn set_clip_to_below(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.clip_to_below = enabled;
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced text ───────────────────────────────────────────────────

#[tauri::command]
pub fn update_text_advanced(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    underline: Option<bool>,
    gradient_from_hex: Option<String>,
    gradient_to_hex: Option<String>,
    gradient_angle: Option<f32>,
    curve_amount: Option<f32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    match &mut layer.kind {
        LayerKind::Text3D {
            underline: ul,
            gradient_colors: gc,
            gradient_angle: ga,
            curve_amount: ca,
            ..
        } => {
            if let Some(v) = underline { *ul = v; }
            if let Some(v) = curve_amount { *ca = v.clamp(-180.0, 180.0); }
            if let Some(v) = gradient_angle { *ga = v; }
            // If either gradient color given, build/update gradient_colors pair
            if gradient_from_hex.is_some() || gradient_to_hex.is_some() {
                let cur = gc.unwrap_or([[255,255,255,255],[255,255,0,255]]);
                let from = gradient_from_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[0]);
                let to   = gradient_to_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[1]);
                *gc = Some([from, to]);
            } else if gradient_from_hex.is_none() && gradient_to_hex.is_none()
                && underline.is_none() && curve_amount.is_none() && gradient_angle.is_none()
            {
                // Clear gradient
                *gc = None;
            }
        }
        _ => return Err("Layer is not a Text layer.".into()),
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced shapes ─────────────────────────────────────────────────

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn add_shape_advanced(
    state: tauri::State<'_, SharedProjectState>,
    shape_type: String,
    width: u32,
    height: u32,
    fill_hex: String,
    border_radius: Option<u32>,
    border_hex: Option<String>,
    border_width: Option<u32>,
    sides: Option<u32>,
    fill_gradient_from: Option<String>,
    fill_gradient_to: Option<String>,
    fill_gradient_angle: Option<f32>,
    x: Option<i64>,
    y: Option<i64>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let cw = s.current.canvas_width as i64;
    let ch = s.current.canvas_height as i64;
    let lx = x.unwrap_or((cw - width as i64) / 2);
    let ly = y.unwrap_or((ch - height as i64) / 2);
    let fill_color = parse_hex_color(&fill_hex);
    let b_color = border_hex.as_deref().map(parse_hex_color).unwrap_or([255,255,255,255]);
    let fill_gradient = match (fill_gradient_from.as_deref(), fill_gradient_to.as_deref()) {
        (Some(a), Some(b)) => Some(BgGradient {
            from: hex_to_rgba8(a),
            to: hex_to_rgba8(b),
            angle: fill_gradient_angle.unwrap_or(0.0),
        }),
        _ => None,
    };
    let id = format!("layer_{}", uuid_v4());
    let name = match shape_type.as_str() {
        "line"     => "Line",
        "arrow"    => "Arrow",
        "triangle" => "Triangle",
        "star"     => "Star",
        "polygon"  => "Polygon",
        "circle"   => "Circle",
        "badge"    => "Badge",
        _          => "Rectangle",
    }.to_string();
    let layer = Layer {
        id: id.clone(),
        name,
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
            shape_type: shape_type.clone(),
            width,
            height,
            fill_color,
            border_radius: border_radius.unwrap_or(0),
            border_color: b_color,
            border_width: border_width.unwrap_or(0),
            sides: sides.unwrap_or(5),
            corner_radii: None,
            fill_gradient,
        },
    };
    s.current.layers.push(layer);
    render_and_respond(&mut s)
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_micros();
    format!("{:x}", ms ^ (ms >> 16))
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

// ── Phase 3: Font management ─────────────────────────────────────────────────

#[tauri::command]
pub fn import_font(source_path: String) -> Result<String, String> {
    let p = std::path::Path::new(&source_path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext != "ttf" && ext != "otf" {
        return Err("Only .ttf and .otf font files are supported.".into());
    }
    let filename = p.file_name().and_then(|n| n.to_str())
        .ok_or("Invalid file name")?
        .to_string();
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    std::fs::create_dir_all(&fonts_dir).map_err(|e| e.to_string())?;
    let dest = fonts_dir.join(&filename);
    std::fs::copy(&source_path, &dest).map_err(|e| format!("Copy failed: {}", e))?;
    Ok(filename)
}

#[tauri::command]
pub fn list_imported_fonts() -> Result<Vec<String>, String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    if !fonts_dir.exists() {
        return Ok(vec![]);
    }
    let mut names = vec![];
    for entry in std::fs::read_dir(&fonts_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let ext = std::path::Path::new(&name)
            .extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext == "ttf" || ext == "otf" {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

#[tauri::command]
pub fn delete_imported_font(filename: String) -> Result<(), String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    let path = fonts_dir.join(&filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Phase 3: Drop shadow / glow / mask / clip ────────────────────────────────

fn hex_to_rgba8(hex: &str) -> [u8; 4] {
    let h = hex.trim_start_matches('#');
    if h.len() == 6 {
        let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(0);
        [r, g, b, 255]
    } else {
        [0, 0, 0, 255]
    }
}

#[tauri::command]
pub fn update_layer_shadow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    color_hex: Option<String>,
    blur: Option<u32>,
    distance: Option<u32>,
    angle: Option<f32>,
    opacity: Option<u8>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.shadow.enabled = enabled;
    if let Some(c) = color_hex { layer.shadow.color = hex_to_rgba8(&c); }
    if let Some(v) = blur { layer.shadow.blur = v; }
    if let Some(v) = distance { layer.shadow.distance = v; }
    if let Some(v) = angle { layer.shadow.angle = v; }
    if let Some(v) = opacity { layer.shadow.opacity = v; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_glow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    kind: String, // "outer" | "inner"
    enabled: bool,
    color_hex: Option<String>,
    intensity: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    let glow = if kind == "inner" { &mut layer.inner_glow } else { &mut layer.outer_glow };
    glow.enabled = enabled;
    if let Some(c) = color_hex { glow.color = hex_to_rgba8(&c); }
    if let Some(v) = intensity { glow.intensity = v.clamp(1, 60); }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_mask(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    mask_shape: Option<String>, // None | "circle" | "rounded" | "star"
    mask_radius: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.mask_shape = mask_shape;
    if let Some(r) = mask_radius { layer.mask_radius = r; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn set_clip_to_below(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.clip_to_below = enabled;
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced text ───────────────────────────────────────────────────

#[tauri::command]
pub fn update_text_advanced(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    underline: Option<bool>,
    gradient_from_hex: Option<String>,
    gradient_to_hex: Option<String>,
    gradient_angle: Option<f32>,
    curve_amount: Option<f32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    match &mut layer.kind {
        LayerKind::Text3D {
            underline: ul,
            gradient_colors: gc,
            gradient_angle: ga,
            curve_amount: ca,
            ..
        } => {
            if let Some(v) = underline { *ul = v; }
            if let Some(v) = curve_amount { *ca = v.clamp(-180.0, 180.0); }
            if let Some(v) = gradient_angle { *ga = v; }
            // If either gradient color given, build/update gradient_colors pair
            if gradient_from_hex.is_some() || gradient_to_hex.is_some() {
                let cur = gc.unwrap_or([[255,255,255,255],[255,255,0,255]]);
                let from = gradient_from_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[0]);
                let to   = gradient_to_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[1]);
                *gc = Some([from, to]);
            } else if gradient_from_hex.is_none() && gradient_to_hex.is_none()
                && underline.is_none() && curve_amount.is_none() && gradient_angle.is_none()
            {
                // Clear gradient
                *gc = None;
            }
        }
        _ => return Err("Layer is not a Text layer.".into()),
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced shapes ─────────────────────────────────────────────────

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn add_shape_advanced(
    state: tauri::State<'_, SharedProjectState>,
    shape_type: String,
    width: u32,
    height: u32,
    fill_hex: String,
    border_radius: Option<u32>,
    border_hex: Option<String>,
    border_width: Option<u32>,
    sides: Option<u32>,
    fill_gradient_from: Option<String>,
    fill_gradient_to: Option<String>,
    fill_gradient_angle: Option<f32>,
    x: Option<i64>,
    y: Option<i64>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let cw = s.current.canvas_width as i64;
    let ch = s.current.canvas_height as i64;
    let lx = x.unwrap_or((cw - width as i64) / 2);
    let ly = y.unwrap_or((ch - height as i64) / 2);
    let fill_color = parse_hex_color(&fill_hex);
    let b_color = border_hex.as_deref().map(parse_hex_color).unwrap_or([255,255,255,255]);
    let fill_gradient = match (fill_gradient_from.as_deref(), fill_gradient_to.as_deref()) {
        (Some(a), Some(b)) => Some(BgGradient {
            from: hex_to_rgba8(a),
            to: hex_to_rgba8(b),
            angle: fill_gradient_angle.unwrap_or(0.0),
        }),
        _ => None,
    };
    let id = format!("layer_{}", uuid_v4());
    let name = match shape_type.as_str() {
        "line"     => "Line",
        "arrow"    => "Arrow",
        "triangle" => "Triangle",
        "star"     => "Star",
        "polygon"  => "Polygon",
        "circle"   => "Circle",
        "badge"    => "Badge",
        _          => "Rectangle",
    }.to_string();
    let layer = Layer {
        id: id.clone(),
        name,
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
            shape_type: shape_type.clone(),
            width,
            height,
            fill_color,
            border_radius: border_radius.unwrap_or(0),
            border_color: b_color,
            border_width: border_width.unwrap_or(0),
            sides: sides.unwrap_or(5),
            corner_radii: None,
            fill_gradient,
        },
    };
    s.current.layers.push(layer);
    render_and_respond(&mut s)
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_micros();
    format!("{:x}", ms ^ (ms >> 16))
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

// ── Phase 3: Font management ─────────────────────────────────────────────────

#[tauri::command]
pub fn import_font(source_path: String) -> Result<String, String> {
    let p = std::path::Path::new(&source_path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext != "ttf" && ext != "otf" {
        return Err("Only .ttf and .otf font files are supported.".into());
    }
    let filename = p.file_name().and_then(|n| n.to_str())
        .ok_or("Invalid file name")?
        .to_string();
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    std::fs::create_dir_all(&fonts_dir).map_err(|e| e.to_string())?;
    let dest = fonts_dir.join(&filename);
    std::fs::copy(&source_path, &dest).map_err(|e| format!("Copy failed: {}", e))?;
    Ok(filename)
}

#[tauri::command]
pub fn list_imported_fonts() -> Result<Vec<String>, String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    if !fonts_dir.exists() {
        return Ok(vec![]);
    }
    let mut names = vec![];
    for entry in std::fs::read_dir(&fonts_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let ext = std::path::Path::new(&name)
            .extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext == "ttf" || ext == "otf" {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

#[tauri::command]
pub fn delete_imported_font(filename: String) -> Result<(), String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    let path = fonts_dir.join(&filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Phase 3: Drop shadow / glow / mask / clip ────────────────────────────────

fn hex_to_rgba8(hex: &str) -> [u8; 4] {
    let h = hex.trim_start_matches('#');
    if h.len() == 6 {
        let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(0);
        [r, g, b, 255]
    } else {
        [0, 0, 0, 255]
    }
}

#[tauri::command]
pub fn update_layer_shadow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    color_hex: Option<String>,
    blur: Option<u32>,
    distance: Option<u32>,
    angle: Option<f32>,
    opacity: Option<u8>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.shadow.enabled = enabled;
    if let Some(c) = color_hex { layer.shadow.color = hex_to_rgba8(&c); }
    if let Some(v) = blur { layer.shadow.blur = v; }
    if let Some(v) = distance { layer.shadow.distance = v; }
    if let Some(v) = angle { layer.shadow.angle = v; }
    if let Some(v) = opacity { layer.shadow.opacity = v; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_glow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    kind: String, // "outer" | "inner"
    enabled: bool,
    color_hex: Option<String>,
    intensity: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    let glow = if kind == "inner" { &mut layer.inner_glow } else { &mut layer.outer_glow };
    glow.enabled = enabled;
    if let Some(c) = color_hex { glow.color = hex_to_rgba8(&c); }
    if let Some(v) = intensity { glow.intensity = v.clamp(1, 60); }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_mask(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    mask_shape: Option<String>, // None | "circle" | "rounded" | "star"
    mask_radius: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.mask_shape = mask_shape;
    if let Some(r) = mask_radius { layer.mask_radius = r; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn set_clip_to_below(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.clip_to_below = enabled;
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced text ───────────────────────────────────────────────────

#[tauri::command]
pub fn update_text_advanced(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    underline: Option<bool>,
    gradient_from_hex: Option<String>,
    gradient_to_hex: Option<String>,
    gradient_angle: Option<f32>,
    curve_amount: Option<f32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    match &mut layer.kind {
        LayerKind::Text3D {
            underline: ul,
            gradient_colors: gc,
            gradient_angle: ga,
            curve_amount: ca,
            ..
        } => {
            if let Some(v) = underline { *ul = v; }
            if let Some(v) = curve_amount { *ca = v.clamp(-180.0, 180.0); }
            if let Some(v) = gradient_angle { *ga = v; }
            // If either gradient color given, build/update gradient_colors pair
            if gradient_from_hex.is_some() || gradient_to_hex.is_some() {
                let cur = gc.unwrap_or([[255,255,255,255],[255,255,0,255]]);
                let from = gradient_from_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[0]);
                let to   = gradient_to_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[1]);
                *gc = Some([from, to]);
            } else if gradient_from_hex.is_none() && gradient_to_hex.is_none()
                && underline.is_none() && curve_amount.is_none() && gradient_angle.is_none()
            {
                // Clear gradient
                *gc = None;
            }
        }
        _ => return Err("Layer is not a Text layer.".into()),
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced shapes ─────────────────────────────────────────────────

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn add_shape_advanced(
    state: tauri::State<'_, SharedProjectState>,
    shape_type: String,
    width: u32,
    height: u32,
    fill_hex: String,
    border_radius: Option<u32>,
    border_hex: Option<String>,
    border_width: Option<u32>,
    sides: Option<u32>,
    fill_gradient_from: Option<String>,
    fill_gradient_to: Option<String>,
    fill_gradient_angle: Option<f32>,
    x: Option<i64>,
    y: Option<i64>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let cw = s.current.canvas_width as i64;
    let ch = s.current.canvas_height as i64;
    let lx = x.unwrap_or((cw - width as i64) / 2);
    let ly = y.unwrap_or((ch - height as i64) / 2);
    let fill_color = parse_hex_color(&fill_hex);
    let b_color = border_hex.as_deref().map(parse_hex_color).unwrap_or([255,255,255,255]);
    let fill_gradient = match (fill_gradient_from.as_deref(), fill_gradient_to.as_deref()) {
        (Some(a), Some(b)) => Some(BgGradient {
            from: hex_to_rgba8(a),
            to: hex_to_rgba8(b),
            angle: fill_gradient_angle.unwrap_or(0.0),
        }),
        _ => None,
    };
    let id = format!("layer_{}", uuid_v4());
    let name = match shape_type.as_str() {
        "line"     => "Line",
        "arrow"    => "Arrow",
        "triangle" => "Triangle",
        "star"     => "Star",
        "polygon"  => "Polygon",
        "circle"   => "Circle",
        "badge"    => "Badge",
        _          => "Rectangle",
    }.to_string();
    let layer = Layer {
        id: id.clone(),
        name,
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
            shape_type: shape_type.clone(),
            width,
            height,
            fill_color,
            border_radius: border_radius.unwrap_or(0),
            border_color: b_color,
            border_width: border_width.unwrap_or(0),
            sides: sides.unwrap_or(5),
            corner_radii: None,
            fill_gradient,
        },
    };
    s.current.layers.push(layer);
    render_and_respond(&mut s)
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_micros();
    format!("{:x}", ms ^ (ms >> 16))
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

// ── Phase 3: Font management ─────────────────────────────────────────────────

#[tauri::command]
pub fn import_font(source_path: String) -> Result<String, String> {
    let p = std::path::Path::new(&source_path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext != "ttf" && ext != "otf" {
        return Err("Only .ttf and .otf font files are supported.".into());
    }
    let filename = p.file_name().and_then(|n| n.to_str())
        .ok_or("Invalid file name")?
        .to_string();
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    std::fs::create_dir_all(&fonts_dir).map_err(|e| e.to_string())?;
    let dest = fonts_dir.join(&filename);
    std::fs::copy(&source_path, &dest).map_err(|e| format!("Copy failed: {}", e))?;
    Ok(filename)
}

#[tauri::command]
pub fn list_imported_fonts() -> Result<Vec<String>, String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    if !fonts_dir.exists() {
        return Ok(vec![]);
    }
    let mut names = vec![];
    for entry in std::fs::read_dir(&fonts_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let ext = std::path::Path::new(&name)
            .extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext == "ttf" || ext == "otf" {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

#[tauri::command]
pub fn delete_imported_font(filename: String) -> Result<(), String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    let path = fonts_dir.join(&filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Phase 3: Drop shadow / glow / mask / clip ────────────────────────────────

fn hex_to_rgba8(hex: &str) -> [u8; 4] {
    let h = hex.trim_start_matches('#');
    if h.len() == 6 {
        let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(0);
        [r, g, b, 255]
    } else {
        [0, 0, 0, 255]
    }
}

#[tauri::command]
pub fn update_layer_shadow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    color_hex: Option<String>,
    blur: Option<u32>,
    distance: Option<u32>,
    angle: Option<f32>,
    opacity: Option<u8>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.shadow.enabled = enabled;
    if let Some(c) = color_hex { layer.shadow.color = hex_to_rgba8(&c); }
    if let Some(v) = blur { layer.shadow.blur = v; }
    if let Some(v) = distance { layer.shadow.distance = v; }
    if let Some(v) = angle { layer.shadow.angle = v; }
    if let Some(v) = opacity { layer.shadow.opacity = v; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_glow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    kind: String, // "outer" | "inner"
    enabled: bool,
    color_hex: Option<String>,
    intensity: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    let glow = if kind == "inner" { &mut layer.inner_glow } else { &mut layer.outer_glow };
    glow.enabled = enabled;
    if let Some(c) = color_hex { glow.color = hex_to_rgba8(&c); }
    if let Some(v) = intensity { glow.intensity = v.clamp(1, 60); }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_mask(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    mask_shape: Option<String>, // None | "circle" | "rounded" | "star"
    mask_radius: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.mask_shape = mask_shape;
    if let Some(r) = mask_radius { layer.mask_radius = r; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn set_clip_to_below(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.clip_to_below = enabled;
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced text ───────────────────────────────────────────────────

#[tauri::command]
pub fn update_text_advanced(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    underline: Option<bool>,
    gradient_from_hex: Option<String>,
    gradient_to_hex: Option<String>,
    gradient_angle: Option<f32>,
    curve_amount: Option<f32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    match &mut layer.kind {
        LayerKind::Text3D {
            underline: ul,
            gradient_colors: gc,
            gradient_angle: ga,
            curve_amount: ca,
            ..
        } => {
            if let Some(v) = underline { *ul = v; }
            if let Some(v) = curve_amount { *ca = v.clamp(-180.0, 180.0); }
            if let Some(v) = gradient_angle { *ga = v; }
            // If either gradient color given, build/update gradient_colors pair
            if gradient_from_hex.is_some() || gradient_to_hex.is_some() {
                let cur = gc.unwrap_or([[255,255,255,255],[255,255,0,255]]);
                let from = gradient_from_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[0]);
                let to   = gradient_to_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[1]);
                *gc = Some([from, to]);
            } else if gradient_from_hex.is_none() && gradient_to_hex.is_none()
                && underline.is_none() && curve_amount.is_none() && gradient_angle.is_none()
            {
                // Clear gradient
                *gc = None;
            }
        }
        _ => return Err("Layer is not a Text layer.".into()),
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced shapes ─────────────────────────────────────────────────

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn add_shape_advanced(
    state: tauri::State<'_, SharedProjectState>,
    shape_type: String,
    width: u32,
    height: u32,
    fill_hex: String,
    border_radius: Option<u32>,
    border_hex: Option<String>,
    border_width: Option<u32>,
    sides: Option<u32>,
    fill_gradient_from: Option<String>,
    fill_gradient_to: Option<String>,
    fill_gradient_angle: Option<f32>,
    x: Option<i64>,
    y: Option<i64>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let cw = s.current.canvas_width as i64;
    let ch = s.current.canvas_height as i64;
    let lx = x.unwrap_or((cw - width as i64) / 2);
    let ly = y.unwrap_or((ch - height as i64) / 2);
    let fill_color = parse_hex_color(&fill_hex);
    let b_color = border_hex.as_deref().map(parse_hex_color).unwrap_or([255,255,255,255]);
    let fill_gradient = match (fill_gradient_from.as_deref(), fill_gradient_to.as_deref()) {
        (Some(a), Some(b)) => Some(BgGradient {
            from: hex_to_rgba8(a),
            to: hex_to_rgba8(b),
            angle: fill_gradient_angle.unwrap_or(0.0),
        }),
        _ => None,
    };
    let id = format!("layer_{}", uuid_v4());
    let name = match shape_type.as_str() {
        "line"     => "Line",
        "arrow"    => "Arrow",
        "triangle" => "Triangle",
        "star"     => "Star",
        "polygon"  => "Polygon",
        "circle"   => "Circle",
        "badge"    => "Badge",
        _          => "Rectangle",
    }.to_string();
    let layer = Layer {
        id: id.clone(),
        name,
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
            shape_type: shape_type.clone(),
            width,
            height,
            fill_color,
            border_radius: border_radius.unwrap_or(0),
            border_color: b_color,
            border_width: border_width.unwrap_or(0),
            sides: sides.unwrap_or(5),
            corner_radii: None,
            fill_gradient,
        },
    };
    s.current.layers.push(layer);
    render_and_respond(&mut s)
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_micros();
    format!("{:x}", ms ^ (ms >> 16))
}

// ── Export & Project Save/Load ──────────────────────────────────────────────

#[tauri::command]
pub fn export_configured(
    state: tauri::State<'_, SharedProjectState>,
    dest_path: String,
    format: String,
    quality: u8,
    scale: f32,
    target_width: Option<u32>,
    target_height: Option<u32>,
) -> Result<String, String> {
    let mut snapshot = {
        let s = state.lock().map_err(|e| e.to_string())?;
        if !s.has_project {
            return Err("No active project to export.".to_string());
        }
        s.current.clone()
    };

    let img = render_project_composite(&mut snapshot, "")?;
    let img = if let (Some(tw), Some(th)) = (target_width, target_height) {
        img.resize_exact(tw.max(1), th.max(1), FilterType::Lanczos3)
    } else if (scale - 1.0).abs() > 0.01 {
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

// ── Phase 3: Font management ─────────────────────────────────────────────────

#[tauri::command]
pub fn import_font(source_path: String) -> Result<String, String> {
    let p = std::path::Path::new(&source_path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext != "ttf" && ext != "otf" {
        return Err("Only .ttf and .otf font files are supported.".into());
    }
    let filename = p.file_name().and_then(|n| n.to_str())
        .ok_or("Invalid file name")?
        .to_string();
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    std::fs::create_dir_all(&fonts_dir).map_err(|e| e.to_string())?;
    let dest = fonts_dir.join(&filename);
    std::fs::copy(&source_path, &dest).map_err(|e| format!("Copy failed: {}", e))?;
    Ok(filename)
}

#[tauri::command]
pub fn list_imported_fonts() -> Result<Vec<String>, String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    if !fonts_dir.exists() {
        return Ok(vec![]);
    }
    let mut names = vec![];
    for entry in std::fs::read_dir(&fonts_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let ext = std::path::Path::new(&name)
            .extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext == "ttf" || ext == "otf" {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

#[tauri::command]
pub fn delete_imported_font(filename: String) -> Result<(), String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    let path = fonts_dir.join(&filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Phase 3: Drop shadow / glow / mask / clip ────────────────────────────────

fn hex_to_rgba8(hex: &str) -> [u8; 4] {
    let h = hex.trim_start_matches('#');
    if h.len() == 6 {
        let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(0);
        [r, g, b, 255]
    } else {
        [0, 0, 0, 255]
    }
}

#[tauri::command]
pub fn update_layer_shadow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    color_hex: Option<String>,
    blur: Option<u32>,
    distance: Option<u32>,
    angle: Option<f32>,
    opacity: Option<u8>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.shadow.enabled = enabled;
    if let Some(c) = color_hex { layer.shadow.color = hex_to_rgba8(&c); }
    if let Some(v) = blur { layer.shadow.blur = v; }
    if let Some(v) = distance { layer.shadow.distance = v; }
    if let Some(v) = angle { layer.shadow.angle = v; }
    if let Some(v) = opacity { layer.shadow.opacity = v; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_glow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    kind: String, // "outer" | "inner"
    enabled: bool,
    color_hex: Option<String>,
    intensity: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    let glow = if kind == "inner" { &mut layer.inner_glow } else { &mut layer.outer_glow };
    glow.enabled = enabled;
    if let Some(c) = color_hex { glow.color = hex_to_rgba8(&c); }
    if let Some(v) = intensity { glow.intensity = v.clamp(1, 60); }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_mask(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    mask_shape: Option<String>, // None | "circle" | "rounded" | "star"
    mask_radius: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.mask_shape = mask_shape;
    if let Some(r) = mask_radius { layer.mask_radius = r; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn set_clip_to_below(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.clip_to_below = enabled;
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced text ───────────────────────────────────────────────────

#[tauri::command]
pub fn update_text_advanced(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    underline: Option<bool>,
    gradient_from_hex: Option<String>,
    gradient_to_hex: Option<String>,
    gradient_angle: Option<f32>,
    curve_amount: Option<f32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    match &mut layer.kind {
        LayerKind::Text3D {
            underline: ul,
            gradient_colors: gc,
            gradient_angle: ga,
            curve_amount: ca,
            ..
        } => {
            if let Some(v) = underline { *ul = v; }
            if let Some(v) = curve_amount { *ca = v.clamp(-180.0, 180.0); }
            if let Some(v) = gradient_angle { *ga = v; }
            // If either gradient color given, build/update gradient_colors pair
            if gradient_from_hex.is_some() || gradient_to_hex.is_some() {
                let cur = gc.unwrap_or([[255,255,255,255],[255,255,0,255]]);
                let from = gradient_from_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[0]);
                let to   = gradient_to_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[1]);
                *gc = Some([from, to]);
            } else if gradient_from_hex.is_none() && gradient_to_hex.is_none()
                && underline.is_none() && curve_amount.is_none() && gradient_angle.is_none()
            {
                // Clear gradient
                *gc = None;
            }
        }
        _ => return Err("Layer is not a Text layer.".into()),
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced shapes ─────────────────────────────────────────────────

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn add_shape_advanced(
    state: tauri::State<'_, SharedProjectState>,
    shape_type: String,
    width: u32,
    height: u32,
    fill_hex: String,
    border_radius: Option<u32>,
    border_hex: Option<String>,
    border_width: Option<u32>,
    sides: Option<u32>,
    fill_gradient_from: Option<String>,
    fill_gradient_to: Option<String>,
    fill_gradient_angle: Option<f32>,
    x: Option<i64>,
    y: Option<i64>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let cw = s.current.canvas_width as i64;
    let ch = s.current.canvas_height as i64;
    let lx = x.unwrap_or((cw - width as i64) / 2);
    let ly = y.unwrap_or((ch - height as i64) / 2);
    let fill_color = parse_hex_color(&fill_hex);
    let b_color = border_hex.as_deref().map(parse_hex_color).unwrap_or([255,255,255,255]);
    let fill_gradient = match (fill_gradient_from.as_deref(), fill_gradient_to.as_deref()) {
        (Some(a), Some(b)) => Some(BgGradient {
            from: hex_to_rgba8(a),
            to: hex_to_rgba8(b),
            angle: fill_gradient_angle.unwrap_or(0.0),
        }),
        _ => None,
    };
    let id = format!("layer_{}", uuid_v4());
    let name = match shape_type.as_str() {
        "line"     => "Line",
        "arrow"    => "Arrow",
        "triangle" => "Triangle",
        "star"     => "Star",
        "polygon"  => "Polygon",
        "circle"   => "Circle",
        "badge"    => "Badge",
        _          => "Rectangle",
    }.to_string();
    let layer = Layer {
        id: id.clone(),
        name,
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
            shape_type: shape_type.clone(),
            width,
            height,
            fill_color,
            border_radius: border_radius.unwrap_or(0),
            border_color: b_color,
            border_width: border_width.unwrap_or(0),
            sides: sides.unwrap_or(5),
            corner_radii: None,
            fill_gradient,
        },
    };
    s.current.layers.push(layer);
    render_and_respond(&mut s)
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_micros();
    format!("{:x}", ms ^ (ms >> 16))
}

// ── Flip, Magic Resize & Persistent User Library (%APPDATA%) ────────────────

#[tauri::command]
pub fn flip_layer(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    horizontal: bool,
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
    if horizontal {
        layer.flip_h = !layer.flip_h;
    } else {
        layer.flip_v = !layer.flip_v;
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Font management ─────────────────────────────────────────────────

#[tauri::command]
pub fn import_font(source_path: String) -> Result<String, String> {
    let p = std::path::Path::new(&source_path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext != "ttf" && ext != "otf" {
        return Err("Only .ttf and .otf font files are supported.".into());
    }
    let filename = p.file_name().and_then(|n| n.to_str())
        .ok_or("Invalid file name")?
        .to_string();
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    std::fs::create_dir_all(&fonts_dir).map_err(|e| e.to_string())?;
    let dest = fonts_dir.join(&filename);
    std::fs::copy(&source_path, &dest).map_err(|e| format!("Copy failed: {}", e))?;
    Ok(filename)
}

#[tauri::command]
pub fn list_imported_fonts() -> Result<Vec<String>, String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    if !fonts_dir.exists() {
        return Ok(vec![]);
    }
    let mut names = vec![];
    for entry in std::fs::read_dir(&fonts_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let ext = std::path::Path::new(&name)
            .extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext == "ttf" || ext == "otf" {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

#[tauri::command]
pub fn delete_imported_font(filename: String) -> Result<(), String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    let path = fonts_dir.join(&filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Phase 3: Drop shadow / glow / mask / clip ────────────────────────────────

fn hex_to_rgba8(hex: &str) -> [u8; 4] {
    let h = hex.trim_start_matches('#');
    if h.len() == 6 {
        let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(0);
        [r, g, b, 255]
    } else {
        [0, 0, 0, 255]
    }
}

#[tauri::command]
pub fn update_layer_shadow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    color_hex: Option<String>,
    blur: Option<u32>,
    distance: Option<u32>,
    angle: Option<f32>,
    opacity: Option<u8>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.shadow.enabled = enabled;
    if let Some(c) = color_hex { layer.shadow.color = hex_to_rgba8(&c); }
    if let Some(v) = blur { layer.shadow.blur = v; }
    if let Some(v) = distance { layer.shadow.distance = v; }
    if let Some(v) = angle { layer.shadow.angle = v; }
    if let Some(v) = opacity { layer.shadow.opacity = v; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_glow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    kind: String, // "outer" | "inner"
    enabled: bool,
    color_hex: Option<String>,
    intensity: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    let glow = if kind == "inner" { &mut layer.inner_glow } else { &mut layer.outer_glow };
    glow.enabled = enabled;
    if let Some(c) = color_hex { glow.color = hex_to_rgba8(&c); }
    if let Some(v) = intensity { glow.intensity = v.clamp(1, 60); }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_mask(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    mask_shape: Option<String>, // None | "circle" | "rounded" | "star"
    mask_radius: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.mask_shape = mask_shape;
    if let Some(r) = mask_radius { layer.mask_radius = r; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn set_clip_to_below(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.clip_to_below = enabled;
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced text ───────────────────────────────────────────────────

#[tauri::command]
pub fn update_text_advanced(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    underline: Option<bool>,
    gradient_from_hex: Option<String>,
    gradient_to_hex: Option<String>,
    gradient_angle: Option<f32>,
    curve_amount: Option<f32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    match &mut layer.kind {
        LayerKind::Text3D {
            underline: ul,
            gradient_colors: gc,
            gradient_angle: ga,
            curve_amount: ca,
            ..
        } => {
            if let Some(v) = underline { *ul = v; }
            if let Some(v) = curve_amount { *ca = v.clamp(-180.0, 180.0); }
            if let Some(v) = gradient_angle { *ga = v; }
            // If either gradient color given, build/update gradient_colors pair
            if gradient_from_hex.is_some() || gradient_to_hex.is_some() {
                let cur = gc.unwrap_or([[255,255,255,255],[255,255,0,255]]);
                let from = gradient_from_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[0]);
                let to   = gradient_to_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[1]);
                *gc = Some([from, to]);
            } else if gradient_from_hex.is_none() && gradient_to_hex.is_none()
                && underline.is_none() && curve_amount.is_none() && gradient_angle.is_none()
            {
                // Clear gradient
                *gc = None;
            }
        }
        _ => return Err("Layer is not a Text layer.".into()),
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced shapes ─────────────────────────────────────────────────

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn add_shape_advanced(
    state: tauri::State<'_, SharedProjectState>,
    shape_type: String,
    width: u32,
    height: u32,
    fill_hex: String,
    border_radius: Option<u32>,
    border_hex: Option<String>,
    border_width: Option<u32>,
    sides: Option<u32>,
    fill_gradient_from: Option<String>,
    fill_gradient_to: Option<String>,
    fill_gradient_angle: Option<f32>,
    x: Option<i64>,
    y: Option<i64>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let cw = s.current.canvas_width as i64;
    let ch = s.current.canvas_height as i64;
    let lx = x.unwrap_or((cw - width as i64) / 2);
    let ly = y.unwrap_or((ch - height as i64) / 2);
    let fill_color = parse_hex_color(&fill_hex);
    let b_color = border_hex.as_deref().map(parse_hex_color).unwrap_or([255,255,255,255]);
    let fill_gradient = match (fill_gradient_from.as_deref(), fill_gradient_to.as_deref()) {
        (Some(a), Some(b)) => Some(BgGradient {
            from: hex_to_rgba8(a),
            to: hex_to_rgba8(b),
            angle: fill_gradient_angle.unwrap_or(0.0),
        }),
        _ => None,
    };
    let id = format!("layer_{}", uuid_v4());
    let name = match shape_type.as_str() {
        "line"     => "Line",
        "arrow"    => "Arrow",
        "triangle" => "Triangle",
        "star"     => "Star",
        "polygon"  => "Polygon",
        "circle"   => "Circle",
        "badge"    => "Badge",
        _          => "Rectangle",
    }.to_string();
    let layer = Layer {
        id: id.clone(),
        name,
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
            shape_type: shape_type.clone(),
            width,
            height,
            fill_color,
            border_radius: border_radius.unwrap_or(0),
            border_color: b_color,
            border_width: border_width.unwrap_or(0),
            sides: sides.unwrap_or(5),
            corner_radii: None,
            fill_gradient,
        },
    };
    s.current.layers.push(layer);
    render_and_respond(&mut s)
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_micros();
    format!("{:x}", ms ^ (ms >> 16))
}

#[tauri::command]
pub fn resize_canvas(
    state: tauri::State<'_, SharedProjectState>,
    width: u32,
    height: u32,
    scale_layers: bool,
) -> Result<ProjectResponse, String> {
    if width == 0 || height == 0 {
        return Err("Canvas width and height must be greater than zero.".to_string());
    }
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if !s.has_project {
        return Err("No active project to resize.".to_string());
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
        for layer in &mut s.current.layers {
            layer.x = ((layer.x as f32) * sx).round() as i64;
            layer.y = ((layer.y as f32) * sy).round() as i64;
            layer.scale = (layer.scale * uniform).clamp(0.05, 8.0);
        }
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Font management ─────────────────────────────────────────────────

#[tauri::command]
pub fn import_font(source_path: String) -> Result<String, String> {
    let p = std::path::Path::new(&source_path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext != "ttf" && ext != "otf" {
        return Err("Only .ttf and .otf font files are supported.".into());
    }
    let filename = p.file_name().and_then(|n| n.to_str())
        .ok_or("Invalid file name")?
        .to_string();
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    std::fs::create_dir_all(&fonts_dir).map_err(|e| e.to_string())?;
    let dest = fonts_dir.join(&filename);
    std::fs::copy(&source_path, &dest).map_err(|e| format!("Copy failed: {}", e))?;
    Ok(filename)
}

#[tauri::command]
pub fn list_imported_fonts() -> Result<Vec<String>, String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    if !fonts_dir.exists() {
        return Ok(vec![]);
    }
    let mut names = vec![];
    for entry in std::fs::read_dir(&fonts_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let ext = std::path::Path::new(&name)
            .extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext == "ttf" || ext == "otf" {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

#[tauri::command]
pub fn delete_imported_font(filename: String) -> Result<(), String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    let path = fonts_dir.join(&filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Phase 3: Drop shadow / glow / mask / clip ────────────────────────────────

fn hex_to_rgba8(hex: &str) -> [u8; 4] {
    let h = hex.trim_start_matches('#');
    if h.len() == 6 {
        let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(0);
        [r, g, b, 255]
    } else {
        [0, 0, 0, 255]
    }
}

#[tauri::command]
pub fn update_layer_shadow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    color_hex: Option<String>,
    blur: Option<u32>,
    distance: Option<u32>,
    angle: Option<f32>,
    opacity: Option<u8>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.shadow.enabled = enabled;
    if let Some(c) = color_hex { layer.shadow.color = hex_to_rgba8(&c); }
    if let Some(v) = blur { layer.shadow.blur = v; }
    if let Some(v) = distance { layer.shadow.distance = v; }
    if let Some(v) = angle { layer.shadow.angle = v; }
    if let Some(v) = opacity { layer.shadow.opacity = v; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_glow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    kind: String, // "outer" | "inner"
    enabled: bool,
    color_hex: Option<String>,
    intensity: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    let glow = if kind == "inner" { &mut layer.inner_glow } else { &mut layer.outer_glow };
    glow.enabled = enabled;
    if let Some(c) = color_hex { glow.color = hex_to_rgba8(&c); }
    if let Some(v) = intensity { glow.intensity = v.clamp(1, 60); }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_mask(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    mask_shape: Option<String>, // None | "circle" | "rounded" | "star"
    mask_radius: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.mask_shape = mask_shape;
    if let Some(r) = mask_radius { layer.mask_radius = r; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn set_clip_to_below(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.clip_to_below = enabled;
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced text ───────────────────────────────────────────────────

#[tauri::command]
pub fn update_text_advanced(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    underline: Option<bool>,
    gradient_from_hex: Option<String>,
    gradient_to_hex: Option<String>,
    gradient_angle: Option<f32>,
    curve_amount: Option<f32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    match &mut layer.kind {
        LayerKind::Text3D {
            underline: ul,
            gradient_colors: gc,
            gradient_angle: ga,
            curve_amount: ca,
            ..
        } => {
            if let Some(v) = underline { *ul = v; }
            if let Some(v) = curve_amount { *ca = v.clamp(-180.0, 180.0); }
            if let Some(v) = gradient_angle { *ga = v; }
            // If either gradient color given, build/update gradient_colors pair
            if gradient_from_hex.is_some() || gradient_to_hex.is_some() {
                let cur = gc.unwrap_or([[255,255,255,255],[255,255,0,255]]);
                let from = gradient_from_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[0]);
                let to   = gradient_to_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[1]);
                *gc = Some([from, to]);
            } else if gradient_from_hex.is_none() && gradient_to_hex.is_none()
                && underline.is_none() && curve_amount.is_none() && gradient_angle.is_none()
            {
                // Clear gradient
                *gc = None;
            }
        }
        _ => return Err("Layer is not a Text layer.".into()),
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced shapes ─────────────────────────────────────────────────

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn add_shape_advanced(
    state: tauri::State<'_, SharedProjectState>,
    shape_type: String,
    width: u32,
    height: u32,
    fill_hex: String,
    border_radius: Option<u32>,
    border_hex: Option<String>,
    border_width: Option<u32>,
    sides: Option<u32>,
    fill_gradient_from: Option<String>,
    fill_gradient_to: Option<String>,
    fill_gradient_angle: Option<f32>,
    x: Option<i64>,
    y: Option<i64>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let cw = s.current.canvas_width as i64;
    let ch = s.current.canvas_height as i64;
    let lx = x.unwrap_or((cw - width as i64) / 2);
    let ly = y.unwrap_or((ch - height as i64) / 2);
    let fill_color = parse_hex_color(&fill_hex);
    let b_color = border_hex.as_deref().map(parse_hex_color).unwrap_or([255,255,255,255]);
    let fill_gradient = match (fill_gradient_from.as_deref(), fill_gradient_to.as_deref()) {
        (Some(a), Some(b)) => Some(BgGradient {
            from: hex_to_rgba8(a),
            to: hex_to_rgba8(b),
            angle: fill_gradient_angle.unwrap_or(0.0),
        }),
        _ => None,
    };
    let id = format!("layer_{}", uuid_v4());
    let name = match shape_type.as_str() {
        "line"     => "Line",
        "arrow"    => "Arrow",
        "triangle" => "Triangle",
        "star"     => "Star",
        "polygon"  => "Polygon",
        "circle"   => "Circle",
        "badge"    => "Badge",
        _          => "Rectangle",
    }.to_string();
    let layer = Layer {
        id: id.clone(),
        name,
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
            shape_type: shape_type.clone(),
            width,
            height,
            fill_color,
            border_radius: border_radius.unwrap_or(0),
            border_color: b_color,
            border_width: border_width.unwrap_or(0),
            sides: sides.unwrap_or(5),
            corner_radii: None,
            fill_gradient,
        },
    };
    s.current.layers.push(layer);
    render_and_respond(&mut s)
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_micros();
    format!("{:x}", ms ^ (ms >> 16))
}

fn user_data_root() -> std::path::PathBuf {
    let appdata = std::env::var("APPDATA")
        .unwrap_or_else(|_| r"C:\Users\Public\AppData\Roaming".to_string());
    std::path::Path::new(&appdata).join("Veltrax Studios")
}

#[tauri::command]
pub fn get_default_projects_dir() -> Result<String, String> {
    let dir = user_data_root().join("projects");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.to_string_lossy().to_string())
}

#[derive(Clone, serde::Serialize)]
pub struct LibraryItem {
    pub id: String,
    pub name: String,
    pub path: String,
    pub width: u32,
    pub height: u32,
}

#[tauri::command]
pub fn get_user_library() -> Result<Vec<LibraryItem>, String> {
    let lib_dir = user_data_root().join("library");
    std::fs::create_dir_all(&lib_dir).map_err(|e| e.to_string())?;
    let mut items = Vec::new();
    let entries = std::fs::read_dir(&lib_dir).map_err(|e| e.to_string())?;
    for entry in entries.flatten() {
        let p = entry.path();
        if !p.is_file() {
            continue;
        }
        let ext = p
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if !matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "bmp") {
            continue;
        }
        let id = p
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        let display_name = id
            .split_once('_')
            .map(|(_, rest)| rest.to_string())
            .unwrap_or_else(|| id.clone());
        let (w, h) = image::image_dimensions(&p).unwrap_or((0, 0));
        items.push(LibraryItem {
            id,
            name: display_name,
            path: p.to_string_lossy().to_string(),
            width: w,
            height: h,
        });
    }
    items.sort_by(|a, b| b.id.cmp(&a.id));
    Ok(items)
}

#[tauri::command]
pub fn save_to_user_library(source_path: String) -> Result<LibraryItem, String> {
    let src = Path::new(&source_path);
    if !src.is_file() {
        return Err(format!("Source file '{}' not found.", source_path));
    }
    let lib_dir = user_data_root().join("library");
    std::fs::create_dir_all(&lib_dir).map_err(|e| e.to_string())?;

    let orig_name = src
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("element.png");
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let id = format!("{}_{}", ms, orig_name);
    let dest = lib_dir.join(&id);
    std::fs::copy(src, &dest).map_err(|e| format!("Failed to save element to library: {}", e))?;

    let (w, h) = image::image_dimensions(&dest).unwrap_or((0, 0));
    Ok(LibraryItem {
        id,
        name: orig_name.to_string(),
        path: dest.to_string_lossy().to_string(),
        width: w,
        height: h,
    })
}

#[tauri::command]
pub fn delete_from_user_library(id: String) -> Result<(), String> {
    if id.contains("..") || id.contains('/') || id.contains('\\') {
        return Err("Invalid library item ID.".to_string());
    }
    let target = user_data_root().join("library").join(&id);
    if target.is_file() {
        std::fs::remove_file(&target).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct BrandKit {
    pub colors: Vec<String>,
    pub recent_colors: Vec<String>,
}

impl Default for BrandKit {
    fn default() -> Self {
        Self {
            colors: vec![
                "#38BDF8".to_string(),
                "#6366F1".to_string(),
                "#EC4899".to_string(),
                "#10B981".to_string(),
                "#F59E0B".to_string(),
            ],
            recent_colors: vec!["#FFFFFF".to_string(), "#000000".to_string()],
        }
    }
}

#[tauri::command]
pub fn get_brand_kit() -> Result<BrandKit, String> {
    let root = user_data_root();
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let path = root.join("brand_kit.json");
    if path.is_file() {
        if let Ok(raw) = std::fs::read_to_string(&path) {
            if let Ok(kit) = serde_json::from_str::<BrandKit>(&raw) {
                return Ok(kit);
            }
        }
    }
    Ok(BrandKit::default())
}

#[tauri::command]
pub fn save_brand_kit(colors: Vec<String>, recent_colors: Vec<String>) -> Result<BrandKit, String> {
    let root = user_data_root();
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let kit = BrandKit {
        colors,
        recent_colors,
    };
    let json = serde_json::to_string_pretty(&kit).map_err(|e| e.to_string())?;
    std::fs::write(root.join("brand_kit.json"), json).map_err(|e| e.to_string())?;
    Ok(kit)
}

// ── Phase 2: Multi-select, Groups, Align, Distribute, Clipboard & Drop ─────

#[derive(Clone, Debug, serde::Deserialize)]
pub struct LayerTransformPatch {
    pub layer_id: String,
    pub x: i64,
    pub y: i64,
    pub scale: f32,
    pub rotation: f32,
}

#[tauri::command]
pub fn transform_layers(
    state: tauri::State<'_, SharedProjectState>,
    updates: Vec<LayerTransformPatch>,
) -> Result<ProjectResponse, String> {
    if updates.is_empty() {
        let s = state.lock().map_err(|e| e.to_string())?;
        return Ok(s.to_response());
    }
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    for u in updates {
        if let Some(l) = s.current.layers.iter_mut().find(|l| l.id == u.layer_id) {
            if !l.locked {
                l.x = u.x;
                l.y = u.y;
                l.scale = u.scale.clamp(0.05, 8.0);
                l.rotation = u.rotation;
            }
        }
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Font management ─────────────────────────────────────────────────

#[tauri::command]
pub fn import_font(source_path: String) -> Result<String, String> {
    let p = std::path::Path::new(&source_path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext != "ttf" && ext != "otf" {
        return Err("Only .ttf and .otf font files are supported.".into());
    }
    let filename = p.file_name().and_then(|n| n.to_str())
        .ok_or("Invalid file name")?
        .to_string();
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    std::fs::create_dir_all(&fonts_dir).map_err(|e| e.to_string())?;
    let dest = fonts_dir.join(&filename);
    std::fs::copy(&source_path, &dest).map_err(|e| format!("Copy failed: {}", e))?;
    Ok(filename)
}

#[tauri::command]
pub fn list_imported_fonts() -> Result<Vec<String>, String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    if !fonts_dir.exists() {
        return Ok(vec![]);
    }
    let mut names = vec![];
    for entry in std::fs::read_dir(&fonts_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let ext = std::path::Path::new(&name)
            .extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext == "ttf" || ext == "otf" {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

#[tauri::command]
pub fn delete_imported_font(filename: String) -> Result<(), String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    let path = fonts_dir.join(&filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Phase 3: Drop shadow / glow / mask / clip ────────────────────────────────

fn hex_to_rgba8(hex: &str) -> [u8; 4] {
    let h = hex.trim_start_matches('#');
    if h.len() == 6 {
        let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(0);
        [r, g, b, 255]
    } else {
        [0, 0, 0, 255]
    }
}

#[tauri::command]
pub fn update_layer_shadow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    color_hex: Option<String>,
    blur: Option<u32>,
    distance: Option<u32>,
    angle: Option<f32>,
    opacity: Option<u8>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.shadow.enabled = enabled;
    if let Some(c) = color_hex { layer.shadow.color = hex_to_rgba8(&c); }
    if let Some(v) = blur { layer.shadow.blur = v; }
    if let Some(v) = distance { layer.shadow.distance = v; }
    if let Some(v) = angle { layer.shadow.angle = v; }
    if let Some(v) = opacity { layer.shadow.opacity = v; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_glow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    kind: String, // "outer" | "inner"
    enabled: bool,
    color_hex: Option<String>,
    intensity: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    let glow = if kind == "inner" { &mut layer.inner_glow } else { &mut layer.outer_glow };
    glow.enabled = enabled;
    if let Some(c) = color_hex { glow.color = hex_to_rgba8(&c); }
    if let Some(v) = intensity { glow.intensity = v.clamp(1, 60); }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_mask(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    mask_shape: Option<String>, // None | "circle" | "rounded" | "star"
    mask_radius: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.mask_shape = mask_shape;
    if let Some(r) = mask_radius { layer.mask_radius = r; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn set_clip_to_below(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.clip_to_below = enabled;
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced text ───────────────────────────────────────────────────

#[tauri::command]
pub fn update_text_advanced(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    underline: Option<bool>,
    gradient_from_hex: Option<String>,
    gradient_to_hex: Option<String>,
    gradient_angle: Option<f32>,
    curve_amount: Option<f32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    match &mut layer.kind {
        LayerKind::Text3D {
            underline: ul,
            gradient_colors: gc,
            gradient_angle: ga,
            curve_amount: ca,
            ..
        } => {
            if let Some(v) = underline { *ul = v; }
            if let Some(v) = curve_amount { *ca = v.clamp(-180.0, 180.0); }
            if let Some(v) = gradient_angle { *ga = v; }
            // If either gradient color given, build/update gradient_colors pair
            if gradient_from_hex.is_some() || gradient_to_hex.is_some() {
                let cur = gc.unwrap_or([[255,255,255,255],[255,255,0,255]]);
                let from = gradient_from_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[0]);
                let to   = gradient_to_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[1]);
                *gc = Some([from, to]);
            } else if gradient_from_hex.is_none() && gradient_to_hex.is_none()
                && underline.is_none() && curve_amount.is_none() && gradient_angle.is_none()
            {
                // Clear gradient
                *gc = None;
            }
        }
        _ => return Err("Layer is not a Text layer.".into()),
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced shapes ─────────────────────────────────────────────────

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn add_shape_advanced(
    state: tauri::State<'_, SharedProjectState>,
    shape_type: String,
    width: u32,
    height: u32,
    fill_hex: String,
    border_radius: Option<u32>,
    border_hex: Option<String>,
    border_width: Option<u32>,
    sides: Option<u32>,
    fill_gradient_from: Option<String>,
    fill_gradient_to: Option<String>,
    fill_gradient_angle: Option<f32>,
    x: Option<i64>,
    y: Option<i64>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let cw = s.current.canvas_width as i64;
    let ch = s.current.canvas_height as i64;
    let lx = x.unwrap_or((cw - width as i64) / 2);
    let ly = y.unwrap_or((ch - height as i64) / 2);
    let fill_color = parse_hex_color(&fill_hex);
    let b_color = border_hex.as_deref().map(parse_hex_color).unwrap_or([255,255,255,255]);
    let fill_gradient = match (fill_gradient_from.as_deref(), fill_gradient_to.as_deref()) {
        (Some(a), Some(b)) => Some(BgGradient {
            from: hex_to_rgba8(a),
            to: hex_to_rgba8(b),
            angle: fill_gradient_angle.unwrap_or(0.0),
        }),
        _ => None,
    };
    let id = format!("layer_{}", uuid_v4());
    let name = match shape_type.as_str() {
        "line"     => "Line",
        "arrow"    => "Arrow",
        "triangle" => "Triangle",
        "star"     => "Star",
        "polygon"  => "Polygon",
        "circle"   => "Circle",
        "badge"    => "Badge",
        _          => "Rectangle",
    }.to_string();
    let layer = Layer {
        id: id.clone(),
        name,
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
            shape_type: shape_type.clone(),
            width,
            height,
            fill_color,
            border_radius: border_radius.unwrap_or(0),
            border_color: b_color,
            border_width: border_width.unwrap_or(0),
            sides: sides.unwrap_or(5),
            corner_radii: None,
            fill_gradient,
        },
    };
    s.current.layers.push(layer);
    render_and_respond(&mut s)
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_micros();
    format!("{:x}", ms ^ (ms >> 16))
}

#[tauri::command]
pub fn duplicate_layers(
    state: tauri::State<'_, SharedProjectState>,
    layer_ids: Vec<String>,
) -> Result<ProjectResponse, String> {
    if layer_ids.is_empty() {
        return Err("No layers selected.".to_string());
    }
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let mut copies = Vec::new();
    for l in &s.current.layers {
        if layer_ids.contains(&l.id) {
            let mut copy = l.clone();
            copy.id = s.next_id();
            copy.name = format!("{} Copy", copy.name);
            copy.x += 20;
            copy.y += 20;
            copies.push(copy);
        }
    }
    s.current.layers.extend(copies);
    render_and_respond(&mut s)
}

// ── Phase 3: Font management ─────────────────────────────────────────────────

#[tauri::command]
pub fn import_font(source_path: String) -> Result<String, String> {
    let p = std::path::Path::new(&source_path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext != "ttf" && ext != "otf" {
        return Err("Only .ttf and .otf font files are supported.".into());
    }
    let filename = p.file_name().and_then(|n| n.to_str())
        .ok_or("Invalid file name")?
        .to_string();
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    std::fs::create_dir_all(&fonts_dir).map_err(|e| e.to_string())?;
    let dest = fonts_dir.join(&filename);
    std::fs::copy(&source_path, &dest).map_err(|e| format!("Copy failed: {}", e))?;
    Ok(filename)
}

#[tauri::command]
pub fn list_imported_fonts() -> Result<Vec<String>, String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    if !fonts_dir.exists() {
        return Ok(vec![]);
    }
    let mut names = vec![];
    for entry in std::fs::read_dir(&fonts_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let ext = std::path::Path::new(&name)
            .extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext == "ttf" || ext == "otf" {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

#[tauri::command]
pub fn delete_imported_font(filename: String) -> Result<(), String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    let path = fonts_dir.join(&filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Phase 3: Drop shadow / glow / mask / clip ────────────────────────────────

fn hex_to_rgba8(hex: &str) -> [u8; 4] {
    let h = hex.trim_start_matches('#');
    if h.len() == 6 {
        let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(0);
        [r, g, b, 255]
    } else {
        [0, 0, 0, 255]
    }
}

#[tauri::command]
pub fn update_layer_shadow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    color_hex: Option<String>,
    blur: Option<u32>,
    distance: Option<u32>,
    angle: Option<f32>,
    opacity: Option<u8>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.shadow.enabled = enabled;
    if let Some(c) = color_hex { layer.shadow.color = hex_to_rgba8(&c); }
    if let Some(v) = blur { layer.shadow.blur = v; }
    if let Some(v) = distance { layer.shadow.distance = v; }
    if let Some(v) = angle { layer.shadow.angle = v; }
    if let Some(v) = opacity { layer.shadow.opacity = v; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_glow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    kind: String, // "outer" | "inner"
    enabled: bool,
    color_hex: Option<String>,
    intensity: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    let glow = if kind == "inner" { &mut layer.inner_glow } else { &mut layer.outer_glow };
    glow.enabled = enabled;
    if let Some(c) = color_hex { glow.color = hex_to_rgba8(&c); }
    if let Some(v) = intensity { glow.intensity = v.clamp(1, 60); }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_mask(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    mask_shape: Option<String>, // None | "circle" | "rounded" | "star"
    mask_radius: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.mask_shape = mask_shape;
    if let Some(r) = mask_radius { layer.mask_radius = r; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn set_clip_to_below(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.clip_to_below = enabled;
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced text ───────────────────────────────────────────────────

#[tauri::command]
pub fn update_text_advanced(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    underline: Option<bool>,
    gradient_from_hex: Option<String>,
    gradient_to_hex: Option<String>,
    gradient_angle: Option<f32>,
    curve_amount: Option<f32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    match &mut layer.kind {
        LayerKind::Text3D {
            underline: ul,
            gradient_colors: gc,
            gradient_angle: ga,
            curve_amount: ca,
            ..
        } => {
            if let Some(v) = underline { *ul = v; }
            if let Some(v) = curve_amount { *ca = v.clamp(-180.0, 180.0); }
            if let Some(v) = gradient_angle { *ga = v; }
            // If either gradient color given, build/update gradient_colors pair
            if gradient_from_hex.is_some() || gradient_to_hex.is_some() {
                let cur = gc.unwrap_or([[255,255,255,255],[255,255,0,255]]);
                let from = gradient_from_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[0]);
                let to   = gradient_to_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[1]);
                *gc = Some([from, to]);
            } else if gradient_from_hex.is_none() && gradient_to_hex.is_none()
                && underline.is_none() && curve_amount.is_none() && gradient_angle.is_none()
            {
                // Clear gradient
                *gc = None;
            }
        }
        _ => return Err("Layer is not a Text layer.".into()),
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced shapes ─────────────────────────────────────────────────

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn add_shape_advanced(
    state: tauri::State<'_, SharedProjectState>,
    shape_type: String,
    width: u32,
    height: u32,
    fill_hex: String,
    border_radius: Option<u32>,
    border_hex: Option<String>,
    border_width: Option<u32>,
    sides: Option<u32>,
    fill_gradient_from: Option<String>,
    fill_gradient_to: Option<String>,
    fill_gradient_angle: Option<f32>,
    x: Option<i64>,
    y: Option<i64>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let cw = s.current.canvas_width as i64;
    let ch = s.current.canvas_height as i64;
    let lx = x.unwrap_or((cw - width as i64) / 2);
    let ly = y.unwrap_or((ch - height as i64) / 2);
    let fill_color = parse_hex_color(&fill_hex);
    let b_color = border_hex.as_deref().map(parse_hex_color).unwrap_or([255,255,255,255]);
    let fill_gradient = match (fill_gradient_from.as_deref(), fill_gradient_to.as_deref()) {
        (Some(a), Some(b)) => Some(BgGradient {
            from: hex_to_rgba8(a),
            to: hex_to_rgba8(b),
            angle: fill_gradient_angle.unwrap_or(0.0),
        }),
        _ => None,
    };
    let id = format!("layer_{}", uuid_v4());
    let name = match shape_type.as_str() {
        "line"     => "Line",
        "arrow"    => "Arrow",
        "triangle" => "Triangle",
        "star"     => "Star",
        "polygon"  => "Polygon",
        "circle"   => "Circle",
        "badge"    => "Badge",
        _          => "Rectangle",
    }.to_string();
    let layer = Layer {
        id: id.clone(),
        name,
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
            shape_type: shape_type.clone(),
            width,
            height,
            fill_color,
            border_radius: border_radius.unwrap_or(0),
            border_color: b_color,
            border_width: border_width.unwrap_or(0),
            sides: sides.unwrap_or(5),
            corner_radii: None,
            fill_gradient,
        },
    };
    s.current.layers.push(layer);
    render_and_respond(&mut s)
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_micros();
    format!("{:x}", ms ^ (ms >> 16))
}

#[tauri::command]
pub fn delete_layers(
    state: tauri::State<'_, SharedProjectState>,
    layer_ids: Vec<String>,
) -> Result<ProjectResponse, String> {
    if layer_ids.is_empty() {
        return Err("No layers selected.".to_string());
    }
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    s.current.layers.retain(|l| !layer_ids.contains(&l.id));
    render_and_respond(&mut s)
}

// ── Phase 3: Font management ─────────────────────────────────────────────────

#[tauri::command]
pub fn import_font(source_path: String) -> Result<String, String> {
    let p = std::path::Path::new(&source_path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext != "ttf" && ext != "otf" {
        return Err("Only .ttf and .otf font files are supported.".into());
    }
    let filename = p.file_name().and_then(|n| n.to_str())
        .ok_or("Invalid file name")?
        .to_string();
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    std::fs::create_dir_all(&fonts_dir).map_err(|e| e.to_string())?;
    let dest = fonts_dir.join(&filename);
    std::fs::copy(&source_path, &dest).map_err(|e| format!("Copy failed: {}", e))?;
    Ok(filename)
}

#[tauri::command]
pub fn list_imported_fonts() -> Result<Vec<String>, String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    if !fonts_dir.exists() {
        return Ok(vec![]);
    }
    let mut names = vec![];
    for entry in std::fs::read_dir(&fonts_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let ext = std::path::Path::new(&name)
            .extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext == "ttf" || ext == "otf" {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

#[tauri::command]
pub fn delete_imported_font(filename: String) -> Result<(), String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    let path = fonts_dir.join(&filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Phase 3: Drop shadow / glow / mask / clip ────────────────────────────────

fn hex_to_rgba8(hex: &str) -> [u8; 4] {
    let h = hex.trim_start_matches('#');
    if h.len() == 6 {
        let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(0);
        [r, g, b, 255]
    } else {
        [0, 0, 0, 255]
    }
}

#[tauri::command]
pub fn update_layer_shadow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    color_hex: Option<String>,
    blur: Option<u32>,
    distance: Option<u32>,
    angle: Option<f32>,
    opacity: Option<u8>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.shadow.enabled = enabled;
    if let Some(c) = color_hex { layer.shadow.color = hex_to_rgba8(&c); }
    if let Some(v) = blur { layer.shadow.blur = v; }
    if let Some(v) = distance { layer.shadow.distance = v; }
    if let Some(v) = angle { layer.shadow.angle = v; }
    if let Some(v) = opacity { layer.shadow.opacity = v; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_glow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    kind: String, // "outer" | "inner"
    enabled: bool,
    color_hex: Option<String>,
    intensity: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    let glow = if kind == "inner" { &mut layer.inner_glow } else { &mut layer.outer_glow };
    glow.enabled = enabled;
    if let Some(c) = color_hex { glow.color = hex_to_rgba8(&c); }
    if let Some(v) = intensity { glow.intensity = v.clamp(1, 60); }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_mask(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    mask_shape: Option<String>, // None | "circle" | "rounded" | "star"
    mask_radius: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.mask_shape = mask_shape;
    if let Some(r) = mask_radius { layer.mask_radius = r; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn set_clip_to_below(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.clip_to_below = enabled;
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced text ───────────────────────────────────────────────────

#[tauri::command]
pub fn update_text_advanced(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    underline: Option<bool>,
    gradient_from_hex: Option<String>,
    gradient_to_hex: Option<String>,
    gradient_angle: Option<f32>,
    curve_amount: Option<f32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    match &mut layer.kind {
        LayerKind::Text3D {
            underline: ul,
            gradient_colors: gc,
            gradient_angle: ga,
            curve_amount: ca,
            ..
        } => {
            if let Some(v) = underline { *ul = v; }
            if let Some(v) = curve_amount { *ca = v.clamp(-180.0, 180.0); }
            if let Some(v) = gradient_angle { *ga = v; }
            // If either gradient color given, build/update gradient_colors pair
            if gradient_from_hex.is_some() || gradient_to_hex.is_some() {
                let cur = gc.unwrap_or([[255,255,255,255],[255,255,0,255]]);
                let from = gradient_from_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[0]);
                let to   = gradient_to_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[1]);
                *gc = Some([from, to]);
            } else if gradient_from_hex.is_none() && gradient_to_hex.is_none()
                && underline.is_none() && curve_amount.is_none() && gradient_angle.is_none()
            {
                // Clear gradient
                *gc = None;
            }
        }
        _ => return Err("Layer is not a Text layer.".into()),
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced shapes ─────────────────────────────────────────────────

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn add_shape_advanced(
    state: tauri::State<'_, SharedProjectState>,
    shape_type: String,
    width: u32,
    height: u32,
    fill_hex: String,
    border_radius: Option<u32>,
    border_hex: Option<String>,
    border_width: Option<u32>,
    sides: Option<u32>,
    fill_gradient_from: Option<String>,
    fill_gradient_to: Option<String>,
    fill_gradient_angle: Option<f32>,
    x: Option<i64>,
    y: Option<i64>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let cw = s.current.canvas_width as i64;
    let ch = s.current.canvas_height as i64;
    let lx = x.unwrap_or((cw - width as i64) / 2);
    let ly = y.unwrap_or((ch - height as i64) / 2);
    let fill_color = parse_hex_color(&fill_hex);
    let b_color = border_hex.as_deref().map(parse_hex_color).unwrap_or([255,255,255,255]);
    let fill_gradient = match (fill_gradient_from.as_deref(), fill_gradient_to.as_deref()) {
        (Some(a), Some(b)) => Some(BgGradient {
            from: hex_to_rgba8(a),
            to: hex_to_rgba8(b),
            angle: fill_gradient_angle.unwrap_or(0.0),
        }),
        _ => None,
    };
    let id = format!("layer_{}", uuid_v4());
    let name = match shape_type.as_str() {
        "line"     => "Line",
        "arrow"    => "Arrow",
        "triangle" => "Triangle",
        "star"     => "Star",
        "polygon"  => "Polygon",
        "circle"   => "Circle",
        "badge"    => "Badge",
        _          => "Rectangle",
    }.to_string();
    let layer = Layer {
        id: id.clone(),
        name,
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
            shape_type: shape_type.clone(),
            width,
            height,
            fill_color,
            border_radius: border_radius.unwrap_or(0),
            border_color: b_color,
            border_width: border_width.unwrap_or(0),
            sides: sides.unwrap_or(5),
            corner_radii: None,
            fill_gradient,
        },
    };
    s.current.layers.push(layer);
    render_and_respond(&mut s)
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_micros();
    format!("{:x}", ms ^ (ms >> 16))
}

#[tauri::command]
pub fn paste_layers_from_json(
    state: tauri::State<'_, SharedProjectState>,
    layers_json: String,
) -> Result<ProjectResponse, String> {
    let parsed: Vec<Layer> =
        serde_json::from_str(&layers_json).map_err(|e| format!("Invalid clipboard layers: {}", e))?;
    if parsed.is_empty() {
        return Err("Clipboard has no layers.".to_string());
    }
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    for mut l in parsed {
        l.id = s.next_id();
        l.x += 24;
        l.y += 24;
        l.group_id = None;
        s.current.layers.push(l);
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Font management ─────────────────────────────────────────────────

#[tauri::command]
pub fn import_font(source_path: String) -> Result<String, String> {
    let p = std::path::Path::new(&source_path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext != "ttf" && ext != "otf" {
        return Err("Only .ttf and .otf font files are supported.".into());
    }
    let filename = p.file_name().and_then(|n| n.to_str())
        .ok_or("Invalid file name")?
        .to_string();
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    std::fs::create_dir_all(&fonts_dir).map_err(|e| e.to_string())?;
    let dest = fonts_dir.join(&filename);
    std::fs::copy(&source_path, &dest).map_err(|e| format!("Copy failed: {}", e))?;
    Ok(filename)
}

#[tauri::command]
pub fn list_imported_fonts() -> Result<Vec<String>, String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    if !fonts_dir.exists() {
        return Ok(vec![]);
    }
    let mut names = vec![];
    for entry in std::fs::read_dir(&fonts_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let ext = std::path::Path::new(&name)
            .extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext == "ttf" || ext == "otf" {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

#[tauri::command]
pub fn delete_imported_font(filename: String) -> Result<(), String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    let path = fonts_dir.join(&filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Phase 3: Drop shadow / glow / mask / clip ────────────────────────────────

fn hex_to_rgba8(hex: &str) -> [u8; 4] {
    let h = hex.trim_start_matches('#');
    if h.len() == 6 {
        let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(0);
        [r, g, b, 255]
    } else {
        [0, 0, 0, 255]
    }
}

#[tauri::command]
pub fn update_layer_shadow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    color_hex: Option<String>,
    blur: Option<u32>,
    distance: Option<u32>,
    angle: Option<f32>,
    opacity: Option<u8>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.shadow.enabled = enabled;
    if let Some(c) = color_hex { layer.shadow.color = hex_to_rgba8(&c); }
    if let Some(v) = blur { layer.shadow.blur = v; }
    if let Some(v) = distance { layer.shadow.distance = v; }
    if let Some(v) = angle { layer.shadow.angle = v; }
    if let Some(v) = opacity { layer.shadow.opacity = v; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_glow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    kind: String, // "outer" | "inner"
    enabled: bool,
    color_hex: Option<String>,
    intensity: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    let glow = if kind == "inner" { &mut layer.inner_glow } else { &mut layer.outer_glow };
    glow.enabled = enabled;
    if let Some(c) = color_hex { glow.color = hex_to_rgba8(&c); }
    if let Some(v) = intensity { glow.intensity = v.clamp(1, 60); }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_mask(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    mask_shape: Option<String>, // None | "circle" | "rounded" | "star"
    mask_radius: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.mask_shape = mask_shape;
    if let Some(r) = mask_radius { layer.mask_radius = r; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn set_clip_to_below(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.clip_to_below = enabled;
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced text ───────────────────────────────────────────────────

#[tauri::command]
pub fn update_text_advanced(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    underline: Option<bool>,
    gradient_from_hex: Option<String>,
    gradient_to_hex: Option<String>,
    gradient_angle: Option<f32>,
    curve_amount: Option<f32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    match &mut layer.kind {
        LayerKind::Text3D {
            underline: ul,
            gradient_colors: gc,
            gradient_angle: ga,
            curve_amount: ca,
            ..
        } => {
            if let Some(v) = underline { *ul = v; }
            if let Some(v) = curve_amount { *ca = v.clamp(-180.0, 180.0); }
            if let Some(v) = gradient_angle { *ga = v; }
            // If either gradient color given, build/update gradient_colors pair
            if gradient_from_hex.is_some() || gradient_to_hex.is_some() {
                let cur = gc.unwrap_or([[255,255,255,255],[255,255,0,255]]);
                let from = gradient_from_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[0]);
                let to   = gradient_to_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[1]);
                *gc = Some([from, to]);
            } else if gradient_from_hex.is_none() && gradient_to_hex.is_none()
                && underline.is_none() && curve_amount.is_none() && gradient_angle.is_none()
            {
                // Clear gradient
                *gc = None;
            }
        }
        _ => return Err("Layer is not a Text layer.".into()),
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced shapes ─────────────────────────────────────────────────

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn add_shape_advanced(
    state: tauri::State<'_, SharedProjectState>,
    shape_type: String,
    width: u32,
    height: u32,
    fill_hex: String,
    border_radius: Option<u32>,
    border_hex: Option<String>,
    border_width: Option<u32>,
    sides: Option<u32>,
    fill_gradient_from: Option<String>,
    fill_gradient_to: Option<String>,
    fill_gradient_angle: Option<f32>,
    x: Option<i64>,
    y: Option<i64>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let cw = s.current.canvas_width as i64;
    let ch = s.current.canvas_height as i64;
    let lx = x.unwrap_or((cw - width as i64) / 2);
    let ly = y.unwrap_or((ch - height as i64) / 2);
    let fill_color = parse_hex_color(&fill_hex);
    let b_color = border_hex.as_deref().map(parse_hex_color).unwrap_or([255,255,255,255]);
    let fill_gradient = match (fill_gradient_from.as_deref(), fill_gradient_to.as_deref()) {
        (Some(a), Some(b)) => Some(BgGradient {
            from: hex_to_rgba8(a),
            to: hex_to_rgba8(b),
            angle: fill_gradient_angle.unwrap_or(0.0),
        }),
        _ => None,
    };
    let id = format!("layer_{}", uuid_v4());
    let name = match shape_type.as_str() {
        "line"     => "Line",
        "arrow"    => "Arrow",
        "triangle" => "Triangle",
        "star"     => "Star",
        "polygon"  => "Polygon",
        "circle"   => "Circle",
        "badge"    => "Badge",
        _          => "Rectangle",
    }.to_string();
    let layer = Layer {
        id: id.clone(),
        name,
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
            shape_type: shape_type.clone(),
            width,
            height,
            fill_color,
            border_radius: border_radius.unwrap_or(0),
            border_color: b_color,
            border_width: border_width.unwrap_or(0),
            sides: sides.unwrap_or(5),
            corner_radii: None,
            fill_gradient,
        },
    };
    s.current.layers.push(layer);
    render_and_respond(&mut s)
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_micros();
    format!("{:x}", ms ^ (ms >> 16))
}

#[tauri::command]
pub fn add_image_bytes_layer(
    state: tauri::State<'_, SharedProjectState>,
    filename: String,
    data_base64: String,
    x: Option<i64>,
    y: Option<i64>,
) -> Result<ProjectResponse, String> {
    use base64::Engine;
    let raw_b64 = data_base64
        .split_once(',')
        .map(|(_, b)| b)
        .unwrap_or(&data_base64);
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(raw_b64.trim())
        .map_err(|e| format!("Invalid base64 image: {}", e))?;

    let pasted_dir = user_data_root().join("pasted");
    std::fs::create_dir_all(&pasted_dir).map_err(|e| e.to_string())?;
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let safe_name = Path::new(&filename)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("pasted.png");
    let dest = pasted_dir.join(format!("{}_{}", ms, safe_name));
    std::fs::write(&dest, &bytes).map_err(|e| format!("Failed to save pasted image: {}", e))?;

    let _ = image::open(&dest).map_err(|e| format!("Unsupported image format: {}", e))?;
    let layer_name = Path::new(safe_name)
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("Pasted Image")
        .to_string();

    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let id = s.next_id();
    s.current.layers.push(Layer {
        id,
        name: layer_name,
        visible: true,
        locked: false,
        opacity: 1.0,
        x: x.unwrap_or(60),
        y: y.unwrap_or(60),
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
        kind: LayerKind::Image {
            source_path: dest.to_string_lossy().to_string(),
            bg_remove_color: None,
            bg_remove_tolerance: 30,
        },
    });
    render_and_respond(&mut s)
}

// ── Phase 3: Font management ─────────────────────────────────────────────────

#[tauri::command]
pub fn import_font(source_path: String) -> Result<String, String> {
    let p = std::path::Path::new(&source_path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext != "ttf" && ext != "otf" {
        return Err("Only .ttf and .otf font files are supported.".into());
    }
    let filename = p.file_name().and_then(|n| n.to_str())
        .ok_or("Invalid file name")?
        .to_string();
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    std::fs::create_dir_all(&fonts_dir).map_err(|e| e.to_string())?;
    let dest = fonts_dir.join(&filename);
    std::fs::copy(&source_path, &dest).map_err(|e| format!("Copy failed: {}", e))?;
    Ok(filename)
}

#[tauri::command]
pub fn list_imported_fonts() -> Result<Vec<String>, String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    if !fonts_dir.exists() {
        return Ok(vec![]);
    }
    let mut names = vec![];
    for entry in std::fs::read_dir(&fonts_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let ext = std::path::Path::new(&name)
            .extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext == "ttf" || ext == "otf" {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

#[tauri::command]
pub fn delete_imported_font(filename: String) -> Result<(), String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    let path = fonts_dir.join(&filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Phase 3: Drop shadow / glow / mask / clip ────────────────────────────────

fn hex_to_rgba8(hex: &str) -> [u8; 4] {
    let h = hex.trim_start_matches('#');
    if h.len() == 6 {
        let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(0);
        [r, g, b, 255]
    } else {
        [0, 0, 0, 255]
    }
}

#[tauri::command]
pub fn update_layer_shadow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    color_hex: Option<String>,
    blur: Option<u32>,
    distance: Option<u32>,
    angle: Option<f32>,
    opacity: Option<u8>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.shadow.enabled = enabled;
    if let Some(c) = color_hex { layer.shadow.color = hex_to_rgba8(&c); }
    if let Some(v) = blur { layer.shadow.blur = v; }
    if let Some(v) = distance { layer.shadow.distance = v; }
    if let Some(v) = angle { layer.shadow.angle = v; }
    if let Some(v) = opacity { layer.shadow.opacity = v; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_glow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    kind: String, // "outer" | "inner"
    enabled: bool,
    color_hex: Option<String>,
    intensity: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    let glow = if kind == "inner" { &mut layer.inner_glow } else { &mut layer.outer_glow };
    glow.enabled = enabled;
    if let Some(c) = color_hex { glow.color = hex_to_rgba8(&c); }
    if let Some(v) = intensity { glow.intensity = v.clamp(1, 60); }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_mask(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    mask_shape: Option<String>, // None | "circle" | "rounded" | "star"
    mask_radius: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.mask_shape = mask_shape;
    if let Some(r) = mask_radius { layer.mask_radius = r; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn set_clip_to_below(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.clip_to_below = enabled;
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced text ───────────────────────────────────────────────────

#[tauri::command]
pub fn update_text_advanced(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    underline: Option<bool>,
    gradient_from_hex: Option<String>,
    gradient_to_hex: Option<String>,
    gradient_angle: Option<f32>,
    curve_amount: Option<f32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    match &mut layer.kind {
        LayerKind::Text3D {
            underline: ul,
            gradient_colors: gc,
            gradient_angle: ga,
            curve_amount: ca,
            ..
        } => {
            if let Some(v) = underline { *ul = v; }
            if let Some(v) = curve_amount { *ca = v.clamp(-180.0, 180.0); }
            if let Some(v) = gradient_angle { *ga = v; }
            // If either gradient color given, build/update gradient_colors pair
            if gradient_from_hex.is_some() || gradient_to_hex.is_some() {
                let cur = gc.unwrap_or([[255,255,255,255],[255,255,0,255]]);
                let from = gradient_from_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[0]);
                let to   = gradient_to_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[1]);
                *gc = Some([from, to]);
            } else if gradient_from_hex.is_none() && gradient_to_hex.is_none()
                && underline.is_none() && curve_amount.is_none() && gradient_angle.is_none()
            {
                // Clear gradient
                *gc = None;
            }
        }
        _ => return Err("Layer is not a Text layer.".into()),
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced shapes ─────────────────────────────────────────────────

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn add_shape_advanced(
    state: tauri::State<'_, SharedProjectState>,
    shape_type: String,
    width: u32,
    height: u32,
    fill_hex: String,
    border_radius: Option<u32>,
    border_hex: Option<String>,
    border_width: Option<u32>,
    sides: Option<u32>,
    fill_gradient_from: Option<String>,
    fill_gradient_to: Option<String>,
    fill_gradient_angle: Option<f32>,
    x: Option<i64>,
    y: Option<i64>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let cw = s.current.canvas_width as i64;
    let ch = s.current.canvas_height as i64;
    let lx = x.unwrap_or((cw - width as i64) / 2);
    let ly = y.unwrap_or((ch - height as i64) / 2);
    let fill_color = parse_hex_color(&fill_hex);
    let b_color = border_hex.as_deref().map(parse_hex_color).unwrap_or([255,255,255,255]);
    let fill_gradient = match (fill_gradient_from.as_deref(), fill_gradient_to.as_deref()) {
        (Some(a), Some(b)) => Some(BgGradient {
            from: hex_to_rgba8(a),
            to: hex_to_rgba8(b),
            angle: fill_gradient_angle.unwrap_or(0.0),
        }),
        _ => None,
    };
    let id = format!("layer_{}", uuid_v4());
    let name = match shape_type.as_str() {
        "line"     => "Line",
        "arrow"    => "Arrow",
        "triangle" => "Triangle",
        "star"     => "Star",
        "polygon"  => "Polygon",
        "circle"   => "Circle",
        "badge"    => "Badge",
        _          => "Rectangle",
    }.to_string();
    let layer = Layer {
        id: id.clone(),
        name,
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
            shape_type: shape_type.clone(),
            width,
            height,
            fill_color,
            border_radius: border_radius.unwrap_or(0),
            border_color: b_color,
            border_width: border_width.unwrap_or(0),
            sides: sides.unwrap_or(5),
            corner_radii: None,
            fill_gradient,
        },
    };
    s.current.layers.push(layer);
    render_and_respond(&mut s)
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_micros();
    format!("{:x}", ms ^ (ms >> 16))
}

#[tauri::command]
pub fn group_layers(
    state: tauri::State<'_, SharedProjectState>,
    layer_ids: Vec<String>,
    name: Option<String>,
    parent_group_id: Option<String>,
) -> Result<ProjectResponse, String> {
    if layer_ids.is_empty() {
        return Err("Select at least one layer to group.".to_string());
    }
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let gid = format!("group-{}", s.next_layer_seq);
    s.next_layer_seq += 1;
    let group_name = name.unwrap_or_else(|| format!("Group {}", s.current.groups.len() + 1));
    s.current.groups.push(LayerGroup {
        id: gid.clone(),
        name: group_name,
        parent_group_id,
        visible: true,
        locked: false,
        collapsed: false,
    });
    for l in &mut s.current.layers {
        if layer_ids.contains(&l.id) {
            l.group_id = Some(gid.clone());
        }
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Font management ─────────────────────────────────────────────────

#[tauri::command]
pub fn import_font(source_path: String) -> Result<String, String> {
    let p = std::path::Path::new(&source_path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext != "ttf" && ext != "otf" {
        return Err("Only .ttf and .otf font files are supported.".into());
    }
    let filename = p.file_name().and_then(|n| n.to_str())
        .ok_or("Invalid file name")?
        .to_string();
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    std::fs::create_dir_all(&fonts_dir).map_err(|e| e.to_string())?;
    let dest = fonts_dir.join(&filename);
    std::fs::copy(&source_path, &dest).map_err(|e| format!("Copy failed: {}", e))?;
    Ok(filename)
}

#[tauri::command]
pub fn list_imported_fonts() -> Result<Vec<String>, String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    if !fonts_dir.exists() {
        return Ok(vec![]);
    }
    let mut names = vec![];
    for entry in std::fs::read_dir(&fonts_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let ext = std::path::Path::new(&name)
            .extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext == "ttf" || ext == "otf" {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

#[tauri::command]
pub fn delete_imported_font(filename: String) -> Result<(), String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    let path = fonts_dir.join(&filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Phase 3: Drop shadow / glow / mask / clip ────────────────────────────────

fn hex_to_rgba8(hex: &str) -> [u8; 4] {
    let h = hex.trim_start_matches('#');
    if h.len() == 6 {
        let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(0);
        [r, g, b, 255]
    } else {
        [0, 0, 0, 255]
    }
}

#[tauri::command]
pub fn update_layer_shadow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    color_hex: Option<String>,
    blur: Option<u32>,
    distance: Option<u32>,
    angle: Option<f32>,
    opacity: Option<u8>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.shadow.enabled = enabled;
    if let Some(c) = color_hex { layer.shadow.color = hex_to_rgba8(&c); }
    if let Some(v) = blur { layer.shadow.blur = v; }
    if let Some(v) = distance { layer.shadow.distance = v; }
    if let Some(v) = angle { layer.shadow.angle = v; }
    if let Some(v) = opacity { layer.shadow.opacity = v; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_glow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    kind: String, // "outer" | "inner"
    enabled: bool,
    color_hex: Option<String>,
    intensity: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    let glow = if kind == "inner" { &mut layer.inner_glow } else { &mut layer.outer_glow };
    glow.enabled = enabled;
    if let Some(c) = color_hex { glow.color = hex_to_rgba8(&c); }
    if let Some(v) = intensity { glow.intensity = v.clamp(1, 60); }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_mask(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    mask_shape: Option<String>, // None | "circle" | "rounded" | "star"
    mask_radius: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.mask_shape = mask_shape;
    if let Some(r) = mask_radius { layer.mask_radius = r; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn set_clip_to_below(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.clip_to_below = enabled;
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced text ───────────────────────────────────────────────────

#[tauri::command]
pub fn update_text_advanced(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    underline: Option<bool>,
    gradient_from_hex: Option<String>,
    gradient_to_hex: Option<String>,
    gradient_angle: Option<f32>,
    curve_amount: Option<f32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    match &mut layer.kind {
        LayerKind::Text3D {
            underline: ul,
            gradient_colors: gc,
            gradient_angle: ga,
            curve_amount: ca,
            ..
        } => {
            if let Some(v) = underline { *ul = v; }
            if let Some(v) = curve_amount { *ca = v.clamp(-180.0, 180.0); }
            if let Some(v) = gradient_angle { *ga = v; }
            // If either gradient color given, build/update gradient_colors pair
            if gradient_from_hex.is_some() || gradient_to_hex.is_some() {
                let cur = gc.unwrap_or([[255,255,255,255],[255,255,0,255]]);
                let from = gradient_from_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[0]);
                let to   = gradient_to_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[1]);
                *gc = Some([from, to]);
            } else if gradient_from_hex.is_none() && gradient_to_hex.is_none()
                && underline.is_none() && curve_amount.is_none() && gradient_angle.is_none()
            {
                // Clear gradient
                *gc = None;
            }
        }
        _ => return Err("Layer is not a Text layer.".into()),
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced shapes ─────────────────────────────────────────────────

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn add_shape_advanced(
    state: tauri::State<'_, SharedProjectState>,
    shape_type: String,
    width: u32,
    height: u32,
    fill_hex: String,
    border_radius: Option<u32>,
    border_hex: Option<String>,
    border_width: Option<u32>,
    sides: Option<u32>,
    fill_gradient_from: Option<String>,
    fill_gradient_to: Option<String>,
    fill_gradient_angle: Option<f32>,
    x: Option<i64>,
    y: Option<i64>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let cw = s.current.canvas_width as i64;
    let ch = s.current.canvas_height as i64;
    let lx = x.unwrap_or((cw - width as i64) / 2);
    let ly = y.unwrap_or((ch - height as i64) / 2);
    let fill_color = parse_hex_color(&fill_hex);
    let b_color = border_hex.as_deref().map(parse_hex_color).unwrap_or([255,255,255,255]);
    let fill_gradient = match (fill_gradient_from.as_deref(), fill_gradient_to.as_deref()) {
        (Some(a), Some(b)) => Some(BgGradient {
            from: hex_to_rgba8(a),
            to: hex_to_rgba8(b),
            angle: fill_gradient_angle.unwrap_or(0.0),
        }),
        _ => None,
    };
    let id = format!("layer_{}", uuid_v4());
    let name = match shape_type.as_str() {
        "line"     => "Line",
        "arrow"    => "Arrow",
        "triangle" => "Triangle",
        "star"     => "Star",
        "polygon"  => "Polygon",
        "circle"   => "Circle",
        "badge"    => "Badge",
        _          => "Rectangle",
    }.to_string();
    let layer = Layer {
        id: id.clone(),
        name,
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
            shape_type: shape_type.clone(),
            width,
            height,
            fill_color,
            border_radius: border_radius.unwrap_or(0),
            border_color: b_color,
            border_width: border_width.unwrap_or(0),
            sides: sides.unwrap_or(5),
            corner_radii: None,
            fill_gradient,
        },
    };
    s.current.layers.push(layer);
    render_and_respond(&mut s)
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_micros();
    format!("{:x}", ms ^ (ms >> 16))
}

#[tauri::command]
pub fn ungroup_layers(
    state: tauri::State<'_, SharedProjectState>,
    group_id: String,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let parent = s
        .current
        .groups
        .iter()
        .find(|g| g.id == group_id)
        .and_then(|g| g.parent_group_id.clone());
    for l in &mut s.current.layers {
        if l.group_id.as_deref() == Some(group_id.as_str()) {
            l.group_id = parent.clone();
        }
    }
    for g in &mut s.current.groups {
        if g.parent_group_id.as_deref() == Some(group_id.as_str()) {
            g.parent_group_id = parent.clone();
        }
    }
    s.current.groups.retain(|g| g.id != group_id);
    render_and_respond(&mut s)
}

// ── Phase 3: Font management ─────────────────────────────────────────────────

#[tauri::command]
pub fn import_font(source_path: String) -> Result<String, String> {
    let p = std::path::Path::new(&source_path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext != "ttf" && ext != "otf" {
        return Err("Only .ttf and .otf font files are supported.".into());
    }
    let filename = p.file_name().and_then(|n| n.to_str())
        .ok_or("Invalid file name")?
        .to_string();
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    std::fs::create_dir_all(&fonts_dir).map_err(|e| e.to_string())?;
    let dest = fonts_dir.join(&filename);
    std::fs::copy(&source_path, &dest).map_err(|e| format!("Copy failed: {}", e))?;
    Ok(filename)
}

#[tauri::command]
pub fn list_imported_fonts() -> Result<Vec<String>, String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    if !fonts_dir.exists() {
        return Ok(vec![]);
    }
    let mut names = vec![];
    for entry in std::fs::read_dir(&fonts_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let ext = std::path::Path::new(&name)
            .extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext == "ttf" || ext == "otf" {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

#[tauri::command]
pub fn delete_imported_font(filename: String) -> Result<(), String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    let path = fonts_dir.join(&filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Phase 3: Drop shadow / glow / mask / clip ────────────────────────────────

fn hex_to_rgba8(hex: &str) -> [u8; 4] {
    let h = hex.trim_start_matches('#');
    if h.len() == 6 {
        let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(0);
        [r, g, b, 255]
    } else {
        [0, 0, 0, 255]
    }
}

#[tauri::command]
pub fn update_layer_shadow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    color_hex: Option<String>,
    blur: Option<u32>,
    distance: Option<u32>,
    angle: Option<f32>,
    opacity: Option<u8>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.shadow.enabled = enabled;
    if let Some(c) = color_hex { layer.shadow.color = hex_to_rgba8(&c); }
    if let Some(v) = blur { layer.shadow.blur = v; }
    if let Some(v) = distance { layer.shadow.distance = v; }
    if let Some(v) = angle { layer.shadow.angle = v; }
    if let Some(v) = opacity { layer.shadow.opacity = v; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_glow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    kind: String, // "outer" | "inner"
    enabled: bool,
    color_hex: Option<String>,
    intensity: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    let glow = if kind == "inner" { &mut layer.inner_glow } else { &mut layer.outer_glow };
    glow.enabled = enabled;
    if let Some(c) = color_hex { glow.color = hex_to_rgba8(&c); }
    if let Some(v) = intensity { glow.intensity = v.clamp(1, 60); }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_mask(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    mask_shape: Option<String>, // None | "circle" | "rounded" | "star"
    mask_radius: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.mask_shape = mask_shape;
    if let Some(r) = mask_radius { layer.mask_radius = r; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn set_clip_to_below(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.clip_to_below = enabled;
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced text ───────────────────────────────────────────────────

#[tauri::command]
pub fn update_text_advanced(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    underline: Option<bool>,
    gradient_from_hex: Option<String>,
    gradient_to_hex: Option<String>,
    gradient_angle: Option<f32>,
    curve_amount: Option<f32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    match &mut layer.kind {
        LayerKind::Text3D {
            underline: ul,
            gradient_colors: gc,
            gradient_angle: ga,
            curve_amount: ca,
            ..
        } => {
            if let Some(v) = underline { *ul = v; }
            if let Some(v) = curve_amount { *ca = v.clamp(-180.0, 180.0); }
            if let Some(v) = gradient_angle { *ga = v; }
            // If either gradient color given, build/update gradient_colors pair
            if gradient_from_hex.is_some() || gradient_to_hex.is_some() {
                let cur = gc.unwrap_or([[255,255,255,255],[255,255,0,255]]);
                let from = gradient_from_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[0]);
                let to   = gradient_to_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[1]);
                *gc = Some([from, to]);
            } else if gradient_from_hex.is_none() && gradient_to_hex.is_none()
                && underline.is_none() && curve_amount.is_none() && gradient_angle.is_none()
            {
                // Clear gradient
                *gc = None;
            }
        }
        _ => return Err("Layer is not a Text layer.".into()),
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced shapes ─────────────────────────────────────────────────

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn add_shape_advanced(
    state: tauri::State<'_, SharedProjectState>,
    shape_type: String,
    width: u32,
    height: u32,
    fill_hex: String,
    border_radius: Option<u32>,
    border_hex: Option<String>,
    border_width: Option<u32>,
    sides: Option<u32>,
    fill_gradient_from: Option<String>,
    fill_gradient_to: Option<String>,
    fill_gradient_angle: Option<f32>,
    x: Option<i64>,
    y: Option<i64>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let cw = s.current.canvas_width as i64;
    let ch = s.current.canvas_height as i64;
    let lx = x.unwrap_or((cw - width as i64) / 2);
    let ly = y.unwrap_or((ch - height as i64) / 2);
    let fill_color = parse_hex_color(&fill_hex);
    let b_color = border_hex.as_deref().map(parse_hex_color).unwrap_or([255,255,255,255]);
    let fill_gradient = match (fill_gradient_from.as_deref(), fill_gradient_to.as_deref()) {
        (Some(a), Some(b)) => Some(BgGradient {
            from: hex_to_rgba8(a),
            to: hex_to_rgba8(b),
            angle: fill_gradient_angle.unwrap_or(0.0),
        }),
        _ => None,
    };
    let id = format!("layer_{}", uuid_v4());
    let name = match shape_type.as_str() {
        "line"     => "Line",
        "arrow"    => "Arrow",
        "triangle" => "Triangle",
        "star"     => "Star",
        "polygon"  => "Polygon",
        "circle"   => "Circle",
        "badge"    => "Badge",
        _          => "Rectangle",
    }.to_string();
    let layer = Layer {
        id: id.clone(),
        name,
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
            shape_type: shape_type.clone(),
            width,
            height,
            fill_color,
            border_radius: border_radius.unwrap_or(0),
            border_color: b_color,
            border_width: border_width.unwrap_or(0),
            sides: sides.unwrap_or(5),
            corner_radii: None,
            fill_gradient,
        },
    };
    s.current.layers.push(layer);
    render_and_respond(&mut s)
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_micros();
    format!("{:x}", ms ^ (ms >> 16))
}

#[tauri::command]
pub fn toggle_group_visibility(
    state: tauri::State<'_, SharedProjectState>,
    group_id: String,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let grp = s
        .current
        .groups
        .iter_mut()
        .find(|g| g.id == group_id)
        .ok_or_else(|| format!("Group '{}' not found.", group_id))?;
    grp.visible = !grp.visible;
    render_and_respond(&mut s)
}

// ── Phase 3: Font management ─────────────────────────────────────────────────

#[tauri::command]
pub fn import_font(source_path: String) -> Result<String, String> {
    let p = std::path::Path::new(&source_path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext != "ttf" && ext != "otf" {
        return Err("Only .ttf and .otf font files are supported.".into());
    }
    let filename = p.file_name().and_then(|n| n.to_str())
        .ok_or("Invalid file name")?
        .to_string();
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    std::fs::create_dir_all(&fonts_dir).map_err(|e| e.to_string())?;
    let dest = fonts_dir.join(&filename);
    std::fs::copy(&source_path, &dest).map_err(|e| format!("Copy failed: {}", e))?;
    Ok(filename)
}

#[tauri::command]
pub fn list_imported_fonts() -> Result<Vec<String>, String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    if !fonts_dir.exists() {
        return Ok(vec![]);
    }
    let mut names = vec![];
    for entry in std::fs::read_dir(&fonts_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let ext = std::path::Path::new(&name)
            .extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext == "ttf" || ext == "otf" {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

#[tauri::command]
pub fn delete_imported_font(filename: String) -> Result<(), String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    let path = fonts_dir.join(&filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Phase 3: Drop shadow / glow / mask / clip ────────────────────────────────

fn hex_to_rgba8(hex: &str) -> [u8; 4] {
    let h = hex.trim_start_matches('#');
    if h.len() == 6 {
        let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(0);
        [r, g, b, 255]
    } else {
        [0, 0, 0, 255]
    }
}

#[tauri::command]
pub fn update_layer_shadow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    color_hex: Option<String>,
    blur: Option<u32>,
    distance: Option<u32>,
    angle: Option<f32>,
    opacity: Option<u8>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.shadow.enabled = enabled;
    if let Some(c) = color_hex { layer.shadow.color = hex_to_rgba8(&c); }
    if let Some(v) = blur { layer.shadow.blur = v; }
    if let Some(v) = distance { layer.shadow.distance = v; }
    if let Some(v) = angle { layer.shadow.angle = v; }
    if let Some(v) = opacity { layer.shadow.opacity = v; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_glow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    kind: String, // "outer" | "inner"
    enabled: bool,
    color_hex: Option<String>,
    intensity: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    let glow = if kind == "inner" { &mut layer.inner_glow } else { &mut layer.outer_glow };
    glow.enabled = enabled;
    if let Some(c) = color_hex { glow.color = hex_to_rgba8(&c); }
    if let Some(v) = intensity { glow.intensity = v.clamp(1, 60); }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_mask(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    mask_shape: Option<String>, // None | "circle" | "rounded" | "star"
    mask_radius: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.mask_shape = mask_shape;
    if let Some(r) = mask_radius { layer.mask_radius = r; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn set_clip_to_below(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.clip_to_below = enabled;
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced text ───────────────────────────────────────────────────

#[tauri::command]
pub fn update_text_advanced(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    underline: Option<bool>,
    gradient_from_hex: Option<String>,
    gradient_to_hex: Option<String>,
    gradient_angle: Option<f32>,
    curve_amount: Option<f32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    match &mut layer.kind {
        LayerKind::Text3D {
            underline: ul,
            gradient_colors: gc,
            gradient_angle: ga,
            curve_amount: ca,
            ..
        } => {
            if let Some(v) = underline { *ul = v; }
            if let Some(v) = curve_amount { *ca = v.clamp(-180.0, 180.0); }
            if let Some(v) = gradient_angle { *ga = v; }
            // If either gradient color given, build/update gradient_colors pair
            if gradient_from_hex.is_some() || gradient_to_hex.is_some() {
                let cur = gc.unwrap_or([[255,255,255,255],[255,255,0,255]]);
                let from = gradient_from_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[0]);
                let to   = gradient_to_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[1]);
                *gc = Some([from, to]);
            } else if gradient_from_hex.is_none() && gradient_to_hex.is_none()
                && underline.is_none() && curve_amount.is_none() && gradient_angle.is_none()
            {
                // Clear gradient
                *gc = None;
            }
        }
        _ => return Err("Layer is not a Text layer.".into()),
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced shapes ─────────────────────────────────────────────────

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn add_shape_advanced(
    state: tauri::State<'_, SharedProjectState>,
    shape_type: String,
    width: u32,
    height: u32,
    fill_hex: String,
    border_radius: Option<u32>,
    border_hex: Option<String>,
    border_width: Option<u32>,
    sides: Option<u32>,
    fill_gradient_from: Option<String>,
    fill_gradient_to: Option<String>,
    fill_gradient_angle: Option<f32>,
    x: Option<i64>,
    y: Option<i64>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let cw = s.current.canvas_width as i64;
    let ch = s.current.canvas_height as i64;
    let lx = x.unwrap_or((cw - width as i64) / 2);
    let ly = y.unwrap_or((ch - height as i64) / 2);
    let fill_color = parse_hex_color(&fill_hex);
    let b_color = border_hex.as_deref().map(parse_hex_color).unwrap_or([255,255,255,255]);
    let fill_gradient = match (fill_gradient_from.as_deref(), fill_gradient_to.as_deref()) {
        (Some(a), Some(b)) => Some(BgGradient {
            from: hex_to_rgba8(a),
            to: hex_to_rgba8(b),
            angle: fill_gradient_angle.unwrap_or(0.0),
        }),
        _ => None,
    };
    let id = format!("layer_{}", uuid_v4());
    let name = match shape_type.as_str() {
        "line"     => "Line",
        "arrow"    => "Arrow",
        "triangle" => "Triangle",
        "star"     => "Star",
        "polygon"  => "Polygon",
        "circle"   => "Circle",
        "badge"    => "Badge",
        _          => "Rectangle",
    }.to_string();
    let layer = Layer {
        id: id.clone(),
        name,
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
            shape_type: shape_type.clone(),
            width,
            height,
            fill_color,
            border_radius: border_radius.unwrap_or(0),
            border_color: b_color,
            border_width: border_width.unwrap_or(0),
            sides: sides.unwrap_or(5),
            corner_radii: None,
            fill_gradient,
        },
    };
    s.current.layers.push(layer);
    render_and_respond(&mut s)
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_micros();
    format!("{:x}", ms ^ (ms >> 16))
}

#[tauri::command]
pub fn toggle_group_lock(
    state: tauri::State<'_, SharedProjectState>,
    group_id: String,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let grp = s
        .current
        .groups
        .iter_mut()
        .find(|g| g.id == group_id)
        .ok_or_else(|| format!("Group '{}' not found.", group_id))?;
    let next_lock = !grp.locked;
    grp.locked = next_lock;
    for l in &mut s.current.layers {
        if l.group_id.as_deref() == Some(group_id.as_str()) {
            l.locked = next_lock;
        }
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Font management ─────────────────────────────────────────────────

#[tauri::command]
pub fn import_font(source_path: String) -> Result<String, String> {
    let p = std::path::Path::new(&source_path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext != "ttf" && ext != "otf" {
        return Err("Only .ttf and .otf font files are supported.".into());
    }
    let filename = p.file_name().and_then(|n| n.to_str())
        .ok_or("Invalid file name")?
        .to_string();
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    std::fs::create_dir_all(&fonts_dir).map_err(|e| e.to_string())?;
    let dest = fonts_dir.join(&filename);
    std::fs::copy(&source_path, &dest).map_err(|e| format!("Copy failed: {}", e))?;
    Ok(filename)
}

#[tauri::command]
pub fn list_imported_fonts() -> Result<Vec<String>, String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    if !fonts_dir.exists() {
        return Ok(vec![]);
    }
    let mut names = vec![];
    for entry in std::fs::read_dir(&fonts_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let ext = std::path::Path::new(&name)
            .extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext == "ttf" || ext == "otf" {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

#[tauri::command]
pub fn delete_imported_font(filename: String) -> Result<(), String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    let path = fonts_dir.join(&filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Phase 3: Drop shadow / glow / mask / clip ────────────────────────────────

fn hex_to_rgba8(hex: &str) -> [u8; 4] {
    let h = hex.trim_start_matches('#');
    if h.len() == 6 {
        let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(0);
        [r, g, b, 255]
    } else {
        [0, 0, 0, 255]
    }
}

#[tauri::command]
pub fn update_layer_shadow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    color_hex: Option<String>,
    blur: Option<u32>,
    distance: Option<u32>,
    angle: Option<f32>,
    opacity: Option<u8>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.shadow.enabled = enabled;
    if let Some(c) = color_hex { layer.shadow.color = hex_to_rgba8(&c); }
    if let Some(v) = blur { layer.shadow.blur = v; }
    if let Some(v) = distance { layer.shadow.distance = v; }
    if let Some(v) = angle { layer.shadow.angle = v; }
    if let Some(v) = opacity { layer.shadow.opacity = v; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_glow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    kind: String, // "outer" | "inner"
    enabled: bool,
    color_hex: Option<String>,
    intensity: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    let glow = if kind == "inner" { &mut layer.inner_glow } else { &mut layer.outer_glow };
    glow.enabled = enabled;
    if let Some(c) = color_hex { glow.color = hex_to_rgba8(&c); }
    if let Some(v) = intensity { glow.intensity = v.clamp(1, 60); }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_mask(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    mask_shape: Option<String>, // None | "circle" | "rounded" | "star"
    mask_radius: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.mask_shape = mask_shape;
    if let Some(r) = mask_radius { layer.mask_radius = r; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn set_clip_to_below(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.clip_to_below = enabled;
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced text ───────────────────────────────────────────────────

#[tauri::command]
pub fn update_text_advanced(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    underline: Option<bool>,
    gradient_from_hex: Option<String>,
    gradient_to_hex: Option<String>,
    gradient_angle: Option<f32>,
    curve_amount: Option<f32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    match &mut layer.kind {
        LayerKind::Text3D {
            underline: ul,
            gradient_colors: gc,
            gradient_angle: ga,
            curve_amount: ca,
            ..
        } => {
            if let Some(v) = underline { *ul = v; }
            if let Some(v) = curve_amount { *ca = v.clamp(-180.0, 180.0); }
            if let Some(v) = gradient_angle { *ga = v; }
            // If either gradient color given, build/update gradient_colors pair
            if gradient_from_hex.is_some() || gradient_to_hex.is_some() {
                let cur = gc.unwrap_or([[255,255,255,255],[255,255,0,255]]);
                let from = gradient_from_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[0]);
                let to   = gradient_to_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[1]);
                *gc = Some([from, to]);
            } else if gradient_from_hex.is_none() && gradient_to_hex.is_none()
                && underline.is_none() && curve_amount.is_none() && gradient_angle.is_none()
            {
                // Clear gradient
                *gc = None;
            }
        }
        _ => return Err("Layer is not a Text layer.".into()),
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced shapes ─────────────────────────────────────────────────

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn add_shape_advanced(
    state: tauri::State<'_, SharedProjectState>,
    shape_type: String,
    width: u32,
    height: u32,
    fill_hex: String,
    border_radius: Option<u32>,
    border_hex: Option<String>,
    border_width: Option<u32>,
    sides: Option<u32>,
    fill_gradient_from: Option<String>,
    fill_gradient_to: Option<String>,
    fill_gradient_angle: Option<f32>,
    x: Option<i64>,
    y: Option<i64>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let cw = s.current.canvas_width as i64;
    let ch = s.current.canvas_height as i64;
    let lx = x.unwrap_or((cw - width as i64) / 2);
    let ly = y.unwrap_or((ch - height as i64) / 2);
    let fill_color = parse_hex_color(&fill_hex);
    let b_color = border_hex.as_deref().map(parse_hex_color).unwrap_or([255,255,255,255]);
    let fill_gradient = match (fill_gradient_from.as_deref(), fill_gradient_to.as_deref()) {
        (Some(a), Some(b)) => Some(BgGradient {
            from: hex_to_rgba8(a),
            to: hex_to_rgba8(b),
            angle: fill_gradient_angle.unwrap_or(0.0),
        }),
        _ => None,
    };
    let id = format!("layer_{}", uuid_v4());
    let name = match shape_type.as_str() {
        "line"     => "Line",
        "arrow"    => "Arrow",
        "triangle" => "Triangle",
        "star"     => "Star",
        "polygon"  => "Polygon",
        "circle"   => "Circle",
        "badge"    => "Badge",
        _          => "Rectangle",
    }.to_string();
    let layer = Layer {
        id: id.clone(),
        name,
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
            shape_type: shape_type.clone(),
            width,
            height,
            fill_color,
            border_radius: border_radius.unwrap_or(0),
            border_color: b_color,
            border_width: border_width.unwrap_or(0),
            sides: sides.unwrap_or(5),
            corner_radii: None,
            fill_gradient,
        },
    };
    s.current.layers.push(layer);
    render_and_respond(&mut s)
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_micros();
    format!("{:x}", ms ^ (ms >> 16))
}

#[tauri::command]
pub fn align_layers(
    state: tauri::State<'_, SharedProjectState>,
    layer_ids: Vec<String>,
    alignment: String, // "left" | "center" | "right" | "top" | "middle" | "bottom"
) -> Result<ProjectResponse, String> {
    if layer_ids.is_empty() {
        return Err("No layers selected for alignment.".to_string());
    }
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();

    let cw = s.current.canvas_width as f32;
    let ch = s.current.canvas_height as f32;
    let selected: Vec<(String, f32, f32, f32, f32)> = s
        .current
        .layers
        .iter()
        .filter(|l| layer_ids.contains(&l.id))
        .map(|l| {
            let w = (l.bounds_width.max(1) as f32) * l.scale;
            let h = (l.bounds_height.max(1) as f32) * l.scale;
            (l.id.clone(), l.x as f32, l.y as f32, w, h)
        })
        .collect();

    if selected.is_empty() {
        return Ok(s.to_response());
    }

    let (ref_left, ref_top, ref_right, ref_bottom) = if selected.len() == 1 {
        (0.0_f32, 0.0_f32, cw, ch)
    } else {
        let min_x = selected.iter().map(|i| i.1).fold(f32::MAX, f32::min);
        let min_y = selected.iter().map(|i| i.2).fold(f32::MAX, f32::min);
        let max_r = selected.iter().map(|i| i.1 + i.3).fold(f32::MIN, f32::max);
        let max_b = selected.iter().map(|i| i.2 + i.4).fold(f32::MIN, f32::max);
        (min_x, min_y, max_r, max_b)
    };

    for l in &mut s.current.layers {
        if !layer_ids.contains(&l.id) || l.locked {
            continue;
        }
        let w = (l.bounds_width.max(1) as f32) * l.scale;
        let h = (l.bounds_height.max(1) as f32) * l.scale;
        match alignment.as_str() {
            "left" => l.x = ref_left.round() as i64,
            "center" => l.x = ((ref_left + ref_right) / 2.0 - w / 2.0).round() as i64,
            "right" => l.x = (ref_right - w).round() as i64,
            "top" => l.y = ref_top.round() as i64,
            "middle" => l.y = ((ref_top + ref_bottom) / 2.0 - h / 2.0).round() as i64,
            "bottom" => l.y = (ref_bottom - h).round() as i64,
            _ => {}
        }
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Font management ─────────────────────────────────────────────────

#[tauri::command]
pub fn import_font(source_path: String) -> Result<String, String> {
    let p = std::path::Path::new(&source_path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext != "ttf" && ext != "otf" {
        return Err("Only .ttf and .otf font files are supported.".into());
    }
    let filename = p.file_name().and_then(|n| n.to_str())
        .ok_or("Invalid file name")?
        .to_string();
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    std::fs::create_dir_all(&fonts_dir).map_err(|e| e.to_string())?;
    let dest = fonts_dir.join(&filename);
    std::fs::copy(&source_path, &dest).map_err(|e| format!("Copy failed: {}", e))?;
    Ok(filename)
}

#[tauri::command]
pub fn list_imported_fonts() -> Result<Vec<String>, String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    if !fonts_dir.exists() {
        return Ok(vec![]);
    }
    let mut names = vec![];
    for entry in std::fs::read_dir(&fonts_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let ext = std::path::Path::new(&name)
            .extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext == "ttf" || ext == "otf" {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

#[tauri::command]
pub fn delete_imported_font(filename: String) -> Result<(), String> {
    let fonts_dir = {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        std::path::PathBuf::from(appdata).join("Veltrax Studios").join("fonts")
    };
    let path = fonts_dir.join(&filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Phase 3: Drop shadow / glow / mask / clip ────────────────────────────────

fn hex_to_rgba8(hex: &str) -> [u8; 4] {
    let h = hex.trim_start_matches('#');
    if h.len() == 6 {
        let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(0);
        [r, g, b, 255]
    } else {
        [0, 0, 0, 255]
    }
}

#[tauri::command]
pub fn update_layer_shadow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    color_hex: Option<String>,
    blur: Option<u32>,
    distance: Option<u32>,
    angle: Option<f32>,
    opacity: Option<u8>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.shadow.enabled = enabled;
    if let Some(c) = color_hex { layer.shadow.color = hex_to_rgba8(&c); }
    if let Some(v) = blur { layer.shadow.blur = v; }
    if let Some(v) = distance { layer.shadow.distance = v; }
    if let Some(v) = angle { layer.shadow.angle = v; }
    if let Some(v) = opacity { layer.shadow.opacity = v; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_glow(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    kind: String, // "outer" | "inner"
    enabled: bool,
    color_hex: Option<String>,
    intensity: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    let glow = if kind == "inner" { &mut layer.inner_glow } else { &mut layer.outer_glow };
    glow.enabled = enabled;
    if let Some(c) = color_hex { glow.color = hex_to_rgba8(&c); }
    if let Some(v) = intensity { glow.intensity = v.clamp(1, 60); }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn update_layer_mask(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    mask_shape: Option<String>, // None | "circle" | "rounded" | "star"
    mask_radius: Option<u32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.mask_shape = mask_shape;
    if let Some(r) = mask_radius { layer.mask_radius = r; }
    render_and_respond(&mut s)
}

#[tauri::command]
pub fn set_clip_to_below(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    enabled: bool,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    layer.clip_to_below = enabled;
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced text ───────────────────────────────────────────────────

#[tauri::command]
pub fn update_text_advanced(
    state: tauri::State<'_, SharedProjectState>,
    layer_id: String,
    underline: Option<bool>,
    gradient_from_hex: Option<String>,
    gradient_to_hex: Option<String>,
    gradient_angle: Option<f32>,
    curve_amount: Option<f32>,
    record_undo: Option<bool>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if record_undo.unwrap_or(true) { s.push_undo(); }
    let layer = s.current.layers.iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or_else(|| format!("Layer '{}' not found.", layer_id))?;
    match &mut layer.kind {
        LayerKind::Text3D {
            underline: ul,
            gradient_colors: gc,
            gradient_angle: ga,
            curve_amount: ca,
            ..
        } => {
            if let Some(v) = underline { *ul = v; }
            if let Some(v) = curve_amount { *ca = v.clamp(-180.0, 180.0); }
            if let Some(v) = gradient_angle { *ga = v; }
            // If either gradient color given, build/update gradient_colors pair
            if gradient_from_hex.is_some() || gradient_to_hex.is_some() {
                let cur = gc.unwrap_or([[255,255,255,255],[255,255,0,255]]);
                let from = gradient_from_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[0]);
                let to   = gradient_to_hex.as_deref().map(hex_to_rgba8).unwrap_or(cur[1]);
                *gc = Some([from, to]);
            } else if gradient_from_hex.is_none() && gradient_to_hex.is_none()
                && underline.is_none() && curve_amount.is_none() && gradient_angle.is_none()
            {
                // Clear gradient
                *gc = None;
            }
        }
        _ => return Err("Layer is not a Text layer.".into()),
    }
    render_and_respond(&mut s)
}

// ── Phase 3: Advanced shapes ─────────────────────────────────────────────────

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn add_shape_advanced(
    state: tauri::State<'_, SharedProjectState>,
    shape_type: String,
    width: u32,
    height: u32,
    fill_hex: String,
    border_radius: Option<u32>,
    border_hex: Option<String>,
    border_width: Option<u32>,
    sides: Option<u32>,
    fill_gradient_from: Option<String>,
    fill_gradient_to: Option<String>,
    fill_gradient_angle: Option<f32>,
    x: Option<i64>,
    y: Option<i64>,
) -> Result<ProjectResponse, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();
    let cw = s.current.canvas_width as i64;
    let ch = s.current.canvas_height as i64;
    let lx = x.unwrap_or((cw - width as i64) / 2);
    let ly = y.unwrap_or((ch - height as i64) / 2);
    let fill_color = parse_hex_color(&fill_hex);
    let b_color = border_hex.as_deref().map(parse_hex_color).unwrap_or([255,255,255,255]);
    let fill_gradient = match (fill_gradient_from.as_deref(), fill_gradient_to.as_deref()) {
        (Some(a), Some(b)) => Some(BgGradient {
            from: hex_to_rgba8(a),
            to: hex_to_rgba8(b),
            angle: fill_gradient_angle.unwrap_or(0.0),
        }),
        _ => None,
    };
    let id = format!("layer_{}", uuid_v4());
    let name = match shape_type.as_str() {
        "line"     => "Line",
        "arrow"    => "Arrow",
        "triangle" => "Triangle",
        "star"     => "Star",
        "polygon"  => "Polygon",
        "circle"   => "Circle",
        "badge"    => "Badge",
        _          => "Rectangle",
    }.to_string();
    let layer = Layer {
        id: id.clone(),
        name,
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
            shape_type: shape_type.clone(),
            width,
            height,
            fill_color,
            border_radius: border_radius.unwrap_or(0),
            border_color: b_color,
            border_width: border_width.unwrap_or(0),
            sides: sides.unwrap_or(5),
            corner_radii: None,
            fill_gradient,
        },
    };
    s.current.layers.push(layer);
    render_and_respond(&mut s)
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_micros();
    format!("{:x}", ms ^ (ms >> 16))
}

#[tauri::command]
pub fn distribute_layers(
    state: tauri::State<'_, SharedProjectState>,
    layer_ids: Vec<String>,
    axis: String, // "horizontal" | "vertical"
) -> Result<ProjectResponse, String> {
    if layer_ids.len() < 3 {
        return Err("Select at least 3 layers to distribute evenly.".to_string());
    }
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.push_undo();

    let mut items: Vec<(String, f32, f32, f32, f32)> = s
        .current
        .layers
        .iter()
        .filter(|l| layer_ids.contains(&l.id) && !l.locked)
        .map(|l| {
            let w = (l.bounds_width.max(1) as f32) * l.scale;
            let h = (l.bounds_height.max(1) as f32) * l.scale;
            (l.id.clone(), l.x as f32, l.y as f32, w, h)
        })
        .collect();

    if items.len() < 3 {
        return Ok(s.to_response());
    }

    if axis == "horizontal" {
        items.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        let first_x = items.first().unwrap().1;
        let last_r = items.last().unwrap().1 + items.last().unwrap().3;
        let total_w: f32 = items.iter().map(|i| i.3).sum();
        let gap = (last_r - first_x - total_w) / ((items.len() - 1) as f32);
        let mut cursor = first_x;
        for (id, _, _, w, _) in items {
            if let Some(l) = s.current.layers.iter_mut().find(|l| l.id == id) {
                l.x = cursor.round() as i64;
            }
            cursor += w + gap;
        }
    } else {
        items.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
        let first_y = items.first().unwrap().2;
        let last_b = items.last().unwrap().2 + items.last().unwrap().4;
        let total_h: f32 = items.iter().map(|i| i.4).sum();
        let gap = (last_b - first_y - total_h) / ((items.len() - 1) as f32);
        let mut cursor = first_y;
        for (id, _, _, _, h) in items {
            if let Some(l) = s.current.layers.iter_mut().find(|l| l.id == id) {
                l.y = cursor.round() as i64;
            }
            cursor += h + gap;
        }
    }
    render_and_respond(&mut s)
}
