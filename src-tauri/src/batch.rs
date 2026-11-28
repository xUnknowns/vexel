use crate::layers::{render_project_composite, SharedProjectState};
use crate::templates::{
    apply_fields_to_snapshot, load_all_vtemplates, resolve_vtemplate, VTemplate,
};
use crate::tools::{save_image_formatted, BatchPresetInput};
use image::{imageops::FilterType, DynamicImage, Rgba, RgbaImage};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use tauri::Emitter;

static BATCH_JOBS: OnceLock<Mutex<HashMap<String, Arc<AtomicBool>>>> = OnceLock::new();

fn jobs() -> &'static Mutex<HashMap<String, Arc<AtomicBool>>> {
    BATCH_JOBS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn register_job(job_id: &str) -> Result<Arc<AtomicBool>, String> {
    if job_id.trim().is_empty() || job_id.len() > 128 {
        return Err("Invalid batch job ID.".to_string());
    }
    let mut active = jobs().lock().map_err(|e| e.to_string())?;
    if active.contains_key(job_id) {
        return Err("A batch job with this ID is already running.".to_string());
    }
    let cancel = Arc::new(AtomicBool::new(false));
    active.insert(job_id.to_string(), cancel.clone());
    Ok(cancel)
}

fn finish_job(job_id: &str) {
    if let Ok(mut active) = jobs().lock() {
        active.remove(job_id);
    }
}

#[derive(Clone, Serialize)]
pub struct BatchProgressEvent {
    pub job_id: String,
    pub done: usize,
    pub total: usize,
    pub current_item: String,
    pub finished: bool,
    pub canceled: bool,
    pub saved: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Clone, Serialize)]
pub struct BatchSummary {
    pub saved: Vec<String>,
    pub errors: Vec<String>,
    pub canceled: bool,
}

#[tauri::command]
pub fn cancel_batch_export(job_id: String) -> Result<(), String> {
    let active = jobs().lock().map_err(|e| e.to_string())?;
    let cancel = active
        .get(&job_id)
        .ok_or_else(|| "This batch job is no longer running.".to_string())?;
    cancel.store(true, Ordering::SeqCst);
    Ok(())
}

fn resize_contain(img: &DynamicImage, width: u32, height: u32, transparent: bool) -> DynamicImage {
    let width = width.max(1);
    let height = height.max(1);
    let source = img.to_rgba8();
    let pad = if transparent {
        Rgba([0, 0, 0, 0])
    } else {
        *source.get_pixel(0, 0)
    };
    let resized = img.resize(width, height, FilterType::Lanczos3).to_rgba8();
    let left = (width.saturating_sub(resized.width()) / 2) as i64;
    let top = (height.saturating_sub(resized.height()) / 2) as i64;
    let mut canvas = RgbaImage::from_pixel(width, height, pad);
    image::imageops::overlay(&mut canvas, &resized, left, top);
    DynamicImage::ImageRgba8(canvas)
}

