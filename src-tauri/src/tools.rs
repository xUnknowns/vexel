use image::{imageops::FilterType, DynamicImage, GenericImageView};
use std::path::Path;

// ── Shared helpers ──────────────────────────────────────────────────────────

fn open_image(source_path: &str) -> Result<DynamicImage, String> {
    image::open(source_path)
        .map_err(|e| format!("Failed to open '{}': {}", source_path, e))
}

fn save_image(img: &DynamicImage, dest_path: &str) -> Result<String, String> {
    let dest = Path::new(dest_path);
    if let Some(parent) = dest.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        }
    }
    let ext = dest
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase());
    match ext.as_deref() {
        Some("jpg") | Some("jpeg") => DynamicImage::ImageRgb8(img.to_rgb8()).save(dest),
        _ => img.save(dest),
    }
    .map_err(|e| format!("Failed to save '{}': {}", dest_path, e))?;
    Ok(dest_path.to_string())
}

fn apply_saturation(img: DynamicImage, saturation: f32) -> DynamicImage {
    let factor = 1.0 + saturation / 100.0;
    let mut rgba = img.to_rgba8();
    for p in rgba.pixels_mut() {
        let r = p[0] as f32 / 255.0;
        let g = p[1] as f32 / 255.0;
        let b = p[2] as f32 / 255.0;
        let gray = 0.299 * r + 0.587 * g + 0.114 * b;
        p[0] = ((gray + factor * (r - gray)).clamp(0.0, 1.0) * 255.0) as u8;
        p[1] = ((gray + factor * (g - gray)).clamp(0.0, 1.0) * 255.0) as u8;
        p[2] = ((gray + factor * (b - gray)).clamp(0.0, 1.0) * 255.0) as u8;
    }
    DynamicImage::ImageRgba8(rgba)
}

fn parse_hex_color(hex: &str) -> [u8; 4] {
    let h = hex.trim_start_matches('#');
    if h.len() >= 6 {
        let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(255);
        let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(255);
        let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(255);
        [r, g, b, 255]
    } else {
        [255, 255, 255, 255]
    }
}

// ── Public preview helper (called by mcp.rs) ─────────────────────────────────

pub fn internal_get_preview(source_path: &str) -> Result<(String, u32, u32), String> {
    let img = open_image(source_path)?;
    let (w, h) = (img.width(), img.height());
    let preview = if w > 800 {
        let scale   = 800.0 / w as f32;
        let new_h   = ((h as f32 * scale) as u32).max(1);
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

// ── Commands ────────────────────────────────────────────────────────────────

#[derive(serde::Serialize)]
pub struct Dimensions { pub width: u32, pub height: u32 }

#[tauri::command]
pub fn get_image_dimensions(path: String) -> Result<Dimensions, String> {
    let img = open_image(&path)?;
    Ok(Dimensions { width: img.width(), height: img.height() })
}

#[tauri::command]
pub fn get_canvas_preview(source_path: String) -> Result<String, String> {
    let (b64, _, _) = internal_get_preview(&source_path)?;
    Ok(b64)
}

#[tauri::command]
pub fn create_blank_canvas(dest_path: String, width: u32, height: u32) -> Result<String, String> {
    if width == 0 || height == 0 {
        return Err("Canvas width and height must be greater than zero.".to_string());
    }
    let img = DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        width, height, image::Rgba([255u8, 255u8, 255u8, 255u8]),
    ));
    save_image(&img, &dest_path)
}

#[tauri::command]
pub fn adjust_brightness_contrast_saturation(
    source_path: String,
    dest_path:   String,
    brightness:  i32,
    contrast:    f32,
    saturation:  f32,
) -> Result<String, String> {
    let img = open_image(&source_path)?;
    let img = if brightness != 0 { img.brighten(brightness) } else { img };
    let img = if contrast.abs() > f32::EPSILON { img.adjust_contrast(contrast) } else { img };
    let img = if saturation.abs() > f32::EPSILON { apply_saturation(img, saturation) } else { img };
    save_image(&img, &dest_path)
}

