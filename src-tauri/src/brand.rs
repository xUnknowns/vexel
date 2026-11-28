use crate::storage::{
    atomic_write_str, ensure_subdir, load_json_or_quarantine, now_unix_millis, user_data_root,
};
use crate::tools::BrandKit;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BrandColor {
    pub name: String,
    pub hex: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BrandLogo {
    pub name: String,
    pub path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct BrandKitProfile {
    pub id: String,
    pub name: String,
    pub colors: Vec<BrandColor>,
    pub fonts: Vec<String>,
    pub logos: Vec<BrandLogo>,
}

impl Default for BrandKitProfile {
    fn default() -> Self {
        Self {
            id: "default".to_string(),
            name: "Default Brand".to_string(),
            colors: vec![
                BrandColor { name: "Primary".into(), hex: "#6366F1".into() },
                BrandColor { name: "Accent".into(), hex: "#38BDF8".into() },
                BrandColor { name: "Highlight".into(), hex: "#EC4899".into() },
                BrandColor { name: "Dark".into(), hex: "#0F172A".into() },
            ],
            fonts: vec!["bold_display".into(), "modern_sans".into()],
            logos: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct BrandKitStore {
    pub active_id: String,
    pub kits: Vec<BrandKitProfile>,
}

impl Default for BrandKitStore {
    fn default() -> Self {
        Self {
            active_id: "default".to_string(),
            kits: vec![BrandKitProfile::default()],
        }
    }
}

pub fn load_brand_store() -> BrandKitStore {
    let root = user_data_root();
    let store_path = root.join("brand_kits.json");
    if store_path.is_file() {
        let (mut store, _) =
            load_json_or_quarantine::<BrandKitStore>(&store_path, "brand_kits.json");
        if store.kits.is_empty() {
            store.kits.push(BrandKitProfile::default());
        }
        return store;
    }
    // Migrate legacy brand_kit.json colors if present
    let mut store = BrandKitStore::default();
    let legacy = root.join("brand_kit.json");
    if legacy.is_file() {
        let (old, _) = load_json_or_quarantine::<BrandKit>(&legacy, "brand_kit.json");
        if !old.colors.is_empty() {
            store.kits[0].colors = old
                .colors
                .into_iter()
                .enumerate()
                .map(|(i, hex)| BrandColor {
                    name: format!("Color {}", i + 1),
                    hex,
                })
                .collect();
        }
    }
    let _ = save_brand_store(&store);
    store
}

pub fn save_brand_store(store: &BrandKitStore) -> Result<(), String> {
    let root = user_data_root();
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(store).map_err(|e| e.to_string())?;
    atomic_write_str(&root.join("brand_kits.json"), &json)
}

#[tauri::command]
pub fn list_brand_kits() -> Result<BrandKitStore, String> {
    Ok(load_brand_store())
}

#[tauri::command]
pub fn save_brand_kit_profile(profile: BrandKitProfile) -> Result<BrandKitStore, String> {
    let mut store = load_brand_store();
    if let Some(existing) = store.kits.iter_mut().find(|k| k.id == profile.id) {
        *existing = profile;
    } else {
        let mut p = profile;
        if p.id.trim().is_empty() {
            p.id = format!("kit_{}", now_unix_millis());
        }
        store.active_id = p.id.clone();
        store.kits.push(p);
    }
    save_brand_store(&store)?;
    Ok(store)
}

#[tauri::command]
pub fn delete_brand_kit_profile(kit_id: String) -> Result<BrandKitStore, String> {
    let mut store = load_brand_store();
    if store.kits.len() <= 1 {
        return Err("At least one Brand Kit must remain.".to_string());
    }
    store.kits.retain(|k| k.id != kit_id);
    if store.active_id == kit_id {
        store.active_id = store.kits[0].id.clone();
    }
    save_brand_store(&store)?;
    Ok(store)
}

#[tauri::command]
pub fn set_active_brand_kit(kit_id: String) -> Result<BrandKitStore, String> {
    let mut store = load_brand_store();
    if !store.kits.iter().any(|k| k.id == kit_id) {
        return Err(format!("Brand Kit '{}' not found.", kit_id));
    }
    store.active_id = kit_id;
    save_brand_store(&store)?;
    Ok(store)
}

#[tauri::command]
pub fn add_brand_logo(kit_id: String, source_path: String) -> Result<BrandKitStore, String> {
    let src = Path::new(&source_path);
    if !src.is_file() {
        return Err(format!("Logo file '{}' does not exist.", source_path));
    }
    let dir = ensure_subdir("brand_logos")?;
    let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("png");
    let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or("logo");
    let dest = dir.join(format!("logo_{}.{}", now_unix_millis(), ext));
    std::fs::copy(src, &dest).map_err(|e| format!("Failed to copy logo: {}", e))?;

    let mut store = load_brand_store();
    let kit = store
        .kits
        .iter_mut()
        .find(|k| k.id == kit_id)
        .ok_or_else(|| format!("Brand Kit '{}' not found.", kit_id))?;
    kit.logos.push(BrandLogo {
        name: stem.to_string(),
        path: dest.to_string_lossy().to_string(),
    });
    save_brand_store(&store)?;
    Ok(store)
}