fn safe_component(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Encodes beside the destination and renames only after encoding succeeds.
/// Existing user files are never overwritten by a batch export.
fn save_output_atomic(
    img: &DynamicImage,
    dest: &Path,
    format: &str,
    quality: u8,
    job_id: &str,
    index: usize,
) -> Result<(), String> {
    if dest.exists() {
        return Err("Output file already exists; it was left unchanged.".to_string());
    }
    let parent = dest
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let stem = dest
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("export");
    let ext = dest.extension().and_then(|s| s.to_str()).unwrap_or("png");
    let temp = parent.join(format!(
        ".{}.{}.{}.tmp.{}",
        safe_component(stem),
        safe_component(job_id),
        index,
        safe_component(ext)
    ));

    save_image_formatted(img, &temp, format, quality)?;
    if dest.exists() {
        let _ = std::fs::remove_file(&temp);
        return Err("Output file already exists; it was left unchanged.".to_string());
    }
    if let Err(error) = std::fs::rename(&temp, dest) {
        let _ = std::fs::remove_file(&temp);
        return Err(format!("Could not commit output file: {}", error));
    }
    Ok(())
}

fn render_template_values(
    template: &VTemplate,
    values: &HashMap<String, String>,
) -> Result<DynamicImage, String> {
    let mut snapshot = apply_fields_to_snapshot(template, values, None);
    render_project_composite(&mut snapshot, "")
}

fn output_format(format: &str) -> Result<(&'static str, &'static str), String> {
    match format.to_ascii_lowercase().as_str() {
        "png" => Ok(("png", "png")),
        "jpeg" | "jpg" => Ok(("jpeg", "jpg")),
        "webp" => Ok(("webp", "webp")),
        _ => Err("Supported output formats are png, jpeg, and webp.".to_string()),
    }
}

fn safe_batch_filename(raw: &str, extension: &str) -> String {
    let safe: String = raw
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let safe = safe.trim_matches(&[' ', '.'][..]);
    let safe = if safe.is_empty() { "output" } else { safe };
    if safe.to_lowercase().ends_with(&format!(".{}", extension)) {
        safe.to_string()
    } else {
        format!("{}.{}", safe, extension)
    }
}

pub fn cli_render_template(
    template_ref: &str,
    values: &HashMap<String, String>,
    output_path: &Path,
    format: &str,
    quality: u8,
    size: Option<(u32, u32)>,
) -> Result<(), String> {
    let template = resolve_vtemplate(template_ref)?;
    let (format, _) = output_format(format)?;
    let mut image = render_template_values(&template, values)?;
    if let Some((width, height)) = size {
        image = resize_contain(&image, width, height, template.snapshot.transparent_bg);
    }
    save_output_atomic(&image, output_path, format, quality, "cli", 0)
}

pub fn cli_batch_render(
    template_ref: &str,
    rows: &[HashMap<String, String>],
    dest_dir: &Path,
    filename_pattern: &str,
    format: &str,
    quality: u8,
) -> Result<BatchSummary, String> {
    if rows.is_empty() {
        return Err("No data rows found.".to_string());
    }
    let template = resolve_vtemplate(template_ref)?;
    let (format, extension) = output_format(format)?;
    std::fs::create_dir_all(dest_dir).map_err(|e| e.to_string())?;
    let pattern = if filename_pattern.trim().is_empty() {
        "output_{index}"
    } else {
        filename_pattern
    };
    let mut saved = Vec::new();
    let mut errors = Vec::new();

    for (index, row) in rows.iter().enumerate() {
        let mut name = pattern.replace("{index}", &(index + 1).to_string());
        for (key, value) in row {
            name = name.replace(&format!("{{{}}}", key), value);
        }
        let file_name = safe_batch_filename(&name, extension);
        let destination = dest_dir.join(&file_name);
        match render_template_values(&template, row).and_then(|image| {
            save_output_atomic(&image, &destination, format, quality, "cli", index)
        }) {
            Ok(()) => saved.push(destination.to_string_lossy().to_string()),
            Err(error) => errors.push(format!("Row {} ({}): {}", index + 1, file_name, error)),
        }
    }

    Ok(BatchSummary {
        saved,
        errors,
        canceled: false,
    })
}

#[tauri::command]
pub async fn export_multi_size_async(
    app: tauri::AppHandle,
    state: tauri::State<'_, SharedProjectState>,
    job_id: String,
    dest_dir: String,
    base_name: String,
    format: String,
    quality: u8,
    presets: Vec<BatchPresetInput>,
) -> Result<BatchSummary, String> {
    if presets.is_empty() {
        return Err("No presets selected for batch export.".to_string());
    }
    let snapshot = {
        let s = state.lock().map_err(|e| e.to_string())?;
        if !s.has_project {
            return Err("No active project to export.".to_string());
        }
        s.current.clone()
    };
    let transparent = snapshot.transparent_bg;
    let cancel = register_job(&job_id)?;
    let worker_cancel = cancel.clone();
    let worker_id = job_id.clone();
    let task = tokio::task::spawn_blocking(move || {
        let result = (|| {
            std::fs::create_dir_all(&dest_dir).map_err(|e| e.to_string())?;
            let mut snap_mut = snapshot;
            let base_img = render_project_composite(&mut snap_mut, "")?;
            let ext = match format.as_str() {
                "jpeg" | "jpg" => "jpg",
                "webp" => "webp",
                "svg" => "svg",
                "pdf" => "pdf",
                "ico" => "ico",
                _ => "png",
            };
            let safe_base = safe_component(&base_name);
            let total = presets.len();
            let mut saved = Vec::new();
            let mut errors = Vec::new();
            let mut canceled = false;

            for (idx, preset) in presets.iter().enumerate() {
                if worker_cancel.load(Ordering::SeqCst) {
                    canceled = true;
                    break;
                }
                let current_item = format!("{} ({}×{})", preset.id, preset.width, preset.height);
                let _ = app.emit(
                    "batch-progress",
                    BatchProgressEvent {
                        job_id: worker_id.clone(),
                        done: idx,
                        total,
                        current_item,
                        finished: false,
                        canceled: false,
                        saved: saved.clone(),
                        errors: errors.clone(),
                    },
                );
                let width = preset.width.max(1);
                let height = preset.height.max(1);
                let resized = resize_contain(&base_img, width, height, transparent);
                let out_path = Path::new(&dest_dir).join(format!(
                    "{}_{}_{}x{}.{}",
                    safe_base,
                    safe_component(&preset.id),
                    width,
                    height,
                    ext
                ));
                match save_output_atomic(&resized, &out_path, &format, quality, &worker_id, idx) {
                    Ok(()) => saved.push(out_path.to_string_lossy().to_string()),
                    Err(e) => errors.push(format!("{}: {}", preset.id, e)),
                }
            }

            let summary = BatchSummary {
                saved: saved.clone(),
                errors: errors.clone(),
                canceled,
            };
            let _ = app.emit(
                "batch-progress",
                BatchProgressEvent {
                    job_id: worker_id.clone(),
                    done: saved.len() + errors.len(),
                    total,
                    current_item: if canceled {
                        "Canceled".into()
                    } else {
                        "Done".into()
                    },
                    finished: true,
                    canceled,
                    saved,
                    errors,
                },
            );
            Ok(summary)
        })();
        finish_job(&worker_id);
        result
    });
    match task.await {
        Ok(result) => result,
        Err(e) => {
            finish_job(&job_id);
            Err(format!("Batch task failed: {}", e))
        }
    }
}

fn parse_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '"' {
            if in_quotes && chars.peek() == Some(&'"') {
                cur.push('"');
                chars.next();
            } else {
                in_quotes = !in_quotes;
            }
        } else if c == ',' && !in_quotes {
            fields.push(cur.trim().to_string());
            cur.clear();
        } else {
            cur.push(c);
        }
    }
    fields.push(cur.trim().to_string());
    fields
}

