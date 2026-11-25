use ab_glyph::{Font, FontVec, PxScale, ScaleFont};
use image::{imageops::FilterType, DynamicImage, Rgba, RgbaImage};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

// ── Data Structures ─────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LayerEffects {
    pub brightness: i32,
    pub contrast: f32,
    pub saturation: f32,
    pub blur: f32,
    pub filter: String, // "original" | "grayscale" | "noir" | "warm" | "cool" | "invert"
}

impl Default for LayerEffects {
    fn default() -> Self {
        Self {
            brightness: 0,
            contrast: 0.0,
            saturation: 0.0,
            blur: 0.0,
            filter: "original".to_string(),
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
    },
    Shape {
        shape_type: String, // "rectangle" | "circle" | "badge"
        width: u32,
        height: u32,
        fill_color: [u8; 4],
        border_radius: u32,
        #[serde(default)]
        border_color: [u8; 4],
        #[serde(default)]
        border_width: u32,
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
    pub effects: LayerEffects,
    pub outline: LayerOutline,
    pub kind: LayerKind,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectSnapshot {
    pub project_name: String,
    pub canvas_width: u32,
    pub canvas_height: u32,
    pub transparent_bg: bool,
    pub bg_color: [u8; 4],
    #[serde(default)]
    pub bg_gradient: Option<BgGradient>,
    pub layers: Vec<Layer>,
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
            project_name: "Untitled Design".to_string(),
            canvas_width: 1280,
            canvas_height: 720,
            transparent_bg: false,
            bg_color: [255, 255, 255, 255],
            bg_gradient: None,
            layers: Vec::new(),
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
        }
    }
}

pub type SharedProjectState = Arc<Mutex<ProjectState>>;

#[derive(Clone, Serialize)]
pub struct ProjectResponse {
    pub preview_path: String,
    pub project_name: String,
    pub canvas_width: u32,
    pub canvas_height: u32,
    pub transparent_bg: bool,
    pub bg_color: [u8; 4],
    pub bg_gradient: Option<BgGradient>,
    pub layers: Vec<Layer>,
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
            canvas_width: self.current.canvas_width,
            canvas_height: self.current.canvas_height,
            transparent_bg: self.current.transparent_bg,
            bg_color: self.current.bg_color,
            bg_gradient: self.current.bg_gradient.clone(),
            layers: self.current.layers.clone(),
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

