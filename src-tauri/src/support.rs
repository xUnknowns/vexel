use crate::layers::{ProjectSnapshot, SharedProjectState};
use crate::storage::{self, AutosaveMeta};
use std::fs;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

/// Placeholder intentionally easy to replace with the project's feedback URL.
pub const FEEDBACK_URL: &str = "https://example.com/veltrax-feedback";
const MAX_LOG_BYTES: u64 = 1024 * 1024;
const LOG_BACKUPS: usize = 3;

static LOG_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
static RECOVERY_CACHE: OnceLock<Mutex<Option<CachedRecovery>>> = OnceLock::new();

#[derive(Clone)]
struct CachedRecovery {
    snapshot: ProjectSnapshot,
    project_file_path: Option<String>,
}

pub fn append_app_log(level: &str, message: &str) {
    let lock = LOG_LOCK.get_or_init(|| Mutex::new(()));
    let _guard = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let Ok(log_dir) = storage::ensure_subdir("logs") else {
        return;
    };
    let log_path = log_dir.join("app.log");
    rotate_logs_if_needed(&log_dir, &log_path);

    let one_line = message.replace('\r', " ").replace('\n', " ");
    let one_line: String = one_line.chars().take(4000).collect();
    let mut content = match fs::read(&log_path) {
        Ok(existing) => existing,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(_) => return,
    };
    content.extend_from_slice(
        format!("{} [{}] {}\n", storage::now_unix_secs(), level, one_line).as_bytes(),
    );
    let _ = storage::atomic_write_bytes(&log_path, &content);
}

fn rotate_logs_if_needed(log_dir: &Path, active_log: &Path) {
    if fs::metadata(active_log)
        .map(|metadata| metadata.len() < MAX_LOG_BYTES)
        .unwrap_or(true)
    {
        return;
    }

    let oldest = log_dir.join(format!("app.log.{}", LOG_BACKUPS));
    let _ = fs::remove_file(oldest);
    for index in (1..LOG_BACKUPS).rev() {
        let source = log_dir.join(format!("app.log.{}", index));
        let destination = log_dir.join(format!("app.log.{}", index + 1));
        if source.is_file() {
            let _ = fs::rename(source, destination);
        }
    }
    let first_backup = log_dir.join("app.log.1");
    let _ = fs::rename(active_log, first_backup);
}

pub fn cache_recovery_snapshot(
    snapshot: &ProjectSnapshot,
    project_file_path: Option<String>,
    has_project: bool,
) {
    let cache = RECOVERY_CACHE.get_or_init(|| Mutex::new(None));
    if let Ok(mut slot) = cache.try_lock() {
        *slot = if has_project {
            Some(CachedRecovery {
                snapshot: snapshot.clone(),
                project_file_path,
            })
        } else {
            None
        };
    }
}

fn latest_recovery_snapshot(state: &SharedProjectState) -> Option<CachedRecovery> {
    match state.try_lock() {
        Ok(project) if project.has_project => Some(CachedRecovery {
            snapshot: project.current.clone(),
            project_file_path: project.project_file_path.clone(),
        }),
        Ok(_) => None,
        Err(std::sync::TryLockError::WouldBlock) => RECOVERY_CACHE
            .get()
            .and_then(|cache| cache.try_lock().ok().and_then(|slot| slot.clone())),
        Err(std::sync::TryLockError::Poisoned(poisoned)) => {
            let project = poisoned.into_inner();
            if project.has_project {
                Some(CachedRecovery {
                    snapshot: project.current.clone(),
                    project_file_path: project.project_file_path.clone(),
                })
            } else {
                None
            }
        }
    }
}

fn persist_panic_recovery(recovery: &CachedRecovery) -> Result<(), String> {
    let autosave_dir = storage::ensure_subdir("autosave")?;
    let project_json = serde_json::to_string(&recovery.snapshot).map_err(|e| e.to_string())?;
    storage::atomic_write_str(&autosave_dir.join("autosave.veltrax"), &project_json)?;

    let meta = AutosaveMeta {
        project_name: recovery.snapshot.project_name.clone(),
        project_file_path: recovery.project_file_path.clone(),
        saved_at: storage::now_unix_secs(),
        unexpected_exit: true,
    };
    let meta_json = serde_json::to_string(&meta).map_err(|e| e.to_string())?;
    storage::atomic_write_str(&autosave_dir.join("autosave_meta.json"), &meta_json)?;
    storage::write_session_lock();
    Ok(())
}

pub fn install_global_panic_hook(state: SharedProjectState) {
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let payload = panic_info
            .payload()
            .downcast_ref::<&str>()
            .map(|text| (*text).to_string())
            .or_else(|| panic_info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "non-string panic payload".to_string());
        let location = panic_info
            .location()
            .map(|location| {
                let file = Path::new(location.file())
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("unknown");
                format!("{}:{}", file, location.line())
            })
            .unwrap_or_else(|| "unknown location".to_string());
        append_app_log("PANIC", &format!("{}: {}", location, payload));

        if let Some(recovery) = latest_recovery_snapshot(&state) {
            if let Err(error) = persist_panic_recovery(&recovery) {
                append_app_log(
                    "ERROR",
                    &format!("Panic recovery snapshot could not be saved: {}", error),
                );
            } else {
                append_app_log("INFO", "Panic recovery snapshot saved locally.");
            }
        } else {
            append_app_log(
                "ERROR",
                "Panic occurred while project state was unavailable; the most recent autosave was left untouched.",
            );
        }

        previous_hook(panic_info);
    }));
}