pub fn parse_batch_rows(raw: &str) -> Result<Vec<HashMap<String, String>>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("Batch table is empty.".to_string());
    }
    if trimmed.starts_with('[') {
        let arr: Vec<serde_json::Value> =
            serde_json::from_str(trimmed).map_err(|e| format!("Invalid JSON array: {}", e))?;
        let mut rows = Vec::new();
        for item in arr {
            if let Some(obj) = item.as_object() {
                let mut map = HashMap::new();
                for (k, v) in obj {
                    let value = match v {
                        serde_json::Value::String(st) => st.clone(),
                        other => other.to_string(),
                    };
                    map.insert(k.clone(), value);
                }
                rows.push(map);
            }
        }
        return Ok(rows);
    }

    let mut lines = trimmed.lines().filter(|l| !l.trim().is_empty());
    let header_line = lines.next().ok_or("CSV has no header row.")?;
    let headers = parse_csv_line(header_line);
    if headers.is_empty() || headers.iter().all(|h| h.is_empty()) {
        return Err("CSV header is empty.".to_string());
    }
    let mut rows = Vec::new();
    for line in lines {
        let cols = parse_csv_line(line);
        let mut map = HashMap::new();
        for (i, header) in headers.iter().enumerate() {
            map.insert(header.clone(), cols.get(i).cloned().unwrap_or_default());
        }
        rows.push(map);
    }
    Ok(rows)
}

