use ab_glyph::{Font, FontVec, PxScale, ScaleFont};
use image::{imageops::FilterType, DynamicImage, Rgba, RgbaImage};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

// ── Data Structures ─────────────────────────────────────────────────────────

fn default_levels_white() -> u8 {
    255
}
fn default_levels_gamma() -> f32 {
    1.0
}
fn default_lut() -> String {
    "none".to_string()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LayerEffects {
    pub brightness: i32,
    pub contrast: f32,
    pub saturation: f32,
    pub blur: f32,
    pub filter: String, // "original" | "grayscale" | "noir" | "warm" | "cool" | "invert"
    #[serde(default)]
    pub hue_shift: f32, // -180..180
    #[serde(default)]
    pub temperature: f32, // -100..100
    #[serde(default)]
    pub sharpness: f32, // 0..100
    #[serde(default)]
    pub vignette: f32, // 0..100
    #[serde(default)]
    pub levels_black: u8, // 0..250
    #[serde(default = "default_levels_white")]
    pub levels_white: u8, // 5..255
    #[serde(default = "default_levels_gamma")]
    pub levels_gamma: f32, // 0.2..3.0
    #[serde(default = "default_lut")]
    pub lut_preset: String, // "none" | "cinematic" | "cyberpunk" | "vintage" | "moody" | "pastel"
}

impl Default for LayerEffects {
    fn default() -> Self {
        Self {
            brightness: 0,
            contrast: 0.0,
            saturation: 0.0,
            blur: 0.0,
            filter: "original".to_string(),
            hue_shift: 0.0,
            temperature: 0.0,
            sharpness: 0.0,
            vignette: 0.0,
            levels_black: 0,
            levels_white: 255,
            levels_gamma: 1.0,
            lut_preset: "none".to_string(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LayerOutline {
    pub enabled: bool,
    pub color: [u8; 4],
    pub thickness: u32,
}

impl Default for LayerOutline {
    fn default() -> Self {
        Self {
            enabled: false,
            color: [255, 255, 255, 255],
            thickness: 6,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DropShadow {
    pub enabled: bool,
    pub color: [u8; 4],
    pub blur: f32,
    pub distance: f32,
    pub angle: f32,
    pub opacity: f32,
}

impl Default for DropShadow {
    fn default() -> Self {
        Self {
            enabled: false,
            color: [0, 0, 0, 255],
            blur: 8.0,
            distance: 10.0,
            angle: 135.0,
            opacity: 0.65,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GlowEffect {
    pub enabled: bool,
    pub color: [u8; 4],
    pub intensity: f32, // radius/strength 1..30
}

impl Default for GlowEffect {
    fn default() -> Self {
        Self {
            enabled: false,
            color: [56, 189, 248, 255],
            intensity: 10.0,
        }
    }
}

fn default_align() -> String {
    "left".to_string()
}
fn default_line_height() -> f32 {
    1.2
}
fn default_sides() -> u32 {
    5
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum LayerKind {
    Image {
        source_path: String,
        bg_remove_color: Option<[u8; 4]>,
        bg_remove_tolerance: u8,
    },
    Text3D {
        content: String,
        font_style: String,
        color: [u8; 4],
        depth: u32,
        shadow_color: [u8; 4],
        stroke_color: [u8; 4],
        stroke_width: u32,
        size: f32,
        #[serde(default)]
        bold: bool,
        #[serde(default)]
        italic: bool,
        #[serde(default = "default_align")]
        align: String,
        #[serde(default = "default_line_height")]
        line_height: f32,
        #[serde(default)]
        letter_spacing: f32,
        #[serde(default)]
        underline: bool,
        #[serde(default)]
        gradient_colors: Option<[[u8; 4]; 2]>,
        #[serde(default)]
        gradient_angle: f32,
        #[serde(default)]
        curve_amount: f32,
    },
    Shape {
        shape_type: String, // "rectangle" | "circle" | "badge" | "line" | "arrow" | "triangle" | "star" | "polygon"
        width: u32,
        height: u32,
        fill_color: [u8; 4],
        border_radius: u32,
        #[serde(default)]
        border_color: [u8; 4],
        #[serde(default)]
        border_width: u32,
        #[serde(default = "default_sides")]
        sides: u32,
        #[serde(default)]
        corner_radii: Option<[u32; 4]>,
        #[serde(default)]
        fill_gradient: Option<BgGradient>,
    },
    Gradient {
        width: u32,
        height: u32,
        from_color: [u8; 4],
        to_color: [u8; 4],
        angle: f32, // degrees: 0=top→bottom, 90=left→right
    },
    Watermark {
        text: String,
        font_style: String,
        color: [u8; 4],
        size: f32,
        position: String, // "top-left"|"top-right"|"bottom-left"|"bottom-right"|"center"
        opacity: f32,
        canvas_width: u32,
        canvas_height: u32,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Layer {
    pub id: String,
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    pub opacity: f32,
    pub x: i64,
    pub y: i64,
    pub scale: f32,
    pub rotation: f32,
    #[serde(default)]
    pub flip_h: bool,
    #[serde(default)]
    pub flip_v: bool,
    pub blend_mode: String, // "normal" | "multiply" | "screen" | "overlay"
    #[serde(default)]
    pub bounds_width: u32,
    #[serde(default)]
    pub bounds_height: u32,
    #[serde(default)]
    pub group_id: Option<String>,
    pub effects: LayerEffects,
    pub outline: LayerOutline,
    #[serde(default)]
    pub shadow: DropShadow,
    #[serde(default)]
    pub outer_glow: GlowEffect,
    #[serde(default)]
    pub inner_glow: GlowEffect,
    #[serde(default)]
    pub clip_to_below: bool,
    #[serde(default)]
    pub mask_shape: Option<String>, // None | "circle" | "rounded" | "star"
    #[serde(default)]
    pub mask_radius: u32,
    #[serde(default)]
    pub alpha_mask_path: Option<String>,
    pub kind: LayerKind,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LayerGroup {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub parent_group_id: Option<String>,
    pub visible: bool,
    pub locked: bool,
    #[serde(default)]
    pub collapsed: bool,
}

fn default_schema_version() -> u32 {
    1
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectSnapshot {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub project_id: String,
    pub project_name: String,
    pub canvas_width: u32,
    pub canvas_height: u32,
    pub transparent_bg: bool,
    pub bg_color: [u8; 4],
    #[serde(default)]
    pub bg_gradient: Option<BgGradient>,
    pub layers: Vec<Layer>,
    #[serde(default)]
    pub groups: Vec<LayerGroup>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BgGradient {
    pub from_color: [u8; 4],
    pub to_color: [u8; 4],
    pub angle: f32, // degrees
}

impl Default for ProjectSnapshot {
    fn default() -> Self {
        Self {
            schema_version: 2,
            project_id: String::new(),
            project_name: "Untitled Design".to_string(),
            canvas_width: 1280,
            canvas_height: 720,
            transparent_bg: false,
            bg_color: [255, 255, 255, 255],
            bg_gradient: None,
            layers: Vec::new(),
            groups: Vec::new(),
        }
    }
}

pub struct ProjectState {
    pub is_in_editor: bool,
    pub has_project: bool,
    pub current: ProjectSnapshot,
    pub undo_stack: Vec<ProjectSnapshot>,
    pub redo_stack: Vec<ProjectSnapshot>,
    pub preview_path: String,
    pub next_layer_seq: u64,
    pub project_file_path: Option<String>,
    pub dirty: bool,
}

impl Default for ProjectState {
    fn default() -> Self {
        Self {
            is_in_editor: false,
            has_project: false,
            current: ProjectSnapshot::default(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            preview_path: String::new(),
            next_layer_seq: 1,
            project_file_path: None,
            dirty: false,
        }
    }
}

pub type SharedProjectState = Arc<Mutex<ProjectState>>;

#[derive(Clone, Serialize)]
pub struct ProjectResponse {
    pub preview_path: String,
    pub project_name: String,
    pub project_file_path: Option<String>,
    pub dirty: bool,
    pub canvas_width: u32,
    pub canvas_height: u32,
    pub transparent_bg: bool,
    pub bg_color: [u8; 4],
    pub bg_gradient: Option<BgGradient>,
    pub layers: Vec<Layer>,
    pub groups: Vec<LayerGroup>,
    pub can_undo: bool,
    pub can_redo: bool,
    pub undo_count: usize,
}

impl ProjectState {
    pub fn next_id(&mut self) -> String {
        let id = format!("layer-{}", self.next_layer_seq);
        self.next_layer_seq += 1;
        id
    }

    pub fn push_undo(&mut self) {
        self.dirty = true;
        self.undo_stack.push(self.current.clone());
        if self.undo_stack.len() > 40 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    pub fn to_response(&self) -> ProjectResponse {
        ProjectResponse {
            preview_path: self.preview_path.clone(),
            project_name: self.current.project_name.clone(),
            project_file_path: self.project_file_path.clone(),
            dirty: self.dirty,
            canvas_width: self.current.canvas_width,
            canvas_height: self.current.canvas_height,
            transparent_bg: self.current.transparent_bg,
            bg_color: self.current.bg_color,
            bg_gradient: self.current.bg_gradient.clone(),
            layers: self.current.layers.clone(),
            groups: self.current.groups.clone(),
            can_undo: !self.undo_stack.is_empty(),
            can_redo: !self.redo_stack.is_empty(),
            undo_count: self.undo_stack.len(),
        }
    }
}

// ── Color & Font Helpers ────────────────────────────────────────────────────

pub fn parse_hex_color(hex: &str) -> [u8; 4] {
    let h = hex.trim().trim_start_matches('#');
    if h.len() >= 6 {
        let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(255);
        let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(255);
        let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(255);
        let a = if h.len() >= 8 {
            u8::from_str_radix(&h[6..8], 16).unwrap_or(255)
        } else {
            255
        };
        [r, g, b, a]
    } else {
        [255, 255, 255, 255]
    }
}

fn load_font_bytes(style: &str) -> Result<Vec<u8>, String> {
    // 1. Check imported custom fonts in %APPDATA%\Veltrax Studios\fonts\
    if let Ok(appdata) = std::env::var("APPDATA") {
        let custom_dir = std::path::Path::new(&appdata).join("Veltrax Studios").join("fonts");
        let custom_path = custom_dir.join(style);
        if custom_path.is_file() {
            if let Ok(data) = std::fs::read(&custom_path) {
                return Ok(data);
            }
        }
    }
    // 2. Check direct system font filename in C:\Windows\Fonts\
    if style.to_lowercase().ends_with(".ttf") || style.to_lowercase().ends_with(".otf") {
        let win_font = std::path::Path::new(r"C:\Windows\Fonts").join(style);
        if win_font.is_file() {
            if let Ok(data) = std::fs::read(&win_font) {
                return Ok(data);
            }
        }
    }
    // 3. Built-in style presets
    let candidates: &[&str] = match style {
        "classic_serif" => &[
            r"C:\Windows\Fonts\times.ttf",
            r"C:\Windows\Fonts\timesbd.ttf",
            r"C:\Windows\Fonts\arial.ttf",
        ],
        "tech_mono" => &[
            r"C:\Windows\Fonts\consola.ttf",
            r"C:\Windows\Fonts\cour.ttf",
            r"C:\Windows\Fonts\lucon.ttf",
            r"C:\Windows\Fonts\arial.ttf",
        ],
        "bold_display" => &[
            r"C:\Windows\Fonts\impact.ttf",
            r"C:\Windows\Fonts\arialbd.ttf",
            r"C:\Windows\Fonts\calibrib.ttf",
            r"C:\Windows\Fonts\arial.ttf",
        ],
        "script" => &[
            r"C:\Windows\Fonts\segoesc.ttf",
            r"C:\Windows\Fonts\comic.ttf",
            r"C:\Windows\Fonts\arial.ttf",
        ],
        _ => &[
            r"C:\Windows\Fonts\arialbd.ttf",
            r"C:\Windows\Fonts\arial.ttf",
            r"C:\Windows\Fonts\segoeui.ttf",
        ],
    };
    for path in candidates {
        if let Ok(data) = std::fs::read(path) {
            return Ok(data);
        }
    }
    Err(format!(
        "No system font found for style '{}' in C:\\Windows\\Fonts.",
        style
    ))
}

// ── Layer Rendering & Compositing ───────────────────────────────────────────

fn is_group_visible(group_id: Option<&str>, groups: &[LayerGroup]) -> bool {
    let mut cur = group_id;
    let mut guard = 0;
    while let Some(gid) = cur {
        guard += 1;
        if guard > 32 {
            break;
        }
        if let Some(g) = groups.iter().find(|gr| gr.id == gid) {
            if !g.visible {
                return false;
            }
            cur = g.parent_group_id.as_deref();
        } else {
            break;
        }
    }
    true
}

pub fn render_project_composite(
    snapshot: &mut ProjectSnapshot,
    dest_path: &str,
) -> Result<DynamicImage, String> {
    let cw = snapshot.canvas_width.max(1);
    let ch = snapshot.canvas_height.max(1);

    // Build background canvas
    let mut canvas = if snapshot.transparent_bg {
        RgbaImage::from_pixel(cw, ch, Rgba([0, 0, 0, 0]))
    } else if let Some(ref grad) = snapshot.bg_gradient {
        render_gradient_image(cw, ch, grad.from_color, grad.to_color, grad.angle)
    } else {
        RgbaImage::from_pixel(cw, ch, Rgba(snapshot.bg_color))
    };

    let groups_ref = snapshot.groups.clone();
    let mut prev_canvas_alpha: Option<Vec<u8>> = None;

    for layer in &mut snapshot.layers {
        let Ok(mut layer_img) = render_layer_kind(&layer.kind) else {
            continue;
        };
        layer.bounds_width = layer_img.width();
        layer.bounds_height = layer_img.height();

        if !layer.visible || !is_group_visible(layer.group_id.as_deref(), &groups_ref) || layer.opacity <= 0.001 {
            continue;
        }

        if let Some(ref mask_path) = layer.alpha_mask_path {
            layer_img = apply_alpha_mask_file(&layer_img, mask_path);
        }

        if let Some(ref mask_kind) = layer.mask_shape {
            layer_img = apply_shape_mask(&layer_img, mask_kind, layer.mask_radius);
        }

        layer_img = apply_layer_effects(layer_img, &layer.effects);

        if (layer.scale - 1.0).abs() > 0.01 {
            let nw = ((layer_img.width() as f32 * layer.scale).round() as u32).max(1);
            let nh = ((layer_img.height() as f32 * layer.scale).round() as u32).max(1);
            layer_img = DynamicImage::ImageRgba8(layer_img)
                .resize_exact(nw, nh, FilterType::Lanczos3)
                .to_rgba8();
        }

        if layer.rotation.abs() > 0.1 {
            layer_img = rotate_rgba_bilinear(&layer_img, layer.rotation);
        }

        if layer.flip_h {
            layer_img = image::imageops::flip_horizontal(&layer_img);
        }
        if layer.flip_v {
            layer_img = image::imageops::flip_vertical(&layer_img);
        }

        if layer.inner_glow.enabled && layer.inner_glow.intensity > 0.5 {
            layer_img = apply_inner_glow(&layer_img, layer.inner_glow.color, layer.inner_glow.intensity);
        }

        let mut draw_x = layer.x;
        let mut draw_y = layer.y;

        if layer.outline.enabled && layer.outline.thickness > 0 {
            let t = layer.outline.thickness.min(24) as i64;
            layer_img = apply_outline(&layer_img, layer.outline.color, layer.outline.thickness);
            draw_x -= t;
            draw_y -= t;
        }

        if layer.outer_glow.enabled && layer.outer_glow.intensity > 0.5 {
            let pad = (layer.outer_glow.intensity.ceil() as i64).min(40);
            layer_img = apply_outer_glow(&layer_img, layer.outer_glow.color, layer.outer_glow.intensity);
            draw_x -= pad;
            draw_y -= pad;
        }

        if layer.shadow.enabled && layer.shadow.opacity > 0.01 {
            let (sh_img, off_x, off_y) = apply_drop_shadow(&layer_img, &layer.shadow);
            layer_img = sh_img;
            draw_x -= off_x;
            draw_y -= off_y;
        }

        if layer.clip_to_below {
            if let Some(ref mask_alpha) = prev_canvas_alpha {
                composite_layer_clipped(
                    &mut canvas,
                    &layer_img,
                    draw_x,
                    draw_y,
                    layer.opacity.clamp(0.0, 1.0),
                    &layer.blend_mode,
                    mask_alpha,
                );
                continue;
            }
        }

        // Record this layer's canvas-space alpha for potential clipping mask on the layer above
        let mut alpha_map = vec![0u8; (cw * ch) as usize];
        for ly in 0..layer_img.height() as i64 {
            let cy = draw_y + ly;
            if cy < 0 || cy >= ch as i64 {
                continue;
            }
            for lx in 0..layer_img.width() as i64 {
                let cx = draw_x + lx;
                if cx < 0 || cx >= cw as i64 {
                    continue;
                }
                alpha_map[(cy as u32 * cw + cx as u32) as usize] =
                    layer_img.get_pixel(lx as u32, ly as u32)[3];
            }
        }
        prev_canvas_alpha = Some(alpha_map);

        composite_layer(
            &mut canvas,
            &layer_img,
            draw_x,
            draw_y,
            layer.opacity.clamp(0.0, 1.0),
            &layer.blend_mode,
        );
    }

    let dyn_img = DynamicImage::ImageRgba8(canvas);
    if !dest_path.is_empty() {
        if let Some(parent) = std::path::Path::new(dest_path).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
        }
        dyn_img
            .save(dest_path)
            .map_err(|e| format!("Failed to save composite preview: {}", e))?;
    }
    Ok(dyn_img)
}

pub fn render_layer_kind(kind: &LayerKind) -> Result<RgbaImage, String> {
    match kind {
        LayerKind::Image {
            source_path,
            bg_remove_color,
            bg_remove_tolerance,
        } => {
            let Ok(img) = image::open(source_path) else {
                // Missing asset placeholder so projects never fail to open
                let (pw, ph) = (240u32, 160u32);
                let mut ph_img = RgbaImage::from_pixel(pw, ph, Rgba([30, 41, 59, 230]));
                for x in 0..pw {
                    for b in 0..3 {
                        ph_img.put_pixel(x, b, Rgba([239, 68, 68, 255]));
                        ph_img.put_pixel(x, ph - 1 - b, Rgba([239, 68, 68, 255]));
                    }
                }
                for y in 0..ph {
                    for b in 0..3 {
                        ph_img.put_pixel(b, y, Rgba([239, 68, 68, 255]));
                        ph_img.put_pixel(pw - 1 - b, y, Rgba([239, 68, 68, 255]));
                    }
                    let d1 = ((y as f32 / ph as f32) * pw as f32) as u32;
                    let d2 = pw.saturating_sub(1 + d1);
                    if d1 < pw { ph_img.put_pixel(d1, y, Rgba([239, 68, 68, 200])); }
                    if d2 < pw { ph_img.put_pixel(d2, y, Rgba([239, 68, 68, 200])); }
                }
                return Ok(ph_img);
            };
            let mut rgba = img.to_rgba8();
            if let Some(target) = bg_remove_color {
                let tol = *bg_remove_tolerance as i32;
                for p in rgba.pixels_mut() {
                    let dr = (p[0] as i32 - target[0] as i32).abs();
                    let dg = (p[1] as i32 - target[1] as i32).abs();
                    let db = (p[2] as i32 - target[2] as i32).abs();
                    if dr <= tol && dg <= tol && db <= tol {
                        p[3] = 0;
                    }
                }
            }
            Ok(rgba)
        }
        LayerKind::Text3D {
            content,
            font_style,
            color,
            depth,
            shadow_color,
            stroke_color,
            stroke_width,
            size,
            bold,
            italic,
            align,
            line_height,
            letter_spacing,
            underline,
            gradient_colors,
            gradient_angle,
            curve_amount,
        } => render_text_3d(
            content,
            font_style,
            *color,
            *depth,
            *shadow_color,
            *stroke_color,
            *stroke_width,
            *size,
            *bold,
            *italic,
            align,
            *line_height,
            *letter_spacing,
            *underline,
            *gradient_colors,
            *gradient_angle,
            *curve_amount,
        ),
        LayerKind::Shape {
            shape_type,
            width,
            height,
            fill_color,
            border_radius,
            border_color,
            border_width,
            sides,
            corner_radii,
            fill_gradient,
        } => Ok(render_shape(
            shape_type,
            (*width).max(1),
            (*height).max(1),
            *fill_color,
            *border_radius,
            *border_color,
            *border_width,
            *sides,
            *corner_radii,
            fill_gradient.as_ref(),
        )),
        LayerKind::Gradient {
            width,
            height,
            from_color,
            to_color,
            angle,
        } => Ok(render_gradient_image(
            (*width).max(1),
            (*height).max(1),
            *from_color,
            *to_color,
            *angle,
        )),
        LayerKind::Watermark {
            text,
            font_style,
            color,
            size,
            position,
            opacity: _,
            canvas_width,
            canvas_height,
        } => render_watermark_layer(
            text,
            font_style,
            *color,
            *size,
            position,
            *canvas_width,
            *canvas_height,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn render_text_3d(
    content: &str,
    font_style: &str,
    color: [u8; 4],
    depth: u32,
    shadow_color: [u8; 4],
    stroke_color: [u8; 4],
    stroke_width: u32,
    size: f32,
    bold: bool,
    italic: bool,
    align: &str,
    line_height: f32,
    letter_spacing: f32,
    underline: bool,
    gradient_colors: Option<[[u8; 4]; 2]>,
    gradient_angle: f32,
    curve_amount: f32,
) -> Result<RgbaImage, String> {
    let font_bytes = load_font_bytes(font_style)?;
    let font = FontVec::try_from_vec(font_bytes).map_err(|_| "Failed to parse font".to_string())?;
    let scale = PxScale::from(size.max(8.0));
    let scaled = font.as_scaled(scale);

    let lines: Vec<&str> = if content.is_empty() {
        vec![""]
    } else {
        content.split('\n').collect()
    };

    let measure_line = |line: &str| -> f32 {
        let mut w: f32 = 0.0;
        let mut prev = None;
        let mut count = 0;
        for ch in line.chars() {
            let gid = scaled.glyph_id(ch);
            if let Some(p) = prev {
                w += scaled.kern(p, gid) + letter_spacing;
            }
            prev = Some(gid);
            w += scaled.h_advance(gid);
            count += 1;
        }
        if bold && count > 0 {
            w += (size * 0.04).max(1.0);
        }
        w.max(0.0)
    };

    let line_widths: Vec<f32> = lines.iter().map(|l| measure_line(l)).collect();
    let max_line_w = line_widths.iter().copied().fold(0.0_f32, f32::max);
    let italic_pad = if italic { (size * 0.25).ceil() as i32 } else { 0 };
    let curve_pad = if curve_amount.abs() > 0.5 {
        (curve_amount.abs() * 1.4).ceil() as i32
    } else {
        0
    };
    let pad = (depth + stroke_width + 14) as i32 + italic_pad + curve_pad;
    let raw_line_h = (scaled.ascent() - scaled.descent()).ceil();
    let step_y = (raw_line_h * line_height.clamp(0.7, 3.0)).max(8.0);
    let total_h = raw_line_h + step_y * (lines.len().saturating_sub(1) as f32);

    let img_w = (max_line_w.ceil() as i32 + pad * 2).max(16) as u32;
    let img_h = (total_h.ceil() as i32 + pad * 2).max(16) as u32;
    let mut img = RgbaImage::from_pixel(img_w, img_h, Rgba([0, 0, 0, 0]));

    let base_x = pad as f32;
    let base_y = pad as f32 + scaled.ascent();

    let draw_multiline = |target: &mut RgbaImage, ox: f32, oy: f32, col: [u8; 4]| {
        for (idx, line) in lines.iter().enumerate() {
            let lw = line_widths[idx];
            let align_off = match align {
                "center" => (max_line_w - lw) / 2.0,
                "right" => max_line_w - lw,
                _ => 0.0,
            };
            let lx = base_x + ox + align_off;
            let ly = base_y + oy + (idx as f32) * step_y;
            draw_text_pass(
                target,
                &font,
                scale,
                line,
                lx,
                ly,
                col,
                bold,
                italic,
                letter_spacing,
                underline,
                curve_amount,
                lw,
            );
        }
    };

    // 1. Drop shadow
    if depth > 0 {
        let sh_off = (depth + 3) as f32;
        draw_multiline(&mut img, sh_off, sh_off, shadow_color);
    }

    // 2. 3D Extrusion layers
    if depth > 0 {
        let extrude_color = [
            (color[0] as f32 * 0.38) as u8,
            (color[1] as f32 * 0.38) as u8,
            (color[2] as f32 * 0.38) as u8,
            color[3],
        ];
        for d in (1..=depth).rev() {
            let df = d as f32;
            draw_multiline(&mut img, df, df, extrude_color);
        }
    }

    // 3. Stroke outline
    if stroke_width > 0 {
        let sw = stroke_width.min(10) as i32;
        for dy in -sw..=sw {
            for dx in -sw..=sw {
                if dx * dx + dy * dy <= sw * sw && (dx != 0 || dy != 0) {
                    draw_multiline(&mut img, dx as f32, dy as f32, stroke_color);
                }
            }
        }
    }

    // 4. Main face (with optional gradient fill)
    if let Some([from_c, to_c]) = gradient_colors {
        let mut face = RgbaImage::from_pixel(img_w, img_h, Rgba([0, 0, 0, 0]));
        draw_multiline(&mut face, 0.0, 0.0, [255, 255, 255, 255]);
        let grad = render_gradient_image(img_w, img_h, from_c, to_c, gradient_angle);
        for y in 0..img_h {
            for x in 0..img_w {
                let fa = face.get_pixel(x, y)[3];
                if fa > 0 {
                    let gp = grad.get_pixel(x, y);
                    let dst = img.get_pixel_mut(x, y);
                    let out_a = ((fa as f32 / 255.0) * (gp[3] as f32 / 255.0) * 255.0) as u8;
                    blend_pixel_over(dst, [gp[0], gp[1], gp[2], out_a]);
                }
            }
        }
    } else {
        draw_multiline(&mut img, 0.0, 0.0, color);
    }
    Ok(img)
}

#[allow(clippy::too_many_arguments)]
fn draw_text_pass(
    img: &mut RgbaImage,
    font: &FontVec,
    scale: PxScale,
    text: &str,
    start_x: f32,
    start_y: f32,
    color: [u8; 4],
    bold: bool,
    italic: bool,
    letter_spacing: f32,
    underline: bool,
    curve_amount: f32,
    line_width: f32,
) {
    let scaled = font.as_scaled(scale);
    let (w, h) = (img.width() as i32, img.height() as i32);
    let mut pen_x = start_x;
    let mut prev = None;
    let bold_offset = if bold { (scale.x * 0.035).max(1.0).round() as i32 } else { 0 };
    let line_center = start_x + line_width / 2.0;

    for ch in text.chars() {
        let gid = scaled.glyph_id(ch);
        if let Some(p) = prev {
            pen_x += scaled.kern(p, gid) + letter_spacing;
        }
        prev = Some(gid);

        let arc_dy = if curve_amount.abs() > 0.5 && line_width > 1.0 {
            let norm = ((pen_x + scaled.h_advance(gid) / 2.0 - line_center) / (line_width / 2.0)).clamp(-1.0, 1.0);
            curve_amount * (norm * norm - 0.5)
        } else {
            0.0
        };

        let glyph = gid.with_scale_and_position(scale, ab_glyph::point(pen_x, start_y + arc_dy));
        if let Some(og) = font.outline_glyph(glyph) {
            let bb = og.px_bounds();
            og.draw(|rx, ry, v| {
                let py = bb.min.y as i32 + ry as i32;
                let shear = if italic { ((start_y + arc_dy - py as f32) * 0.22).round() as i32 } else { 0 };
                let px = bb.min.x as i32 + rx as i32 + shear;
                let src_a = (v * (color[3] as f32 / 255.0)).clamp(0.0, 1.0);
                let rgba = [color[0], color[1], color[2], (src_a * 255.0) as u8];
                if px >= 0 && py >= 0 && px < w && py < h {
                    let dst = img.get_pixel_mut(px as u32, py as u32);
                    blend_pixel_over(dst, rgba);
                }
                if bold_offset > 0 {
                    let bpx = px + bold_offset;
                    if bpx >= 0 && py >= 0 && bpx < w && py < h {
                        let dst = img.get_pixel_mut(bpx as u32, py as u32);
                        blend_pixel_over(dst, rgba);
                    }
                }
            });
        }
        pen_x += scaled.h_advance(gid);
    }

    if underline && line_width > 1.0 {
        let ul_y = (start_y + (scale.y * 0.12).max(2.0)).round() as i32;
        let ul_h = (scale.y * 0.06).max(2.0).round() as i32;
        let x0 = start_x.round() as i32;
        let x1 = (start_x + line_width).round() as i32;
        for py in ul_y..(ul_y + ul_h) {
            for px in x0..x1 {
                if px >= 0 && py >= 0 && px < w && py < h {
                    let dst = img.get_pixel_mut(px as u32, py as u32);
                    blend_pixel_over(dst, color);
                }
            }
        }
    }
}

pub fn point_in_polygon(px: f32, py: f32, verts: &[(f32, f32)]) -> bool {
    let mut inside = false;
    let n = verts.len();
    let mut j = n.saturating_sub(1);
    for i in 0..n {
        let (xi, yi) = verts[i];
        let (xj, yj) = verts[j];
        if ((yi > py) != (yj > py)) && (px < (xj - xi) * (py - yi) / ((yj - yi) + 1e-6) + xi) {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn shape_hit(
    shape_type: &str,
    px: f32,
    py: f32,
    w: f32,
    h: f32,
    border_radius: u32,
    sides: u32,
    corner_radii: Option<[u32; 4]>,
) -> bool {
    match shape_type {
        "circle" => {
            let cx = w / 2.0;
            let cy = h / 2.0;
            let dx = (px - cx) / (w / 2.0).max(1.0);
            let dy = (py - cy) / (h / 2.0).max(1.0);
            dx * dx + dy * dy <= 1.0
        }
        "line" => {
            let cy = h / 2.0;
            (py - cy).abs() <= (h * 0.18).max(3.0)
        }
        "arrow" => {
            let shaft_end = w * 0.68;
            let mid_y = h / 2.0;
            if px <= shaft_end {
                (py - mid_y).abs() <= h * 0.18
            } else {
                let t = (px - shaft_end) / (w - shaft_end).max(1.0);
                (py - mid_y).abs() <= (h * 0.48) * (1.0 - t)
            }
        }
        "triangle" => {
            let verts = [(w / 2.0, 1.0), (w - 1.0, h - 1.0), (1.0, h - 1.0)];
            point_in_polygon(px, py, &verts)
        }
        "star" => {
            let cx = w / 2.0;
            let cy = h / 2.0;
            let rx_out = w / 2.0 - 1.0;
            let ry_out = h / 2.0 - 1.0;
            let rx_in = rx_out * 0.42;
            let ry_in = ry_out * 0.42;
            let pts = sides.max(4).min(12) as usize * 2;
            let mut verts = Vec::with_capacity(pts);
            for i in 0..pts {
                let ang = (i as f32) * std::f32::consts::PI / (pts as f32 / 2.0) - std::f32::consts::FRAC_PI_2;
                let (rx, ry) = if i % 2 == 0 { (rx_out, ry_out) } else { (rx_in, ry_in) };
                verts.push((cx + rx * ang.cos(), cy + ry * ang.sin()));
            }
            point_in_polygon(px, py, &verts)
        }
        "polygon" => {
            let cx = w / 2.0;
            let cy = h / 2.0;
            let rx = w / 2.0 - 1.0;
            let ry = h / 2.0 - 1.0;
            let n = sides.max(3).min(16) as usize;
            let mut verts = Vec::with_capacity(n);
            for i in 0..n {
                let ang = (i as f32) * 2.0 * std::f32::consts::PI / (n as f32) - std::f32::consts::FRAC_PI_2;
                verts.push((cx + rx * ang.cos(), cy + ry * ang.sin()));
            }
            point_in_polygon(px, py, &verts)
        }
        "badge" => rounded_rect_hit(px, py, w, h, h / 2.0),
        _ => {
            if let Some([tl, tr, br, bl]) = corner_radii {
                let max_r = w.min(h) / 2.0;
                let r = if px < w / 2.0 && py < h / 2.0 {
                    (tl as f32).min(max_r)
                } else if px >= w / 2.0 && py < h / 2.0 {
                    (tr as f32).min(max_r)
                } else if px >= w / 2.0 && py >= h / 2.0 {
                    (br as f32).min(max_r)
                } else {
                    (bl as f32).min(max_r)
                };
                if r > 0.5 {
                    rounded_rect_hit(px, py, w, h, r)
                } else {
                    true
                }
            } else if border_radius > 0 {
                rounded_rect_hit(px, py, w, h, (border_radius as f32).min(w.min(h) / 2.0))
            } else {
                true
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_shape(
    shape_type: &str,
    width: u32,
    height: u32,
    fill_color: [u8; 4],
    border_radius: u32,
    border_color: [u8; 4],
    border_width: u32,
    sides: u32,
    corner_radii: Option<[u32; 4]>,
    fill_gradient: Option<&BgGradient>,
) -> RgbaImage {
    let mut img = RgbaImage::from_pixel(width, height, Rgba([0, 0, 0, 0]));
    let grad_img = fill_gradient.map(|g| render_gradient_image(width, height, g.from_color, g.to_color, g.angle));
    let (wf, hf) = (width as f32, height as f32);

    let mut mask = vec![false; (width * height) as usize];
    for y in 0..height {
        for x in 0..width {
            mask[(y * width + x) as usize] = shape_hit(
                shape_type,
                x as f32 + 0.5,
                y as f32 + 0.5,
                wf,
                hf,
                border_radius,
                sides,
                corner_radii,
            );
        }
    }

    let bw = border_width.min(32) as i32;
    for y in 0..height as i32 {
        for x in 0..width as i32 {
            if !mask[(y as u32 * width + x as u32) as usize] {
                continue;
            }
            let mut on_border = false;
            if bw > 0 && border_color[3] > 0 {
                'edge: for oy in -bw..=bw {
                    for ox in -bw..=bw {
                        if ox * ox + oy * oy <= bw * bw {
                            let nx = x + ox;
                            let ny = y + oy;
                            if nx < 0 || ny < 0 || nx >= width as i32 || ny >= height as i32 || !mask[(ny as u32 * width + nx as u32) as usize] {
                                on_border = true;
                                break 'edge;
                            }
                        }
                    }
                }
            }
            let px_col = if on_border {
                Rgba(border_color)
            } else if let Some(ref gi) = grad_img {
                *gi.get_pixel(x as u32, y as u32)
            } else {
                Rgba(fill_color)
            };
            img.put_pixel(x as u32, y as u32, px_col);
        }
    }
    img
}

fn render_gradient_image(
    width: u32,
    height: u32,
    from_color: [u8; 4],
    to_color: [u8; 4],
    angle: f32,
) -> RgbaImage {
    let mut img = RgbaImage::new(width, height);
    let rad = angle.to_radians();
    let cos_a = rad.cos();
    let sin_a = rad.sin();
    let (cw, ch) = (width as f32, height as f32);
    // Max projection so gradient covers full canvas regardless of angle
    let max_proj = (cw * cos_a.abs() + ch * sin_a.abs()).max(1.0);

    for y in 0..height {
        for x in 0..width {
            // Project pixel onto gradient direction
            let px = x as f32 - cw / 2.0;
            let py = y as f32 - ch / 2.0;
            let proj = px * sin_a + py * cos_a; // along angle direction
            let t = ((proj + max_proj / 2.0) / max_proj).clamp(0.0, 1.0);

            let r = (from_color[0] as f32 + t * (to_color[0] as f32 - from_color[0] as f32)) as u8;
            let g = (from_color[1] as f32 + t * (to_color[1] as f32 - from_color[1] as f32)) as u8;
            let b = (from_color[2] as f32 + t * (to_color[2] as f32 - from_color[2] as f32)) as u8;
            let a = (from_color[3] as f32 + t * (to_color[3] as f32 - from_color[3] as f32)) as u8;
            img.put_pixel(x, y, Rgba([r, g, b, a]));
        }
    }
    img
}

fn render_watermark_layer(
    text: &str,
    font_style: &str,
    color: [u8; 4],
    size: f32,
    position: &str,
    canvas_width: u32,
    canvas_height: u32,
) -> Result<RgbaImage, String> {
    // Render the text, then place it on a transparent canvas at the right corner
    let text_img = render_text_3d(
        text,
        font_style,
        color,
        0,
        [0, 0, 0, 0],
        [0, 0, 0, 0],
        0,
        size,
        false,
        false,
        "left",
        1.2,
        0.0,
        false,
        None,
        0.0,
        0.0,
    )?;
    let mut canvas = RgbaImage::from_pixel(canvas_width, canvas_height, Rgba([0, 0, 0, 0]));
    let tw = text_img.width();
    let th = text_img.height();
    let margin = 20i64;
    let (px, py): (i64, i64) = match position {
        "top-left"     => (margin, margin),
        "top-right"    => ((canvas_width as i64 - tw as i64 - margin).max(0), margin),
        "bottom-left"  => (margin, (canvas_height as i64 - th as i64 - margin).max(0)),
        "center"       => ((canvas_width as i64 - tw as i64) / 2, (canvas_height as i64 - th as i64) / 2),
        _              => ((canvas_width as i64 - tw as i64 - margin).max(0), (canvas_height as i64 - th as i64 - margin).max(0)), // bottom-right
    };
    composite_layer(&mut canvas, &text_img, px, py, 1.0, "normal");
    Ok(canvas)
}

fn rounded_rect_hit(x: f32, y: f32, w: f32, h: f32, r: f32) -> bool {
    let cx = if x < r { r } else if x > w - r { w - r } else { x };
    let cy = if y < r { r } else if y > h - r { h - r } else { y };
    let dx = x - cx;
    let dy = y - cy;
    dx * dx + dy * dy <= r * r
}

fn apply_layer_effects(rgba: RgbaImage, fx: &LayerEffects) -> RgbaImage {
    let mut dyn_img = DynamicImage::ImageRgba8(rgba);

    if fx.brightness != 0 {
        dyn_img = dyn_img.brighten(fx.brightness);
    }
    if fx.contrast.abs() > f32::EPSILON {
        dyn_img = dyn_img.adjust_contrast(fx.contrast);
    }
    if fx.saturation.abs() > f32::EPSILON {
        let factor = 1.0 + fx.saturation / 100.0;
        let mut buf = dyn_img.to_rgba8();
        for p in buf.pixels_mut() {
            let r = p[0] as f32 / 255.0;
            let g = p[1] as f32 / 255.0;
            let b = p[2] as f32 / 255.0;
            let gray = 0.299 * r + 0.587 * g + 0.114 * b;
            p[0] = ((gray + factor * (r - gray)).clamp(0.0, 1.0) * 255.0) as u8;
            p[1] = ((gray + factor * (g - gray)).clamp(0.0, 1.0) * 255.0) as u8;
            p[2] = ((gray + factor * (b - gray)).clamp(0.0, 1.0) * 255.0) as u8;
        }
        dyn_img = DynamicImage::ImageRgba8(buf);
    }
    if fx.blur > 0.1 {
        dyn_img = dyn_img.blur(fx.blur);
    }

    match fx.filter.as_str() {
        "grayscale" => dyn_img = dyn_img.grayscale(),
        "noir" => dyn_img = dyn_img.grayscale().adjust_contrast(45.0),
        "invert" => dyn_img.invert(),
        "warm" => {
            let mut buf = dyn_img.to_rgba8();
            for p in buf.pixels_mut() {
                p[0] = (p[0] as u16 + 25).min(255) as u8;
                p[1] = (p[1] as u16 + 8).min(255) as u8;
                p[2] = p[2].saturating_sub(20);
            }
            dyn_img = DynamicImage::ImageRgba8(buf);
        }
        "cool" => {
            let mut buf = dyn_img.to_rgba8();
            for p in buf.pixels_mut() {
                p[0] = p[0].saturating_sub(20);
                p[1] = (p[1] as u16 + 5).min(255) as u8;
                p[2] = (p[2] as u16 + 25).min(255) as u8;
            }
            dyn_img = DynamicImage::ImageRgba8(buf);
        }
        _ => {}
    }

    // Sharpness (unsharp mask)
    if fx.sharpness > 0.5 {
        dyn_img = dyn_img.unsharpen((fx.sharpness / 22.0).clamp(0.2, 5.0), 4);
    }

    let mut out = dyn_img.to_rgba8();

    // Levels / Gamma LUT
    let has_levels = fx.levels_black > 0 || fx.levels_white < 255 || (fx.levels_gamma - 1.0).abs() > 0.02;
    let levels_lut: Option<[u8; 256]> = if has_levels {
        let b = fx.levels_black.min(250) as f32;
        let w = fx.levels_white.max(fx.levels_black.saturating_add(5)) as f32;
        let inv_gamma = 1.0 / fx.levels_gamma.clamp(0.2, 3.0);
        let mut lut = [0u8; 256];
        for i in 0..256 {
            let norm = ((i as f32 - b) / (w - b)).clamp(0.0, 1.0);
            lut[i] = (norm.powf(inv_gamma) * 255.0).round().clamp(0.0, 255.0) as u8;
        }
        Some(lut)
    } else {
        None
    };

    let has_temp = fx.temperature.abs() > 0.5;
    let has_hue = fx.hue_shift.abs() > 0.5;
    let has_lut = fx.lut_preset != "none" && !fx.lut_preset.is_empty();
    let has_vig = fx.vignette > 0.5;

    if levels_lut.is_some() || has_temp || has_hue || has_lut || has_vig {
        let (pw, ph) = (out.width() as f32, out.height() as f32);
        let cx = pw * 0.5;
        let cy = ph * 0.5;
        let max_r = (cx * cx + cy * cy).sqrt().max(1.0);
        let vig_str = (fx.vignette / 100.0).clamp(0.0, 1.0);

        for (px, py, p) in out.enumerate_pixels_mut() {
            if p[3] == 0 {
                continue;
            }
            if let Some(ref lut) = levels_lut {
                p[0] = lut[p[0] as usize];
                p[1] = lut[p[1] as usize];
                p[2] = lut[p[2] as usize];
            }
            let mut r = p[0] as f32;
            let mut g = p[1] as f32;
            let mut b = p[2] as f32;

            if has_temp {
                let t = fx.temperature * 0.45;
                r = (r + t).clamp(0.0, 255.0);
                b = (b - t).clamp(0.0, 255.0);
            }

            if has_hue {
                let (h, s, l) = rgb_to_hsl(r / 255.0, g / 255.0, b / 255.0);
                let nh = (h + fx.hue_shift / 360.0).rem_euclid(1.0);
                let (nr, ng, nb) = hsl_to_rgb(nh, s, l);
                r = nr * 255.0;
                g = ng * 255.0;
                b = nb * 255.0;
            }

            if has_lut {
                let lum = (0.299 * r + 0.587 * g + 0.114 * b) / 255.0;
                match fx.lut_preset.as_str() {
                    "cinematic" => {
                        // Teal shadows, warm highlights
                        r = (r + (lum - 0.35) * 28.0).clamp(0.0, 255.0);
                        b = (b + (0.55 - lum) * 30.0).clamp(0.0, 255.0);
                    }
                    "cyberpunk" => {
                        r = (r * 1.08 + 12.0).clamp(0.0, 255.0);
                        g = (g * 0.88).clamp(0.0, 255.0);
                        b = (b * 1.15 + 18.0).clamp(0.0, 255.0);
                    }
                    "vintage" => {
                        r = (r * 0.92 + 24.0).clamp(0.0, 255.0);
                        g = (g * 0.88 + 18.0).clamp(0.0, 255.0);
                        b = (b * 0.78 + 14.0).clamp(0.0, 255.0);
                    }
                    "moody" => {
                        let gray = 0.299 * r + 0.587 * g + 0.114 * b;
                        r = ((r * 0.75 + gray * 0.25) * 0.92).clamp(0.0, 255.0);
                        g = ((g * 0.75 + gray * 0.25) * 0.94).clamp(0.0, 255.0);
                        b = ((b * 0.75 + gray * 0.25) * 1.02).clamp(0.0, 255.0);
                    }
                    "pastel" => {
                        r = (r * 0.82 + 44.0).clamp(0.0, 255.0);
                        g = (g * 0.82 + 42.0).clamp(0.0, 255.0);
                        b = (b * 0.82 + 48.0).clamp(0.0, 255.0);
                    }
                    _ => {}
                }
            }

            if has_vig {
                let dx = px as f32 - cx;
                let dy = py as f32 - cy;
                let dist = (dx * dx + dy * dy).sqrt() / max_r;
                let falloff = (1.0 - (dist * dist * vig_str * 0.9)).clamp(0.15, 1.0);
                r *= falloff;
                g *= falloff;
                b *= falloff;
            }

            p[0] = r.round().clamp(0.0, 255.0) as u8;
            p[1] = g.round().clamp(0.0, 255.0) as u8;
            p[2] = b.round().clamp(0.0, 255.0) as u8;
        }
    }

    out
}

fn rgb_to_hsl(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) * 0.5;
    if (max - min).abs() < 1e-6 {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
    let h = if (max - r).abs() < 1e-6 {
        ((g - b) / d + if g < b { 6.0 } else { 0.0 }) / 6.0
    } else if (max - g).abs() < 1e-6 {
        ((b - r) / d + 2.0) / 6.0
    } else {
        ((r - g) / d + 4.0) / 6.0
    };
    (h, s, l)
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    if s <= 1e-6 {
        return (l, l, l);
    }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let hue2rgb = |mut t: f32| -> f32 {
        if t < 0.0 { t += 1.0; }
        if t > 1.0 { t -= 1.0; }
        if t < 1.0 / 6.0 { return p + (q - p) * 6.0 * t; }
        if t < 0.5 { return q; }
        if t < 2.0 / 3.0 { return p + (q - p) * (2.0 / 3.0 - t) * 6.0; }
        p
    };
    (hue2rgb(h + 1.0 / 3.0), hue2rgb(h), hue2rgb(h - 1.0 / 3.0))
}

fn rotate_rgba_bilinear(src: &RgbaImage, deg: f32) -> RgbaImage {
    let rad = deg.to_radians();
    let (cos_a, sin_a) = (rad.cos(), rad.sin());
    let (sw, sh) = (src.width() as f32, src.height() as f32);
    let nw = (sw * cos_a.abs() + sh * sin_a.abs()).ceil().max(1.0) as u32;
    let nh = (sw * sin_a.abs() + sh * cos_a.abs()).ceil().max(1.0) as u32;

    let mut dst = RgbaImage::from_pixel(nw, nh, Rgba([0, 0, 0, 0]));
    let (scx, scy) = (sw / 2.0, sh / 2.0);
    let (dcx, dcy) = (nw as f32 / 2.0, nh as f32 / 2.0);

    for dy in 0..nh {
        for dx in 0..nw {
            let rx = dx as f32 - dcx;
            let ry = dy as f32 - dcy;
            let sx = rx * cos_a + ry * sin_a + scx;
            let sy = -rx * sin_a + ry * cos_a + scy;
            let ix = sx.round() as i32;
            let iy = sy.round() as i32;
            if ix >= 0 && iy >= 0 && ix < src.width() as i32 && iy < src.height() as i32 {
                dst.put_pixel(dx, dy, *src.get_pixel(ix as u32, iy as u32));
            }
        }
    }
    dst
}

fn apply_outline(src: &RgbaImage, color: [u8; 4], thickness: u32) -> RgbaImage {
    let t = thickness.min(24) as i32;
    let sw = src.width() as i32;
    let sh = src.height() as i32;
    let nw = (sw + t * 2) as u32;
    let nh = (sh + t * 2) as u32;
    let mut dst = RgbaImage::from_pixel(nw, nh, Rgba([0, 0, 0, 0]));

    let t2 = t * t;
    for y in 0..nh as i32 {
        for x in 0..nw as i32 {
            let sx = x - t;
            let sy = y - t;
            if sx >= 0 && sy >= 0 && sx < sw && sy < sh {
                let p = src.get_pixel(sx as u32, sy as u32);
                if p[3] > 180 {
                    dst.put_pixel(x as u32, y as u32, *p);
                    continue;
                }
            }
            let mut hit = false;
            'search: for oy in -t..=t {
                for ox in -t..=t {
                    if ox * ox + oy * oy <= t2 {
                        let nx = sx + ox;
                        let ny = sy + oy;
                        if nx >= 0 && ny >= 0 && nx < sw && ny < sh {
                            if src.get_pixel(nx as u32, ny as u32)[3] > 32 {
                                hit = true;
                                break 'search;
                            }
                        }
                    }
                }
            }
            if hit {
                dst.put_pixel(x as u32, y as u32, Rgba(color));
            }
        }
    }
    // Re-stamp original foreground cleanly on top
    for sy in 0..sh {
        for sx in 0..sw {
            let p = src.get_pixel(sx as u32, sy as u32);
            if p[3] > 0 {
                let dp = dst.get_pixel_mut((sx + t) as u32, (sy + t) as u32);
                blend_pixel_over(dp, p.0);
            }
        }
    }
    dst
}

fn composite_layer(
    canvas: &mut RgbaImage,
    layer: &RgbaImage,
    pos_x: i64,
    pos_y: i64,
    opacity: f32,
    blend_mode: &str,
) {
    let cw = canvas.width() as i64;
    let ch = canvas.height() as i64;
    let lw = layer.width() as i64;
    let lh = layer.height() as i64;

    for ly in 0..lh {
        let cy = pos_y + ly;
        if cy < 0 || cy >= ch {
            continue;
        }
        for lx in 0..lw {
            let cx = pos_x + lx;
            if cx < 0 || cx >= cw {
                continue;
            }
            let src = layer.get_pixel(lx as u32, ly as u32).0;
            if src[3] == 0 {
                continue;
            }
            let sa = (src[3] as f32 / 255.0) * opacity;
            if sa <= 0.001 {
                continue;
            }

            let dst = canvas.get_pixel_mut(cx as u32, cy as u32);
            let mut blended = [src[0], src[1], src[2]];
            if dst[3] > 0 {
                for i in 0..3 {
                    let s = src[i] as f32 / 255.0;
                    let d = dst[i] as f32 / 255.0;
                    let out = match blend_mode {
                        "multiply" => s * d,
                        "screen" => 1.0 - (1.0 - s) * (1.0 - d),
                        "overlay" => {
                            if d < 0.5 {
                                2.0 * s * d
                            } else {
                                1.0 - 2.0 * (1.0 - s) * (1.0 - d)
                            }
                        }
                        "difference" => (s - d).abs(),
                        "exclusion" => s + d - 2.0 * s * d,
                        "color-dodge" => {
                            if s >= 0.999 { 1.0 } else { (d / (1.0 - s)).min(1.0) }
                        }
                        "color-burn" => {
                            if s <= 0.001 { 0.0 } else { 1.0 - ((1.0 - d) / s).min(1.0) }
                        }
                        _ => s,
                    };
                    blended[i] = (out.clamp(0.0, 1.0) * 255.0) as u8;
                }
            }
            blend_pixel_over(dst, [blended[0], blended[1], blended[2], (sa * 255.0) as u8]);
        }
    }
}

fn blend_pixel_over(dst: &mut Rgba<u8>, src: [u8; 4]) {
    let sa = src[3] as f32 / 255.0;
    let da = dst[3] as f32 / 255.0;
    let out_a = sa + da * (1.0 - sa);
    if out_a <= 0.0001 {
        *dst = Rgba([0, 0, 0, 0]);
        return;
    }
    for i in 0..3 {
        let sc = src[i] as f32 / 255.0;
        let dc = dst[i] as f32 / 255.0;
        let oc = (sc * sa + dc * da * (1.0 - sa)) / out_a;
        dst[i] = (oc.clamp(0.0, 1.0) * 255.0) as u8;
    }
    dst[3] = (out_a.clamp(0.0, 1.0) * 255.0) as u8;
}

fn apply_shape_mask(src: &RgbaImage, mask_kind: &str, radius: u32) -> RgbaImage {
    let mut out = src.clone();
    let (w, h) = (out.width() as f32, out.height() as f32);
    let shape = match mask_kind {
        "circle" => "circle",
        "star" => "star",
        _ => "rectangle",
    };
    let r = if mask_kind == "rounded" { radius.max(16) } else { 0 };
    for y in 0..out.height() {
        for x in 0..out.width() {
            if !shape_hit(shape, x as f32 + 0.5, y as f32 + 0.5, w, h, r, 5, None) {
                out.get_pixel_mut(x, y)[3] = 0;
            }
        }
    }
    out
}

fn apply_outer_glow(src: &RgbaImage, color: [u8; 4], intensity: f32) -> RgbaImage {
    let pad = (intensity.ceil() as u32).min(40).max(2);
    let nw = src.width() + pad * 2;
    let nh = src.height() + pad * 2;
    let mut sil = RgbaImage::from_pixel(nw, nh, Rgba([0, 0, 0, 0]));
    for y in 0..src.height() {
        for x in 0..src.width() {
            let a = src.get_pixel(x, y)[3];
            if a > 0 {
                let ga = ((a as f32 / 255.0) * (color[3] as f32 / 255.0) * 255.0) as u8;
                sil.put_pixel(x + pad, y + pad, Rgba([color[0], color[1], color[2], ga]));
            }
        }
    }
    let mut blurred = DynamicImage::ImageRgba8(sil)
        .blur((intensity * 0.45).clamp(1.0, 18.0))
        .to_rgba8();
    for y in 0..src.height() {
        for x in 0..src.width() {
            let p = src.get_pixel(x, y);
            if p[3] > 0 {
                let dst = blurred.get_pixel_mut(x + pad, y + pad);
                blend_pixel_over(dst, p.0);
            }
        }
    }
    blurred
}

fn apply_inner_glow(src: &RgbaImage, color: [u8; 4], intensity: f32) -> RgbaImage {
    let mut out = src.clone();
    let rad = (intensity.round() as i32).clamp(1, 20);
    let r2 = rad * rad;
    let (sw, sh) = (src.width() as i32, src.height() as i32);
    for y in 0..sh {
        for x in 0..sw {
            let orig = src.get_pixel(x as u32, y as u32).0;
            if orig[3] == 0 {
                continue;
            }
            let mut min_d2 = r2 + 1;
            for oy in -rad..=rad {
                for ox in -rad..=rad {
                    let d2 = ox * ox + oy * oy;
                    if d2 < min_d2 {
                        let nx = x + ox;
                        let ny = y + oy;
                        if nx < 0 || ny < 0 || nx >= sw || ny >= sh || src.get_pixel(nx as u32, ny as u32)[3] < 32 {
                            min_d2 = d2;
                        }
                    }
                }
            }
            if min_d2 <= r2 {
                let t = 1.0 - (min_d2 as f32).sqrt() / (rad as f32 + 0.5);
                let ga = (t * (color[3] as f32 / 255.0) * 200.0).clamp(0.0, 255.0) as u8;
                let dst = out.get_pixel_mut(x as u32, y as u32);
                blend_pixel_over(dst, [color[0], color[1], color[2], ga]);
                dst[3] = orig[3];
            }
        }
    }
    out
}

fn apply_drop_shadow(src: &RgbaImage, sh: &DropShadow) -> (RgbaImage, i64, i64) {
    let pad = (sh.blur.ceil() as i32 + sh.distance.ceil() as i32 + 6).min(60).max(4);
    let nw = (src.width() as i32 + pad * 2) as u32;
    let nh = (src.height() as i32 + pad * 2) as u32;

    let rad = sh.angle.to_radians();
    let dx = (sh.distance * rad.cos()).round() as i32;
    let dy = (sh.distance * rad.sin()).round() as i32;

    let mut sil = RgbaImage::from_pixel(nw, nh, Rgba([0, 0, 0, 0]));
    for y in 0..src.height() as i32 {
        for x in 0..src.width() as i32 {
            let a = src.get_pixel(x as u32, y as u32)[3];
            if a > 0 {
                let tx = x + pad + dx;
                let ty = y + pad + dy;
                if tx >= 0 && ty >= 0 && tx < nw as i32 && ty < nh as i32 {
                    let sa = ((a as f32 / 255.0) * sh.opacity.clamp(0.0, 1.0) * 255.0) as u8;
                    sil.put_pixel(tx as u32, ty as u32, Rgba([sh.color[0], sh.color[1], sh.color[2], sa]));
                }
            }
        }
    }
    let mut out = if sh.blur > 0.4 {
        DynamicImage::ImageRgba8(sil)
            .blur((sh.blur * 0.5).clamp(0.5, 25.0))
            .to_rgba8()
    } else {
        sil
    };
    for y in 0..src.height() {
        for x in 0..src.width() {
            let p = src.get_pixel(x, y);
            if p[3] > 0 {
                let dst = out.get_pixel_mut(x + pad as u32, y + pad as u32);
                blend_pixel_over(dst, p.0);
            }
        }
    }
    (out, pad as i64, pad as i64)
}

fn composite_layer_clipped(
    canvas: &mut RgbaImage,
    layer: &RgbaImage,
    pos_x: i64,
    pos_y: i64,
    opacity: f32,
    blend_mode: &str,
    mask_alpha: &[u8],
) {
    let cw = canvas.width() as i64;
    let ch = canvas.height() as i64;
    let lw = layer.width() as i64;
    let lh = layer.height() as i64;

    for ly in 0..lh {
        let cy = pos_y + ly;
        if cy < 0 || cy >= ch {
            continue;
        }
        for lx in 0..lw {
            let cx = pos_x + lx;
            if cx < 0 || cx >= cw {
                continue;
            }
            let ma = mask_alpha[(cy as u32 * canvas.width() + cx as u32) as usize];
            if ma == 0 {
                continue;
            }
            let src = layer.get_pixel(lx as u32, ly as u32).0;
            if src[3] == 0 {
                continue;
            }
            let sa = (src[3] as f32 / 255.0) * (ma as f32 / 255.0) * opacity;
            if sa <= 0.001 {
                continue;
            }
            let dst = canvas.get_pixel_mut(cx as u32, cy as u32);
            let mut blended = [src[0], src[1], src[2]];
            if dst[3] > 0 {
                for i in 0..3 {
                    let s = src[i] as f32 / 255.0;
                    let d = dst[i] as f32 / 255.0;
                    let out = match blend_mode {
                        "multiply" => s * d,
                        "screen" => 1.0 - (1.0 - s) * (1.0 - d),
                        "overlay" => if d < 0.5 { 2.0 * s * d } else { 1.0 - 2.0 * (1.0 - s) * (1.0 - d) },
                        _ => s,
                    };
                    blended[i] = (out.clamp(0.0, 1.0) * 255.0) as u8;
                }
            }
            blend_pixel_over(dst, [blended[0], blended[1], blended[2], (sa * 255.0) as u8]);
        }
    }
}

pub fn apply_alpha_mask_file(src: &RgbaImage, mask_path: &str) -> RgbaImage {
    let Ok(mask_dyn) = image::open(mask_path) else {
        return src.clone();
    };
    let (w, h) = (src.width(), src.height());
    let mask_luma = if mask_dyn.width() == w && mask_dyn.height() == h {
        mask_dyn.to_luma8()
    } else {
        mask_dyn
            .resize_exact(w, h, FilterType::Triangle)
            .to_luma8()
    };
    let mut out = src.clone();
    for y in 0..h {
        for x in 0..w {
            let m = mask_luma.get_pixel(x, y)[0] as u16;
            let p = out.get_pixel_mut(x, y);
            p[3] = ((p[3] as u16 * m) / 255) as u8;
        }
    }
    out
}

#[derive(serde::Serialize)]
pub struct DragLayersPreview {
    pub below_path: String,
    pub layer_path: String,
    pub above_path: Option<String>,
    pub layer_x: i64,
    pub layer_y: i64,
    pub layer_w: u32,
    pub layer_h: u32,
    pub pad_left: i64,
    pub pad_top: i64,
    pub scale: f32,
    pub rotation: f32,
    pub opacity: f32,
    pub blend_mode: String,
}

fn render_layer_into_canvas(
    canvas: &mut RgbaImage,
    layer: &mut Layer,
    groups_ref: &[LayerGroup],
    prev_canvas_alpha: &mut Option<Vec<u8>>,
    cw: u32,
    ch: u32,
) {
    let Ok(mut layer_img) = render_layer_kind(&layer.kind) else {
        return;
    };
    layer.bounds_width = layer_img.width();
    layer.bounds_height = layer_img.height();

    if !layer.visible || !is_group_visible(layer.group_id.as_deref(), groups_ref) || layer.opacity <= 0.001 {
        return;
    }

    if let Some(ref mask_path) = layer.alpha_mask_path {
        layer_img = apply_alpha_mask_file(&layer_img, mask_path);
    }
    if let Some(ref mask_kind) = layer.mask_shape {
        layer_img = apply_shape_mask(&layer_img, mask_kind, layer.mask_radius);
    }
    layer_img = apply_layer_effects(layer_img, &layer.effects);

    if (layer.scale - 1.0).abs() > 0.01 {
        let nw = ((layer_img.width() as f32 * layer.scale).round() as u32).max(1);
        let nh = ((layer_img.height() as f32 * layer.scale).round() as u32).max(1);
        layer_img = DynamicImage::ImageRgba8(layer_img)
            .resize_exact(nw, nh, FilterType::Lanczos3)
            .to_rgba8();
    }
    if layer.rotation.abs() > 0.1 {
        layer_img = rotate_rgba_bilinear(&layer_img, layer.rotation);
    }
    if layer.flip_h {
        layer_img = image::imageops::flip_horizontal(&layer_img);
    }
    if layer.flip_v {
        layer_img = image::imageops::flip_vertical(&layer_img);
    }
    if layer.inner_glow.enabled && layer.inner_glow.intensity > 0.5 {
        layer_img = apply_inner_glow(&layer_img, layer.inner_glow.color, layer.inner_glow.intensity);
    }

    let mut draw_x = layer.x;
    let mut draw_y = layer.y;

    if layer.outline.enabled && layer.outline.thickness > 0 {
        let t = layer.outline.thickness.min(24) as i64;
        layer_img = apply_outline(&layer_img, layer.outline.color, layer.outline.thickness);
        draw_x -= t;
        draw_y -= t;
    }
    if layer.outer_glow.enabled && layer.outer_glow.intensity > 0.5 {
        let pad = (layer.outer_glow.intensity.ceil() as i64).min(40);
        layer_img = apply_outer_glow(&layer_img, layer.outer_glow.color, layer.outer_glow.intensity);
        draw_x -= pad;
        draw_y -= pad;
    }
    if layer.shadow.enabled && layer.shadow.opacity > 0.01 {
        let (sh_img, off_x, off_y) = apply_drop_shadow(&layer_img, &layer.shadow);
        layer_img = sh_img;
        draw_x -= off_x;
        draw_y -= off_y;
    }

    if layer.clip_to_below {
        if let Some(ref mask_alpha) = prev_canvas_alpha {
            composite_layer_clipped(
                canvas,
                &layer_img,
                draw_x,
                draw_y,
                layer.opacity.clamp(0.0, 1.0),
                &layer.blend_mode,
                mask_alpha,
            );
            return;
        }
    }

    let mut alpha_map = vec![0u8; (cw * ch) as usize];
    for ly in 0..layer_img.height() as i64 {
        let cy = draw_y + ly;
        if cy < 0 || cy >= ch as i64 {
            continue;
        }
        for lx in 0..layer_img.width() as i64 {
            let cx = draw_x + lx;
            if cx < 0 || cx >= cw as i64 {
                continue;
            }
            alpha_map[(cy as u32 * cw + cx as u32) as usize] =
                layer_img.get_pixel(lx as u32, ly as u32)[3];
        }
    }
    *prev_canvas_alpha = Some(alpha_map);

    composite_layer(
        canvas,
        &layer_img,
        draw_x,
        draw_y,
        layer.opacity.clamp(0.0, 1.0),
        &layer.blend_mode,
    );
}

/// Renders 3 split PNGs for 60 FPS GPU-accelerated drag/scale/rotate preview.
pub fn render_drag_split(
    snapshot: &mut ProjectSnapshot,
    target_layer_id: &str,
    below_dest: &str,
    layer_dest: &str,
    above_dest: &str,
) -> Result<DragLayersPreview, String> {
    let cw = snapshot.canvas_width.max(1);
    let ch = snapshot.canvas_height.max(1);

    let mut below_canvas = if snapshot.transparent_bg {
        RgbaImage::from_pixel(cw, ch, Rgba([0, 0, 0, 0]))
    } else if let Some(ref grad) = snapshot.bg_gradient {
        render_gradient_image(cw, ch, grad.from_color, grad.to_color, grad.angle)
    } else {
        RgbaImage::from_pixel(cw, ch, Rgba(snapshot.bg_color))
    };
    let mut above_canvas = RgbaImage::from_pixel(cw, ch, Rgba([0, 0, 0, 0]));

    let groups_ref = snapshot.groups.clone();
    let mut prev_canvas_alpha: Option<Vec<u8>> = None;
    let mut found_target = false;
    let mut has_above = false;
    let mut target_preview: Option<(RgbaImage, i64, i64, u32, u32, i64, i64, f32, f32, f32, String)> = None;

    for layer in &mut snapshot.layers {
        if layer.id == target_layer_id {
            found_target = true;
            let mut layer_img = render_layer_kind(&layer.kind)?;
            let base_w = layer_img.width();
            let base_h = layer_img.height();
            layer.bounds_width = base_w;
            layer.bounds_height = base_h;

            if let Some(ref mask_path) = layer.alpha_mask_path {
                layer_img = apply_alpha_mask_file(&layer_img, mask_path);
            }
            if let Some(ref mask_kind) = layer.mask_shape {
                layer_img = apply_shape_mask(&layer_img, mask_kind, layer.mask_radius);
            }
            layer_img = apply_layer_effects(layer_img, &layer.effects);
            if layer.flip_h {
                layer_img = image::imageops::flip_horizontal(&layer_img);
            }
            if layer.flip_v {
                layer_img = image::imageops::flip_vertical(&layer_img);
            }
            if layer.inner_glow.enabled && layer.inner_glow.intensity > 0.5 {
                layer_img = apply_inner_glow(&layer_img, layer.inner_glow.color, layer.inner_glow.intensity);
            }
            let mut pad_left = 0i64;
            let mut pad_top = 0i64;
            if layer.outline.enabled && layer.outline.thickness > 0 {
                let t = layer.outline.thickness.min(24) as i64;
                layer_img = apply_outline(&layer_img, layer.outline.color, layer.outline.thickness);
                pad_left += t;
                pad_top += t;
            }
            if layer.outer_glow.enabled && layer.outer_glow.intensity > 0.5 {
                let pad = (layer.outer_glow.intensity.ceil() as i64).min(40);
                layer_img = apply_outer_glow(&layer_img, layer.outer_glow.color, layer.outer_glow.intensity);
                pad_left += pad;
                pad_top += pad;
            }
            if layer.shadow.enabled && layer.shadow.opacity > 0.01 {
                let (sh_img, off_x, off_y) = apply_drop_shadow(&layer_img, &layer.shadow);
                layer_img = sh_img;
                pad_left += off_x;
                pad_top += off_y;
            }
            target_preview = Some((
                layer_img,
                layer.x,
                layer.y,
                base_w,
                base_h,
                pad_left,
                pad_top,
                layer.scale,
                layer.rotation,
                layer.opacity.clamp(0.0, 1.0),
                layer.blend_mode.clone(),
            ));
            continue;
        }

        if !found_target {
            render_layer_into_canvas(
                &mut below_canvas,
                layer,
                &groups_ref,
                &mut prev_canvas_alpha,
                cw,
                ch,
            );
        } else {
            if layer.visible && is_group_visible(layer.group_id.as_deref(), &groups_ref) && layer.opacity > 0.001 {
                has_above = true;
            }
            render_layer_into_canvas(
                &mut above_canvas,
                layer,
                &groups_ref,
                &mut prev_canvas_alpha,
                cw,
                ch,
            );
        }
    }

    let (layer_img, layer_x, layer_y, layer_w, layer_h, pad_left, pad_top, scale, rotation, opacity, blend_mode) =
        target_preview.ok_or_else(|| format!("Layer '{}' not found.", target_layer_id))?;

    DynamicImage::ImageRgba8(below_canvas)
        .save(below_dest)
        .map_err(|e| format!("Failed to save drag below: {}", e))?;
    DynamicImage::ImageRgba8(layer_img)
        .save(layer_dest)
        .map_err(|e| format!("Failed to save drag layer: {}", e))?;
    let above_path = if has_above {
        DynamicImage::ImageRgba8(above_canvas)
            .save(above_dest)
            .map_err(|e| format!("Failed to save drag above: {}", e))?;
        Some(above_dest.to_string())
    } else {
        None
    };

    Ok(DragLayersPreview {
        below_path: below_dest.to_string(),
        layer_path: layer_dest.to_string(),
        above_path,
        layer_x,
        layer_y,
        layer_w,
        layer_h,
        pad_left,
        pad_top,
        scale,
        rotation,
        opacity,
        blend_mode,
    })
}