    for layer in &mut snapshot.layers {
        let Ok(mut layer_img) = render_layer_kind(&layer.kind) else {
            continue;
        };
        layer.bounds_width = layer_img.width();
        layer.bounds_height = layer_img.height();

        if !layer.visible || layer.opacity <= 0.001 {
            continue;
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

        if layer.outline.enabled && layer.outline.thickness > 0 {
            layer_img = apply_outline(&layer_img, layer.outline.color, layer.outline.thickness);
        }

        composite_layer(
            &mut canvas,
            &layer_img,
            layer.x,
            layer.y,
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

fn render_layer_kind(kind: &LayerKind) -> Result<RgbaImage, String> {
    match kind {
        LayerKind::Image {
            source_path,
            bg_remove_color,
            bg_remove_tolerance,
        } => {
            let img = image::open(source_path)
                .map_err(|e| format!("Failed to open '{}': {}", source_path, e))?;
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
        } => render_text_3d(
            content,
            font_style,
            *color,
            *depth,
            *shadow_color,
            *stroke_color,
            *stroke_width,
            *size,
        ),
        LayerKind::Shape {
            shape_type,
            width,
            height,
            fill_color,
            border_radius,
            border_color,
            border_width,
        } => Ok(render_shape(
            shape_type,
            (*width).max(1),
            (*height).max(1),
            *fill_color,
            *border_radius,
            *border_color,
            *border_width,
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

fn render_text_3d(
    content: &str,
    font_style: &str,
    color: [u8; 4],
    depth: u32,
    shadow_color: [u8; 4],
    stroke_color: [u8; 4],
    stroke_width: u32,
    size: f32,
) -> Result<RgbaImage, String> {
    let font_bytes = load_font_bytes(font_style)?;
    let font = FontVec::try_from_vec(font_bytes).map_err(|_| "Failed to parse font".to_string())?;
    let scale = PxScale::from(size.max(8.0));
    let scaled = font.as_scaled(scale);

    let mut total_w: f32 = 0.0;
    let mut prev = None;
    for ch in content.chars() {
        let gid = scaled.glyph_id(ch);
        if let Some(p) = prev {
            total_w += scaled.kern(p, gid);
        }
        prev = Some(gid);
        total_w += scaled.h_advance(gid);
    }

    let pad = (depth + stroke_width + 12) as i32;
    let line_h = (scaled.ascent() - scaled.descent()).ceil() as i32;
    let img_w = (total_w.ceil() as i32 + pad * 2).max(16) as u32;
    let img_h = (line_h + pad * 2).max(16) as u32;
    let mut img = RgbaImage::from_pixel(img_w, img_h, Rgba([0, 0, 0, 0]));

    let base_x = pad as f32;
    let base_y = pad as f32 + scaled.ascent();

    // 1. Drop shadow
    if depth > 0 {
        let sh_off = (depth + 3) as f32;
        draw_text_pass(&mut img, &font, scale, content, base_x + sh_off, base_y + sh_off, shadow_color);
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
            draw_text_pass(&mut img, &font, scale, content, base_x + df, base_y + df, extrude_color);
        }
    }

    // 3. Stroke outline
    if stroke_width > 0 {
        let sw = stroke_width.min(10) as i32;
        for dy in -sw..=sw {
            for dx in -sw..=sw {
                if dx * dx + dy * dy <= sw * sw && (dx != 0 || dy != 0) {
                    draw_text_pass(
                        &mut img,
                        &font,
                        scale,
                        content,
                        base_x + dx as f32,
                        base_y + dy as f32,
                        stroke_color,
                    );
                }
            }
        }
    }

    // 4. Main face
    draw_text_pass(&mut img, &font, scale, content, base_x, base_y, color);
    Ok(img)
}

fn draw_text_pass(
    img: &mut RgbaImage,
    font: &FontVec,
    scale: PxScale,
    text: &str,
    start_x: f32,
    start_y: f32,
    color: [u8; 4],
) {
    let scaled = font.as_scaled(scale);
    let (w, h) = (img.width() as i32, img.height() as i32);
    let mut pen_x = start_x;
    let mut prev = None;

    for ch in text.chars() {
        let gid = scaled.glyph_id(ch);
        if let Some(p) = prev {
            pen_x += scaled.kern(p, gid);
        }
        prev = Some(gid);

        let glyph = gid.with_scale_and_position(scale, ab_glyph::point(pen_x, start_y));
        if let Some(og) = font.outline_glyph(glyph) {
            let bb = og.px_bounds();
            og.draw(|rx, ry, v| {
                let px = bb.min.x as i32 + rx as i32;
                let py = bb.min.y as i32 + ry as i32;
                if px >= 0 && py >= 0 && px < w && py < h {
                    let dst = img.get_pixel_mut(px as u32, py as u32);
                    let src_a = (v * (color[3] as f32 / 255.0)).clamp(0.0, 1.0);
                    blend_pixel_over(dst, [color[0], color[1], color[2], (src_a * 255.0) as u8]);
                }
            });
        }
        pen_x += scaled.h_advance(gid);
    }
}

fn render_shape(
    shape_type: &str,
    width: u32,
    height: u32,
    fill_color: [u8; 4],
    border_radius: u32,
    border_color: [u8; 4],
    border_width: u32,
) -> RgbaImage {
    let mut img = RgbaImage::from_pixel(width, height, Rgba([0, 0, 0, 0]));
    let r = match shape_type {
        "circle" => width.min(height) as f32 / 2.0,
        "badge" => height as f32 / 2.0,
        _ => (border_radius as f32).min(width.min(height) as f32 / 2.0),
    };

    for y in 0..height {
        for x in 0..width {
            let inside = if shape_type == "circle" {
                let cx = width as f32 / 2.0;
                let cy = height as f32 / 2.0;
                let rx = (width as f32 / 2.0).max(1.0);
                let ry = (height as f32 / 2.0).max(1.0);
                let dx = (x as f32 + 0.5 - cx) / rx;
                let dy = (y as f32 + 0.5 - cy) / ry;
                dx * dx + dy * dy <= 1.0
            } else if r > 0.5 {
                rounded_rect_hit(x as f32 + 0.5, y as f32 + 0.5, width as f32, height as f32, r)
            } else {
                true
            };
            if inside {
                // Check if pixel is on the border
                let bw = border_width as i32;
                let on_border = if border_width > 0 && border_color[3] > 0 {
                    let near_edge = (x as i32) < bw
                        || (x as i32) >= (width as i32 - bw)
                        || (y as i32) < bw
                        || (y as i32) >= (height as i32 - bw);
                    near_edge
                } else {
                    false
                };
                img.put_pixel(x, y, if on_border { Rgba(border_color) } else { Rgba(fill_color) });
            }
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
    let text_img = render_text_3d(text, font_style, color, 0, [0, 0, 0, 0], [0, 0, 0, 0], 0, size)?;
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

    dyn_img.to_rgba8()
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