#[tauri::command]
pub fn transform_image(
    source_path: String,
    dest_path:   String,
    action:      String,
) -> Result<String, String> {
    let img = open_image(&source_path)?;
    let t = match action.as_str() {
        "rotate90"  => img.rotate90(),
        "rotate180" => img.rotate180(),
        "flip_h"    => img.fliph(),
        "flip_v"    => img.flipv(),
        other => return Err(format!("Unsupported action '{}'.", other)),
    };
    save_image(&t, &dest_path)
}

#[tauri::command]
pub fn crop_image(
    source_path: String,
    dest_path:   String,
    x: u32, y: u32, width: u32, height: u32,
) -> Result<String, String> {
    if width == 0 || height == 0 {
        return Err("Crop width and height must be greater than zero.".to_string());
    }
    let img = open_image(&source_path)?;
    if x.saturating_add(width) > img.width() || y.saturating_add(height) > img.height() {
        return Err(format!(
            "Crop region exceeds image bounds ({}×{}).", img.width(), img.height()
        ));
    }
    save_image(&img.crop_imm(x, y, width, height), &dest_path)
}

#[tauri::command]
pub fn resize_image(
    source_path: String,
    dest_path:   String,
    new_width:  u32,
    new_height: u32,
) -> Result<String, String> {
    if new_width == 0 || new_height == 0 {
        return Err("Width and height must be greater than zero.".to_string());
    }
    let img = open_image(&source_path)?;
    save_image(&img.resize_exact(new_width, new_height, FilterType::Lanczos3), &dest_path)
}

#[tauri::command]
pub fn apply_filter(
    source_path: String,
    dest_path:   String,
    filter:      String,
) -> Result<String, String> {
    let mut img = open_image(&source_path)?;
    let result = match filter.as_str() {
        "grayscale" => img.grayscale(),
        "invert"    => { img.invert(); img }
        "blur"      => img.blur(2.0),
        "noir"      => img.grayscale().adjust_contrast(45.0),
        "warm" => {
            let mut rgba = img.to_rgba8();
            for p in rgba.pixels_mut() {
                p[0] = (p[0] as u16 + 25).min(255) as u8;
                p[1] = (p[1] as u16 +  8).min(255) as u8;
                p[2] = p[2].saturating_sub(20);
            }
            DynamicImage::ImageRgba8(rgba)
        }
        "cool" => {
            let mut rgba = img.to_rgba8();
            for p in rgba.pixels_mut() {
                p[0] = p[0].saturating_sub(20);
                p[1] = (p[1] as u16 + 5).min(255) as u8;
                p[2] = (p[2] as u16 + 25).min(255) as u8;
            }
            DynamicImage::ImageRgba8(rgba)
        }
        other => return Err(format!("Unsupported filter '{}'.", other)),
    };
    save_image(&result, &dest_path)
}

#[tauri::command]
pub fn export_file(source_path: String, dest_path: String) -> Result<String, String> {
    save_image(&open_image(&source_path)?, &dest_path)
}

// ── Export with format / quality / scale ─────────────────────────────────────

#[tauri::command]
pub fn export_image_configured(
    source_path: String,
    dest_path:   String,
    format:      String,  // "png" | "jpeg" | "webp"
    quality:     u8,      // 1–100
    scale:       f32,     // 0.5 | 1.0 | 2.0
) -> Result<String, String> {
    let img = open_image(&source_path)?;
    let img = if (scale - 1.0).abs() > 0.01 {
        let nw = ((img.width()  as f32 * scale) as u32).max(1);
        let nh = ((img.height() as f32 * scale) as u32).max(1);
        img.resize_exact(nw, nh, FilterType::Lanczos3)
    } else {
        img
    };

    let dest = Path::new(&dest_path);
    if let Some(p) = dest.parent() {
        if !p.as_os_str().is_empty() {
            std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
        }
    }

    match format.as_str() {
        "jpeg" | "jpg" => {
            let file = std::fs::File::create(dest).map_err(|e| e.to_string())?;
            let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(
                file,
                quality.clamp(1, 100),
            );
            enc.encode_image(&img.to_rgb8()).map_err(|e| e.to_string())?;
        }
        _ => { img.save(dest).map_err(|e| e.to_string())?; } // png, webp
    }
    Ok(dest_path)
}