#[tauri::command]
pub async fn batch_render_template(
    app: tauri::AppHandle,
    job_id: String,
    template_id: String,
    table_data: String,
    filename_pattern: String,
    dest_dir: String,
    format: String,
    quality: u8,
) -> Result<BatchSummary, String> {
    let rows = parse_batch_rows(&table_data)?;
    if rows.is_empty() {
        return Err("No data rows found.".to_string());
    }
    let all = load_all_vtemplates()?;
    let (tpl, _) = all
        .into_iter()
        .find(|(template, _)| template.id == template_id)
        .ok_or_else(|| format!("Template '{}' not found.", template_id))?;

    let cancel = register_job(&job_id)?;
    let worker_cancel = cancel.clone();
    let worker_id = job_id.clone();
    let task = tokio::task::spawn_blocking(move || {
        let result = (|| {
            std::fs::create_dir_all(&dest_dir).map_err(|e| e.to_string())?;
            let (format, ext) = output_format(&format)?;
            let pattern = if filename_pattern.trim().is_empty() {
                "output_{index}".to_string()
            } else {
                filename_pattern
            };
            let total = rows.len();
            let mut saved = Vec::new();
            let mut errors = Vec::new();
            let mut canceled = false;

            for (idx, row) in rows.iter().enumerate() {
                if worker_cancel.load(Ordering::SeqCst) {
                    canceled = true;
                    break;
                }
                let mut fname = pattern.replace("{index}", &(idx + 1).to_string());
                for (key, value) in row {
                    fname = fname.replace(&format!("{{{}}}", key), value);
                }
                let final_name = safe_batch_filename(&fname, ext);

                let _ = app.emit(
                    "batch-progress",
                    BatchProgressEvent {
                        job_id: worker_id.clone(),
                        done: idx,
                        total,
                        current_item: final_name.clone(),
                        finished: false,
                        canceled: false,
                        saved: saved.clone(),
                        errors: errors.clone(),
                    },
                );

                let out_path = Path::new(&dest_dir).join(&final_name);
                match render_template_values(&tpl, row).and_then(|img| {
                    save_output_atomic(&img, &out_path, format, quality, &worker_id, idx)
                }) {
                    Ok(()) => saved.push(out_path.to_string_lossy().to_string()),
                    Err(e) => errors.push(format!("Row {} ({}): {}", idx + 1, final_name, e)),
                }
            }

            let summary = BatchSummary {
                saved: saved.clone(),
                errors: errors.clone(),
                canceled,
            };
            let _ = app.emit(
                "batch-progress",
                BatchProgressEvent {
                    job_id: worker_id.clone(),
                    done: saved.len() + errors.len(),
                    total,
                    current_item: if canceled {
                        "Canceled".into()
                    } else {
                        "Done".into()
                    },
                    finished: true,
                    canceled,
                    saved,
                    errors,
                },
            );
            Ok(summary)
        })();
        finish_job(&worker_id);
        result
    });
    match task.await {
        Ok(result) => result,
        Err(e) => {
            finish_job(&job_id);
            Err(format!("Batch task failed: {}", e))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contain_resize_keeps_canvas_dimensions_and_aspect_ratio() {
        let source =
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(1600, 900, Rgba([12, 34, 56, 255])));
        let result = resize_contain(&source, 1080, 1920, false);
        assert_eq!((result.width(), result.height()), (1080, 1920));
        let pixels = result.to_rgba8();
        assert_eq!(*pixels.get_pixel(0, 0), Rgba([12, 34, 56, 255]));
        assert_eq!(*pixels.get_pixel(0, 500), Rgba([12, 34, 56, 255]));
    }

    #[test]
    fn contained_resize_leaves_transparent_padding_when_requested() {
        let source =
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(1600, 900, Rgba([12, 34, 56, 255])));
        let result = resize_contain(&source, 1080, 1920, true).to_rgba8();
        assert_eq!(*result.get_pixel(0, 0), Rgba([0, 0, 0, 0]));
        assert_eq!(*result.get_pixel(540, 960), Rgba([12, 34, 56, 255]));
    }

    #[test]
    fn csv_parser_handles_quoted_commas_and_escaped_quotes() {
        assert_eq!(
            parse_csv_line(r#"alpha,"beta, gamma","say ""hi"""#),
            vec!["alpha", "beta, gamma", "say \"hi\""]
        );
    }
}