fn redact_diagnostic_text(line: &str) -> String {
    let timestamp = line.split_whitespace().next().unwrap_or("?");
    if line.contains("[PANIC]") {
        format!(
            "{} [PANIC] Pánico local; detalles omitidos por privacidad.",
            timestamp
        )
    } else if line.contains("Panic recovery snapshot could not be saved") {
        format!(
            "{} [ERROR] No se pudo guardar el estado recuperable local.",
            timestamp
        )
    } else {
        format!(
            "{} [ERROR] Error local registrado; detalles omitidos por privacidad.",
            timestamp
        )
    }
}

fn recent_error_lines() -> Vec<String> {
    let Ok(log_dir) = storage::ensure_subdir("logs") else {
        return Vec::new();
    };
    let lock = LOG_LOCK.get_or_init(|| Mutex::new(()));
    let _guard = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

    let mut all_lines = Vec::new();
    for suffix in (1..=LOG_BACKUPS).rev() {
        let path = log_dir.join(format!("app.log.{}", suffix));
        if let Ok(raw) = fs::read_to_string(path) {
            all_lines.extend(raw.lines().map(str::to_string));
        }
    }
    if let Ok(raw) = fs::read_to_string(log_dir.join("app.log")) {
        all_lines.extend(raw.lines().map(str::to_string));
    }

    all_lines
        .iter()
        .rev()
        .filter(|line| line.contains("[ERROR]") || line.contains("[PANIC]"))
        .take(5)
        .map(|line| redact_diagnostic_text(line))
        .collect()
}

#[tauri::command]
pub fn get_feedback_diagnostics(screen_width: Option<u32>, screen_height: Option<u32>) -> String {
    let os_name = if cfg!(target_os = "windows") {
        "Windows"
    } else {
        std::env::consts::OS
    };
    let resolution = match (screen_width, screen_height) {
        (Some(width), Some(height)) if width > 0 && height > 0 => {
            format!("{} × {} px", width, height)
        }
        _ => "no disponible".to_string(),
    };
    let mut output = format!(
        "Veltrax Studios v{} (Beta)\nSistema: {}\nResolución de pantalla: {}\nErrores locales recientes:",
        env!("CARGO_PKG_VERSION"),
        os_name,
        resolution
    );
    let errors = recent_error_lines();
    if errors.is_empty() {
        output.push_str("\n- Sin errores registrados.");
    } else {
        for error in errors {
            output.push_str("\n- ");
            output.push_str(&error);
        }
    }
    output
}

#[tauri::command]
pub fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
pub fn record_local_error(source: String, error_message: String) -> Result<(), String> {
    if !["webview", "unhandled_promise", "frontend"].contains(&source.as_str()) {
        return Err("Unsupported local error source.".to_string());
    }
    let message: String = error_message.chars().take(2000).collect();
    append_app_log("ERROR", &format!("{}: {}", source, message));
    Ok(())
}

#[tauri::command]
pub fn open_feedback_site() -> Result<String, String> {
    if !FEEDBACK_URL.starts_with("https://") || FEEDBACK_URL.chars().any(char::is_whitespace) {
        return Err("The configured feedback URL must be a valid HTTPS URL.".to_string());
    }

    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("rundll32.exe")
        .args(["url.dll,FileProtocolHandler", FEEDBACK_URL])
        .spawn();
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(FEEDBACK_URL).spawn();
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    let result = std::process::Command::new("xdg-open")
        .arg(FEEDBACK_URL)
        .spawn();

    result
        .map(|_| {
            append_app_log("INFO", "User opened the configured feedback site.");
            "Se abrió el sitio de feedback; no se envió ningún diagnóstico.".to_string()
        })
        .map_err(|error| format!("Could not open the configured feedback site: {}", error))
}

#[cfg(test)]
mod tests {
    use super::{redact_diagnostic_text, rotate_logs_if_needed, LOG_BACKUPS, MAX_LOG_BYTES};
    use crate::storage;
    use std::fs;

    #[test]
    fn diagnostic_redaction_omits_arbitrary_private_error_text() {
        let line = r"123 [PANIC] C:\Users\Alice\project.veltrax /home/alice/design.veltrax mail alice@example.com api_key=abc123";
        let safe = redact_diagnostic_text(line);
        assert!(!safe.contains("C:\\Users\\Alice"));
        assert!(!safe.contains("/home/alice"));
        assert!(!safe.contains("alice@example.com"));
        assert!(!safe.contains("abc123"));
        assert!(safe.contains("detalles omitidos por privacidad"));
    }

    #[test]
    fn log_rotation_keeps_only_the_three_newest_backups() {
        let log_dir = std::env::temp_dir().join(format!(
            "veltrax-log-rotation-{}-{}",
            std::process::id(),
            storage::now_unix_millis()
        ));
        fs::create_dir_all(&log_dir).unwrap();
        let active = log_dir.join("app.log");
        fs::write(&active, vec![b'x'; MAX_LOG_BYTES as usize]).unwrap();
        for index in 1..=LOG_BACKUPS {
            fs::write(
                log_dir.join(format!("app.log.{}", index)),
                format!("backup{}", index),
            )
            .unwrap();
        }

        rotate_logs_if_needed(&log_dir, &active);

        assert!(!active.exists());
        assert_eq!(
            fs::read(log_dir.join("app.log.1")).unwrap().len(),
            MAX_LOG_BYTES as usize
        );
        assert_eq!(
            fs::read_to_string(log_dir.join("app.log.2")).unwrap(),
            "backup1"
        );
        assert_eq!(
            fs::read_to_string(log_dir.join("app.log.3")).unwrap(),
            "backup2"
        );
        let _ = fs::remove_dir_all(log_dir);
    }
}
