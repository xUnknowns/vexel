use crate::layers::{render_project_composite, LayerKind, ProjectSnapshot};
use image::imageops::FilterType;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const CURRENT_SCHEMA_VERSION: u32 = 2;

pub fn user_data_root() -> PathBuf {
    let appdata = std::env::var("APPDATA")
        .unwrap_or_else(|_| r"C:\Users\Public\AppData\Roaming".to_string());
    Path::new(&appdata).join("Veltrax Studios")
}

pub fn ensure_subdir(name: &str) -> Result<PathBuf, String> {
    let dir = user_data_root().join(name);
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create '{}': {}", dir.display(), e))?;
    Ok(dir)
}

pub fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn now_unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

/// Write bytes atomically: write to sibling temp file, flush, then rename over `dest`.
pub fn atomic_write_bytes(dest: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create parent dir '{}': {}", parent.display(), e))?;
    }
    let tmp_name = format!(
        ".{}.tmp.{}.{}",
        dest.file_name().and_then(|n| n.to_str()).unwrap_or("file"),
        std::process::id(),
        now_unix_millis()
    );
    let tmp_path = dest.with_file_name(tmp_name);
    {
        let mut f = std::fs::File::create(&tmp_path)
            .map_err(|e| format!("Failed to create temp file '{}': {}", tmp_path.display(), e))?;
        f.write_all(bytes)
            .map_err(|e| format!("Failed to write temp file: {}", e))?;
        f.sync_all().ok();
    }
    if let Err(e) = std::fs::rename(&tmp_path, dest) {
        // Fallback on Windows if target exists and rename fails
        let _ = std::fs::remove_file(dest);
        if let Err(e2) = std::fs::rename(&tmp_path, dest) {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(format!("Atomic rename failed ({}; {}): {}", e, e2, dest.display()));
        }
    }
    Ok(())
}

pub fn atomic_write_str(dest: &Path, text: &str) -> Result<(), String> {
    atomic_write_bytes(dest, text.as_bytes())
}

/// Move a corrupt JSON file to `*.corrupt.<ts>` so it is never overwritten silently.
pub fn quarantine_corrupt_file(path: &Path) -> Option<String> {
    if !path.is_file() {
        return None;
    }
    let fname = path.file_name().and_then(|n| n.to_str()).unwrap_or("file.json");
    let corrupt_path = path.with_file_name(format!("{}.corrupt.{}", fname, now_unix_secs()));
    if std::fs::rename(path, &corrupt_path).is_ok() {
        Some(corrupt_path.to_string_lossy().to_string())
    } else {
        None
    }
}

/// Read and deserialize a JSON file; if corrupt, quarantine it to `*.corrupt` and return `Ok((default, Some(warning)))`.
pub fn load_json_or_quarantine<T: for<'de> Deserialize<'de> + Default>(
    path: &Path,
    label: &str,
) -> (T, Option<String>) {
    if !path.is_file() {
        return (T::default(), None);
    }
    match std::fs::read_to_string(path) {
        Ok(raw) => match serde_json::from_str::<T>(&raw) {
            Ok(val) => (val, None),
            Err(e) => {
                let moved = quarantine_corrupt_file(path);
                let msg = match moved {
                    Some(cp) => format!(
                        "{} estaba corrupto ({}) y se movió a {} para no perder datos.",
                        label, e, cp
                    ),
                    None => format!("{} estaba corrupto ({}) y se usaron valores por defecto.", label, e),
                };
                (T::default(), Some(msg))
            }
        },
        Err(e) => (
            T::default(),
            Some(format!("No se pudo leer {} ({}); usando valores por defecto.", label, e)),
        ),
    }
}

// ── App Settings (settings.json) ────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub window_width: u32,
    pub window_height: u32,
    pub last_export_format: String,
    pub last_export_dir: String,
    pub recent_presets: Vec<String>,
    #[serde(default = "default_true")]
    pub show_splash: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub corrupt_warning: Option<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            window_width: 1280,
            window_height: 800,
            last_export_format: "png".to_string(),
            last_export_dir: String::new(),
            recent_presets: Vec::new(),
            show_splash: true,
            corrupt_warning: None,
        }
    }
}

#[tauri::command]
pub fn get_app_settings() -> Result<AppSettings, String> {
    let root = user_data_root();
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let path = root.join("settings.json");
    let (mut settings, warn) = load_json_or_quarantine::<AppSettings>(&path, "settings.json");
    settings.corrupt_warning = warn;
    Ok(settings)
}