// ── Text layer via ab_glyph ───────────────────────────────────────────────────

#[tauri::command]
pub fn add_text_layer(
    source_path: String,
    dest_path:   String,
    text:        String,
    font_style:  String,  // "modern_sans"|"classic_serif"|"tech_mono"|"bold_display"|"script"
    font_size:   f32,
    color_hex:   String,
    pos_x:       i32,
    pos_y:       i32,
) -> Result<String, String> {
    if text.is_empty() {
        return Err("Text content cannot be empty.".to_string());
    }
    let img     = open_image(&source_path)?;
    let mut rgba = img.to_rgba8();
    let color   = parse_hex_color(&color_hex);
    let font_data = load_font_bytes(&font_style)?;

    render_text_ab(&mut rgba, &font_data, &text, font_size, pos_x, pos_y, color)?;

    save_image(&DynamicImage::ImageRgba8(rgba), &dest_path)
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
            r"C:\Windows\Fonts\arialbd.ttf",
            r"C:\Windows\Fonts\calibrib.ttf",
            r"C:\Windows\Fonts\arial.ttf",
        ],
        "script" => &[
            r"C:\Windows\Fonts\segoesc.ttf",
            r"C:\Windows\Fonts\comic.ttf",
            r"C:\Windows\Fonts\arial.ttf",
        ],
        _ => &[ // modern_sans / default
            r"C:\Windows\Fonts\arial.ttf",
            r"C:\Windows\Fonts\calibri.ttf",
            r"C:\Windows\Fonts\segoeui.ttf",
        ],
    };
    for path in candidates {
        if let Ok(data) = std::fs::read(path) {
            return Ok(data);
        }
    }
    Err(format!(
        "No system font found for style '{}'. Expected fonts in C:\\Windows\\Fonts\\.",
        style
    ))
}

fn render_text_ab(
    img:     &mut image::RgbaImage,
    font_data: &[u8],
    text:    &str,
    size:    f32,
    start_x: i32,
    start_y: i32,
    color:   [u8; 4],
) -> Result<(), String> {
    use ab_glyph::{Font, FontVec, PxScale, ScaleFont};

    let font   = FontVec::try_from_vec(font_data.to_vec())
        .map_err(|_| "Failed to parse font data.".to_string())?;
    let scale  = PxScale::from(size);
    let scaled = font.as_scaled(scale);

    let (img_w, img_h) = (img.width() as i32, img.height() as i32);
    let mut pen_x      = start_x as f32;
    let pen_y          = start_y as f32;
    let mut prev_id    = None;

    for ch in text.chars() {
        let glyph_id = scaled.glyph_id(ch);
        if let Some(prev) = prev_id {
            pen_x += scaled.kern(prev, glyph_id);
        }
        prev_id = Some(glyph_id);

        let glyph = glyph_id.with_scale_and_position(scale, ab_glyph::point(pen_x, pen_y));
        if let Some(og) = font.outline_glyph(glyph) {
            let bb = og.px_bounds();
            og.draw(|rx, ry, v| {
                let px = bb.min.x as i32 + rx as i32;
                let py = bb.min.y as i32 + ry as i32;
                if px >= 0 && py >= 0 && px < img_w && py < img_h {
                    let pix    = img.get_pixel_mut(px as u32, py as u32);
                    let alpha  = v * (color[3] as f32 / 255.0);
                    let ialpha = 1.0 - alpha;
                    pix[0] = ((color[0] as f32 * alpha) + (pix[0] as f32 * ialpha)).clamp(0.0, 255.0) as u8;
                    pix[1] = ((color[1] as f32 * alpha) + (pix[1] as f32 * ialpha)).clamp(0.0, 255.0) as u8;
                    pix[2] = ((color[2] as f32 * alpha) + (pix[2] as f32 * ialpha)).clamp(0.0, 255.0) as u8;
                    pix[3] = 255;
                }
            });
        }
        pen_x += scaled.h_advance(glyph_id);
    }
    Ok(())
}
