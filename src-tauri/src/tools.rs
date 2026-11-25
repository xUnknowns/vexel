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

// ── Commands ────────────────────────────────────────────────────────────────

#[derive(serde::Serialize)]
pub struct Dimensions {
    pub width: u32,
    pub height: u32,
}

#[tauri::command]
pub fn get_image_dimensions(path: String) -> Result<Dimensions, String> {
    let img = open_image(&path)?;
    Ok(Dimensions { width: img.width(), height: img.height() })
}

#[tauri::command]
pub fn create_blank_canvas(dest_path: String, width: u32, height: u32) -> Result<String, String> {
    if width == 0 || height == 0 {
        return Err("Canvas width and height must be greater than zero.".to_string());
    }
    let img = DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        width,
        height,
        image::Rgba([255u8, 255u8, 255u8, 255u8]),
    ));
    save_image(&img, &dest_path)
}

#[tauri::command]
pub fn adjust_brightness_contrast_saturation(
    source_path: String,
    dest_path: String,
    brightness: i32,
    contrast: f32,
    saturation: f32,
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
    dest_path: String,
    action: String,
) -> Result<String, String> {
    let img = open_image(&source_path)?;
    let transformed = match action.as_str() {
        "rotate90"  => img.rotate90(),
        "rotate180" => img.rotate180(),
        "flip_h"    => img.fliph(),
        "flip_v"    => img.flipv(),
        other => return Err(format!("Unsupported action '{}'. Expected rotate90, rotate180, flip_h, or flip_v.", other)),
    };
    save_image(&transformed, &dest_path)
}

#[tauri::command]
pub fn crop_image(
    source_path: String,
    dest_path: String,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> Result<String, String> {
    if width == 0 || height == 0 {
        return Err("Crop width and height must be greater than zero.".to_string());
    }
    let img = open_image(&source_path)?;
    if x.saturating_add(width) > img.width() || y.saturating_add(height) > img.height() {
        return Err(format!(
            "Crop region ({x},{y} + {width}×{height}) exceeds image bounds ({}×{}).",
            img.width(), img.height()
        ));
    }
    let cropped = img.crop_imm(x, y, width, height);
    save_image(&cropped, &dest_path)
}

#[tauri::command]
pub fn resize_image(
    source_path: String,
    dest_path: String,
    new_width: u32,
    new_height: u32,
) -> Result<String, String> {
    if new_width == 0 || new_height == 0 {
        return Err("Width and height must be greater than zero.".to_string());
    }
    let img = open_image(&source_path)?;
    let resized = img.resize_exact(new_width, new_height, FilterType::Lanczos3);
    save_image(&resized, &dest_path)
}

#[tauri::command]
pub fn apply_filter(
    source_path: String,
    dest_path: String,
    filter: String,
) -> Result<String, String> {
    let mut img = open_image(&source_path)?;
    let result = match filter.as_str() {
        "grayscale" => img.grayscale(),
        "invert" => { img.invert(); img }
        "blur"   => img.blur(2.0),
        "noir"   => img.grayscale().adjust_contrast(45.0),
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
        other => return Err(format!("Unsupported filter '{}'. Expected grayscale, invert, blur, noir, warm, or cool.", other)),
    };
    save_image(&result, &dest_path)
}

#[tauri::command]
pub fn export_file(source_path: String, dest_path: String) -> Result<String, String> {
    let img = open_image(&source_path)?;
    save_image(&img, &dest_path)
}
