use image::{imageops::FilterType, DynamicImage};
use std::path::Path;

fn open_image(source_path: &str) -> Result<DynamicImage, String> {
    image::open(source_path)
        .map_err(|e| format!("Failed to open image '{}': {}", source_path, e))
}

fn save_image(img: &DynamicImage, dest_path: &str) -> Result<String, String> {
    let dest = Path::new(dest_path);
    if let Some(parent) = dest.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create destination directory: {}", e))?;
        }
    }

    let ext = dest
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase());

    let result = match ext.as_deref() {
        Some("jpg") | Some("jpeg") => DynamicImage::ImageRgb8(img.to_rgb8()).save(dest),
        _ => img.save(dest),
    };

    result.map_err(|e| format!("Failed to save image '{}': {}", dest_path, e))?;
    Ok(dest_path.to_string())
}

#[tauri::command]
pub fn adjust_brightness_contrast(
    source_path: String,
    dest_path: String,
    brightness: i32,
    contrast: f32,
) -> Result<String, String> {
    let img = open_image(&source_path)?;
    let adjusted = if brightness != 0 {
        img.brighten(brightness)
    } else {
        img
    };
    let final_img = if contrast.abs() > f32::EPSILON {
        adjusted.adjust_contrast(contrast)
    } else {
        adjusted
    };
    save_image(&final_img, &dest_path)
}

#[tauri::command]
pub fn transform_image(
    source_path: String,
    dest_path: String,
    action: String,
) -> Result<String, String> {
    let img = open_image(&source_path)?;
    let transformed = match action.as_str() {
        "rotate90" => img.rotate90(),
        "rotate180" => img.rotate180(),
        "flip_h" => img.fliph(),
        "flip_v" => img.flipv(),
        _ => {
            return Err(format!(
                "Unsupported transform action '{}'. Expected rotate90, rotate180, flip_h, or flip_v.",
                action
            ))
        }
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
            "Crop region ({}, {}, {}x{}) exceeds image dimensions ({}x{}).",
            x,
            y,
            width,
            height,
            img.width(),
            img.height()
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
        return Err("New width and height must be greater than zero.".to_string());
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
    let filtered = match filter.as_str() {
        "grayscale" => img.grayscale(),
        "invert" => {
            img.invert();
            img
        }
        "blur" => img.blur(2.0),
        _ => {
            return Err(format!(
                "Unsupported filter '{}'. Expected grayscale, invert, or blur.",
                filter
            ))
        }
    };
    save_image(&filtered, &dest_path)
}

#[tauri::command]
pub fn export_file(source_path: String, dest_path: String) -> Result<String, String> {
    let img = open_image(&source_path)?;
    save_image(&img, &dest_path)
}