#[tauri::command]
pub fn save_app_settings(mut settings: AppSettings) -> Result<AppSettings, String> {
    settings.corrupt_warning = None;
    let root = user_data_root();
    let path = root.join("settings.json");
    let json = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    atomic_write_str(&path, &json)?;
    Ok(settings)
}

// ── Persistent Recents (recent.json + thumbnails/) ──────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecentProjectEntry {
    pub path: String,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub updated_at: u64,
    #[serde(default)]
    pub thumbnail_path: Option<String>,
    #[serde(default = "default_true")]
    pub exists: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RecentStore {
    pub items: Vec<RecentProjectEntry>,
}

fn sanitize_slug(s: &str) -> String {
    let clean: String = s
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let trimmed = clean.trim_matches('_');
    if trimmed.is_empty() {
        "project".to_string()
    } else {
        trimmed.chars().take(48).collect()
    }
}

pub fn project_assets_dir(project_id: &str) -> Result<PathBuf, String> {
    let dir = ensure_subdir("projects")?.join(format!("{}.assets", sanitize_slug(project_id)));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// Copy an external image into `projects/<project_id>.assets/` so moving/deleting the original doesn't break the project.
pub fn copy_asset_into_project(project_id: &str, source_path: &str) -> Result<String, String> {
    let src = Path::new(source_path);
    if !src.is_file() {
        return Err(format!("Source image '{}' does not exist.", source_path));
    }
    let assets_dir = project_assets_dir(project_id)?;
    // Already inside this project's assets dir?
    if let (Ok(canon_src), Ok(canon_dir)) = (std::fs::canonicalize(src), std::fs::canonicalize(&assets_dir)) {
        if canon_src.starts_with(&canon_dir) {
            return Ok(source_path.to_string());
        }
    }
    let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("png");
    let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or("asset");
    let dest = assets_dir.join(format!("{}_{}.{}", sanitize_slug(stem), now_unix_millis(), ext));
    std::fs::copy(src, &dest)
        .map_err(|e| format!("Failed to copy asset '{}' into project: {}", source_path, e))?;
    Ok(dest.to_string_lossy().to_string())
}

/// Migrate any external LayerKind::Image paths in a loaded snapshot into `projects/<project_id>.assets/`.
pub fn migrate_snapshot_assets(snap: &mut ProjectSnapshot) -> bool {
    if snap.project_id.trim().is_empty() {
        snap.project_id = format!("proj_{}", now_unix_millis());
    }
    let mut changed = false;
    for layer in &mut snap.layers {
        if let LayerKind::Image { source_path, .. } = &mut layer.kind {
            if Path::new(source_path.as_str()).is_file() {
                if let Ok(copied) = copy_asset_into_project(&snap.project_id, source_path) {
                    if copied != *source_path {
                        *source_path = copied;
                        changed = true;
                    }
                }
            }
        }
    }
    changed
}

pub fn save_project_thumbnail(project_key: &str, snap: &ProjectSnapshot) -> Option<String> {
    let thumbs_dir = ensure_subdir("thumbnails").ok()?;
    let tmp = std::env::temp_dir().join(format!("veltrax-thumb-{}.png", now_unix_millis()));
    let mut clone = snap.clone();
    if render_project_composite(&mut clone, tmp.to_str()?).is_err() {
        return None;
    }
    let img = image::open(&tmp).ok()?;
    let _ = std::fs::remove_file(&tmp);
    let (w, h) = (img.width().max(1), img.height().max(1));
    let scale = (320.0 / w.max(h) as f32).min(1.0);
    let nw = ((w as f32 * scale) as u32).max(1);
    let nh = ((h as f32 * scale) as u32).max(1);
    let resized = img.resize_exact(nw, nh, FilterType::Triangle);
    let dest = thumbs_dir.join(format!("{}.png", sanitize_slug(project_key)));
    let mut buf = Vec::new();
    resized
        .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
        .ok()?;
    atomic_write_bytes(&dest, &buf).ok()?;
    Some(dest.to_string_lossy().to_string())
}

pub fn record_recent_project(
    file_path: &str,
    snap: &ProjectSnapshot,
    thumb_path: Option<String>,
) -> Result<(), String> {
    let root = user_data_root();
    let recent_path = root.join("recent.json");
    let (mut store, _) = load_json_or_quarantine::<RecentStore>(&recent_path, "recent.json");
    let norm = file_path.replace('\\', "/").to_lowercase();
    store
        .items
        .retain(|r| r.path.replace('\\', "/").to_lowercase() != norm);
    let existing_thumb = thumb_path.or_else(|| {
        let key = if snap.project_id.is_empty() {
            &snap.project_name
        } else {
            &snap.project_id
        };
        save_project_thumbnail(key, snap)
    });
    store.items.insert(
        0,
        RecentProjectEntry {
            path: file_path.to_string(),
            name: snap.project_name.clone(),
            width: snap.canvas_width,
            height: snap.canvas_height,
            updated_at: now_unix_secs(),
            thumbnail_path: existing_thumb,
            exists: Path::new(file_path).is_file(),
        },
    );
    store.items.truncate(20);
    let json = serde_json::to_string_pretty(&store).map_err(|e| e.to_string())?;
    atomic_write_str(&recent_path, &json)
}

#[tauri::command]
pub fn get_recent_projects() -> Result<(Vec<RecentProjectEntry>, Option<String>), String> {
    let root = user_data_root();
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let recent_path = root.join("recent.json");
    let (mut store, warn) = load_json_or_quarantine::<RecentStore>(&recent_path, "recent.json");
    for item in &mut store.items {
        item.exists = Path::new(&item.path).is_file();
        if let Some(ref tp) = item.thumbnail_path {
            if !Path::new(tp).is_file() {
                item.thumbnail_path = None;
            }
        }
    }
    Ok((store.items, warn))
}

#[tauri::command]
pub fn remove_recent_project(path: String) -> Result<(), String> {
    let recent_path = user_data_root().join("recent.json");
    let (mut store, _) = load_json_or_quarantine::<RecentStore>(&recent_path, "recent.json");
    let norm = path.replace('\\', "/").to_lowercase();
    store
        .items
        .retain(|r| r.path.replace('\\', "/").to_lowercase() != norm);
    let json = serde_json::to_string_pretty(&store).map_err(|e| e.to_string())?;
    atomic_write_str(&recent_path, &json)
}

#[tauri::command]
pub fn relink_recent_project(old_path: String, new_path: String) -> Result<(), String> {
    if !Path::new(&new_path).is_file() {
        return Err(format!("Selected file '{}' does not exist.", new_path));
    }
    let recent_path = user_data_root().join("recent.json");
    let (mut store, _) = load_json_or_quarantine::<RecentStore>(&recent_path, "recent.json");
    let norm_old = old_path.replace('\\', "/").to_lowercase();
    for item in &mut store.items {
        if item.path.replace('\\', "/").to_lowercase() == norm_old {
            item.path = new_path.clone();
            item.exists = true;
            item.updated_at = now_unix_secs();
        }
    }
    let json = serde_json::to_string_pretty(&store).map_err(|e| e.to_string())?;
    atomic_write_str(&recent_path, &json)
}

// ── Schema Versioning & Migrations ──────────────────────────────────────────

pub fn backup_project_file(source_path: &Path, from_version: u32) -> Result<String, String> {
    if !source_path.is_file() {
        return Ok(String::new());
    }
    let backups_dir = ensure_subdir("backups")?;
    let stem = source_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("project");
    let dest = backups_dir.join(format!(
        "{}_v{}_{}.veltrax.bak",
        sanitize_slug(stem),
        from_version,
        now_unix_secs()
    ));
    std::fs::copy(source_path, &dest)
        .map_err(|e| format!("Failed to backup before migration: {}", e))?;

    // Keep only the latest 15 backups
    if let Ok(entries) = std::fs::read_dir(&backups_dir) {
        let mut files: Vec<_> = entries
            .flatten()
            .filter(|e| e.path().is_file())
            .filter_map(|e| {
                let modified = e.metadata().ok()?.modified().ok()?;
                Some((modified, e.path()))
            })
            .collect();
        files.sort_by_key(|(t, _)| *t);
        if files.len() > 15 {
            for (_, p) in &files[..files.len() - 15] {
                let _ = std::fs::remove_file(p);
            }
        }
    }
    Ok(dest.to_string_lossy().to_string())
}

pub fn migrate_snapshot_if_needed(
    snap: &mut ProjectSnapshot,
    source_path: Option<&Path>,
) -> Result<Option<String>, String> {
    if snap.schema_version > CURRENT_SCHEMA_VERSION {
        return Err(format!(
            "Este proyecto fue creado con una versión más nueva de Veltrax Studios (esquema v{}, esta app soporta hasta v{}). Actualiza la app antes de abrirlo para no perder datos.",
            snap.schema_version, CURRENT_SCHEMA_VERSION
        ));
    }
    if snap.schema_version == CURRENT_SCHEMA_VERSION {
        return Ok(None);
    }
    let old_ver = snap.schema_version;
    let mut backup_msg = None;
    if let Some(p) = source_path {
        if let Ok(bpath) = backup_project_file(p, old_ver) {
            if !bpath.is_empty() {
                backup_msg = Some(format!(
                    "Proyecto migrado de v{} a v{}. Respaldo guardado en {}",
                    old_ver, CURRENT_SCHEMA_VERSION, bpath
                ));
            }
        }
    }
    // Step-by-step migrations
    if snap.schema_version < 2 {
        if snap.project_id.trim().is_empty() {
            snap.project_id = format!("proj_{}", now_unix_millis());
        }
        snap.schema_version = 2;
    }
    Ok(backup_msg)
}

// ── Session Lock & Autosave Recovery ────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AutosaveMeta {
    pub project_name: String,
    pub project_file_path: Option<String>,
    pub saved_at: u64,
    pub unexpected_exit: bool,
}

