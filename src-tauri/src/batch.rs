use crate::layers::{render_project_composite, SharedProjectState};
use crate::templates::{apply_fields_to_snapshot, load_all_vtemplates};
use crate::tools::{save_image_formatted, BatchPresetInput};
use image::imageops::FilterType;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Emitter;

static BATCH_CANCEL: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Serialize)]
pub struct BatchProgressEvent {
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
pub fn cancel_batch_export() {
    BATCH_CANCEL.store(true, Ordering::SeqCst);
}

#[tauri::command]
pub async fn export_multi_size_async(
    app: tauri::AppHandle,
    state: tauri::State<'_, SharedProjectState>,
    dest_dir: String,
    base_name: String,
    format: String,
    quality: u8,
    presets: Vec<BatchPresetInput>,
) -> Result<BatchSummary, String> {
    let snapshot = {
        let s = state.lock().map_err(|e| e.to_string())?;
        if !s.has_project {
            return Err("No active project to export.".to_string());
        }
        s.current.clone()
    };

    BATCH_CANCEL.store(false, Ordering::SeqCst);

    tokio::task::spawn_blocking(move || {
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
        let safe_base: String = base_name
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect();
        let total = presets.len();
        let mut saved = Vec::new();
        let mut errors = Vec::new();
        let mut canceled = false;

        for (idx, p) in presets.iter().enumerate() {
            if BATCH_CANCEL.load(Ordering::SeqCst) {
                canceled = true;
                break;
            }
            let _ = app.emit(
                "batch-progress",
                BatchProgressEvent {
                    done: idx,
                    total,
                    current_item: format!("{} ({}×{})", p.id, p.width, p.height),
                    finished: false,
                    canceled: false,
                    saved: saved.clone(),
                    errors: errors.clone(),
                },
            );
            let out_path = Path::new(&dest_dir).join(format!(
                "{}_{}_{}x{}.{}",
                safe_base, p.id, p.width, p.height, ext
            ));
            let resized = base_img.resize_exact(p.width.max(1), p.height.max(1), FilterType::Lanczos3);
            match save_image_formatted(&resized, &out_path, &format, quality) {
                Ok(_) => saved.push(out_path.to_string_lossy().to_string()),
                Err(e) => errors.push(format!("{}: {}", p.id, e)),
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
                done: saved.len() + errors.len(),
                total,
                current_item: if canceled { "Canceled".into() } else { "Done".into() },
                finished: true,
                canceled,
                saved,
                errors,
            },
        );
        Ok(summary)
    })
    .await
    .map_err(|e| format!("Batch task failed: {}", e))?
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
    // Try JSON array first
    if trimmed.starts_with('[') {
        let arr: Vec<serde_json::Value> =
            serde_json::from_str(trimmed).map_err(|e| format!("Invalid JSON array: {}", e))?;
        let mut rows = Vec::new();
        for item in arr {
            if let Some(obj) = item.as_object() {
                let mut map = HashMap::new();
                for (k, v) in obj {
                    let s = match v {
                        serde_json::Value::String(st) => st.clone(),
                        other => other.to_string(),
                    };
                    map.insert(k.clone(), s);
                }
                rows.push(map);
            }
        }
        return Ok(rows);
    }

    // Parse CSV
    let mut lines = trimmed.lines().filter(|l| !l.trim().is_empty());
    let header_line = lines.next().ok_or("CSV has no header row.")?;
    let headers = parse_csv_line(header_line);
    if headers.is_empty() {
        return Err("CSV header is empty.".to_string());
    }
    let mut rows = Vec::new();
    for line in lines {
        let cols = parse_csv_line(line);
        let mut map = HashMap::new();
        for (i, h) in headers.iter().enumerate() {
            map.insert(h.clone(), cols.get(i).cloned().unwrap_or_default());
        }
        rows.push(map);
    }
    Ok(rows)
}

#[tauri::command]
pub async fn batch_render_template(
    app: tauri::AppHandle,
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
        .find(|(t, _)| t.id == template_id)
        .ok_or_else(|| format!("Template '{}' not found.", template_id))?;

    BATCH_CANCEL.store(false, Ordering::SeqCst);

    tokio::task::spawn_blocking(move || {
        std::fs::create_dir_all(&dest_dir).map_err(|e| e.to_string())?;
        let ext = match format.as_str() {
            "jpeg" | "jpg" => "jpg",
            "webp" => "webp",
            _ => "png",
        };
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
            if BATCH_CANCEL.load(Ordering::SeqCst) {
                canceled = true;
                break;
            }
            let mut fname = pattern.replace("{index}", &(idx + 1).to_string());
            for (k, v) in row {
                fname = fname.replace(&format!("{{{}}}", k), v);
            }
            let safe_name: String = fname
                .chars()
                .map(|c| {
                    if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == ' ' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect();
            let final_name = if safe_name.to_lowercase().ends_with(&format!(".{}", ext)) {
                safe_name
            } else {
                format!("{}.{}", safe_name.trim_end_matches('.'), ext)
            };

            let _ = app.emit(
                "batch-progress",
                BatchProgressEvent {
                    done: idx,
                    total,
                    current_item: final_name.clone(),
                    finished: false,
                    canceled: false,
                    saved: saved.clone(),
                    errors: errors.clone(),
                },
            );

            let mut snap = apply_fields_to_snapshot(&tpl, row, None);
            let out_path = Path::new(&dest_dir).join(&final_name);
            match render_project_composite(&mut snap, "") {
                Ok(img) => match save_image_formatted(&img, &out_path, &format, quality) {
                    Ok(_) => saved.push(out_path.to_string_lossy().to_string()),
                    Err(e) => errors.push(format!("Row {} ({}): {}", idx + 1, final_name, e)),
                },
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
                done: saved.len() + errors.len(),
                total,
                current_item: if canceled { "Canceled".into() } else { "Done".into() },
                finished: true,
                canceled,
                saved,
                errors,
            },
        );
        Ok(summary)
    })
    .await
    .map_err(|e| format!("Batch task failed: {}", e))?
}