pub fn write_session_lock() {
    if let Ok(dir) = ensure_subdir("autosave") {
        let lock = dir.join("session.lock");
        let _ = atomic_write_str(&lock, &now_unix_secs().to_string());
    }
}

pub fn clear_session_lock() {
    if let Ok(dir) = ensure_subdir("autosave") {
        let lock = dir.join("session.lock");
        if lock.is_file() {
            let _ = std::fs::remove_file(&lock);
        }
    }
}

// ── Manual ZIP Backup Export & Import ───────────────────────────────────────

fn add_dir_to_zip<W: Write + std::io::Seek>(
    zip: &mut zip::ZipWriter<W>,
    base_dir: &Path,
    prefix: &str,
    options: zip::write::SimpleFileOptions,
) -> Result<(), String> {
    if !base_dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(base_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let rel = if prefix.is_empty() {
            name
        } else {
            format!("{}/{}", prefix, name)
        };
        if path.is_dir() {
            add_dir_to_zip(zip, &path, &rel, options)?;
        } else if path.is_file() {
            zip.start_file(&rel, options).map_err(|e| e.to_string())?;
            let mut f = std::fs::File::open(&path).map_err(|e| e.to_string())?;
            let mut buf = Vec::new();
            f.read_to_end(&mut buf).map_err(|e| e.to_string())?;
            zip.write_all(&buf).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn export_user_data_zip(dest_zip: String) -> Result<String, String> {
    let root = user_data_root();
    let dest = Path::new(&dest_zip);
    let tmp = dest.with_file_name(format!(".veltrax-backup-{}.zip.tmp", now_unix_millis()));
    {
        let file = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        for sub in ["projects", "library", "templates", "fonts", "thumbnails"] {
            add_dir_to_zip(&mut zip, &root.join(sub), sub, options)?;
        }
        for json_file in ["brand_kit.json", "settings.json", "recent.json"] {
            let p = root.join(json_file);
            if p.is_file() {
                zip.start_file(json_file, options).map_err(|e| e.to_string())?;
                let bytes = std::fs::read(&p).map_err(|e| e.to_string())?;
                zip.write_all(&bytes).map_err(|e| e.to_string())?;
            }
        }
        zip.finish().map_err(|e| e.to_string())?;
    }
    if let Err(e) = std::fs::rename(&tmp, dest) {
        let _ = std::fs::remove_file(dest);
        std::fs::rename(&tmp, dest).map_err(|e2| format!("Failed to save ZIP ({}; {})", e, e2))?;
    }
    Ok(dest_zip)
}

#[tauri::command]
pub fn import_user_data_zip(src_zip: String) -> Result<usize, String> {
    let root = user_data_root();
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let file = std::fs::File::open(&src_zip).map_err(|e| format!("Cannot open ZIP: {}", e))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("Invalid ZIP: {}", e))?;

    let mut count = 0usize;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        if entry.is_dir() {
            continue;
        }
        let Some(enclosed) = entry.enclosed_name().map(|p| p.to_path_buf()) else {
            continue;
        };
        let out_path = root.join(enclosed);
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        atomic_write_bytes(&out_path, &buf)?;
        count += 1;
    }
    Ok(count)
}
