use crate::layers::{
    parse_hex_color, render_project_composite, BgGradient, DropShadow, GlowEffect, Layer,
    LayerEffects, LayerKind, LayerOutline, ProjectResponse, ProjectSnapshot, SharedProjectState,
};
use crate::storage::{
    atomic_write_bytes, atomic_write_str, copy_asset_into_project, ensure_subdir,
    now_unix_millis, record_recent_project, write_session_lock, CURRENT_SCHEMA_VERSION,
};
use crate::tools::render_and_respond;
use base64::Engine;
use image::imageops::FilterType;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TemplateField {
    pub id: String,
    pub label: String,
    pub kind: String, // "text" | "image" | "color"
    pub layer_id: String,
    #[serde(default)]
    pub variable_name: String,
    #[serde(default)]
    pub default_value: String,
    #[serde(default = "default_fit")]
    pub image_fit: String, // "cover" | "contain"
}

fn default_fit() -> String {
    "cover".to_string()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VTemplate {
    #[serde(default = "default_tpl_ver")]
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    #[serde(default = "default_cat")]
    pub category: String,
    #[serde(default)]
    pub builtin: bool,
    pub snapshot: ProjectSnapshot,
    #[serde(default)]
    pub fields: Vec<TemplateField>,
    #[serde(default)]
    pub embedded_assets: HashMap<String, String>, // layer_id -> base64 PNG/JPEG
}

fn default_tpl_ver() -> u32 {
    1
}
fn default_cat() -> String {
    "Custom".to_string()
}

#[derive(Clone, Debug, Serialize)]
pub struct TemplateSummary {
    pub id: String,
    pub name: String,
    pub category: String,
    pub builtin: bool,
    pub file_path: String,
    pub canvas_width: u32,
    pub canvas_height: u32,
    pub fields: Vec<TemplateField>,
    pub preview_data_url: Option<String>,
}

fn make_text_layer(
    id: &str,
    name: &str,
    content: &str,
    x: i64,
    y: i64,
    size: f32,
    color: [u8; 4],
    bold: bool,
) -> Layer {
    Layer {
        id: id.to_string(),
        name: name.to_string(),
        visible: true,
        locked: false,
        opacity: 1.0,
        x,
        y,
        scale: 1.0,
        rotation: 0.0,
        flip_h: false,
        flip_v: false,
        blend_mode: "normal".to_string(),
        bounds_width: 600,
        bounds_height: 100,
        group_id: None,
        effects: LayerEffects::default(),
        outline: LayerOutline::default(),
        shadow: DropShadow {
            enabled: true,
            color: [0, 0, 0, 180],
            blur: 8.0,
            distance: 6.0,
            angle: 135.0,
            opacity: 0.65,
        },
        outer_glow: GlowEffect::default(),
        inner_glow: GlowEffect::default(),
        clip_to_below: false,
        mask_shape: None,
        mask_radius: 0,
        alpha_mask_path: None,
        kind: LayerKind::Text3D {
            content: content.to_string(),
            font_style: if bold { "bold_display" } else { "modern_sans" }.to_string(),
            color,
            depth: 0,
            shadow_color: [0, 0, 0, 180],
            stroke_color: [0, 0, 0, 255],
            stroke_width: 0,
            size,
            bold,
            italic: false,
            align: "left".to_string(),
            line_height: 1.15,
            letter_spacing: 0.0,
            underline: false,
            gradient_colors: None,
            gradient_angle: 0.0,
            curve_amount: 0.0,
        },
    }
}

fn make_shape_layer(
    id: &str,
    name: &str,
    shape_type: &str,
    x: i64,
    y: i64,
    width: u32,
    height: u32,
    fill_color: [u8; 4],
    border_radius: u32,
) -> Layer {
    Layer {
        id: id.to_string(),
        name: name.to_string(),
        visible: true,
        locked: false,
        opacity: 1.0,
        x,
        y,
        scale: 1.0,
        rotation: 0.0,
        flip_h: false,
        flip_v: false,
        blend_mode: "normal".to_string(),
        bounds_width: width,
        bounds_height: height,
        group_id: None,
        effects: LayerEffects::default(),
        outline: LayerOutline::default(),
        shadow: DropShadow::default(),
        outer_glow: GlowEffect::default(),
        inner_glow: GlowEffect::default(),
        clip_to_below: false,
        mask_shape: None,
        mask_radius: 0,
        alpha_mask_path: None,
        kind: LayerKind::Shape {
            shape_type: shape_type.to_string(),
            width,
            height,
            fill_color,
            border_radius,
            border_color: [255, 255, 255, 255],
            border_width: 0,
            sides: 5,
            corner_radii: None,
            fill_gradient: None,
        },
    }
}

pub fn builtin_templates() -> Vec<VTemplate> {
    vec![
        // 1. YouTube Thumbnail (1280x720)
        VTemplate {
            schema_version: 1,
            id: "builtin_youtube_thumb".to_string(),
            name: "YouTube Thumbnail".to_string(),
            category: "YouTube".to_string(),
            builtin: true,
            snapshot: ProjectSnapshot {
                schema_version: CURRENT_SCHEMA_VERSION,
                project_id: "tpl_yt".to_string(),
                project_name: "YouTube Thumbnail".to_string(),
                canvas_width: 1280,
                canvas_height: 720,
                transparent_bg: false,
                bg_color: [15, 23, 42, 255],
                bg_gradient: Some(BgGradient {
                    from_color: [15, 23, 42, 255],
                    to_color: [49, 46, 129, 255],
                    angle: 135.0,
                }),
                layers: vec![
                    make_shape_layer("yt_accent", "Accent Card", "rectangle", 60, 80, 18, 560, [99, 102, 241, 255], 9),
                    make_text_layer("yt_badge", "Badge", "{{tag}}", 110, 110, 34.0, [56, 189, 248, 255], true),
                    make_text_layer("yt_title", "Headline", "{{title}}", 110, 185, 92.0, [255, 255, 255, 255], true),
                    make_text_layer("yt_sub", "Subtitle", "{{subtitle}}", 110, 440, 42.0, [203, 213, 225, 255], false),
                ],
                groups: Vec::new(),
            },
            fields: vec![
                TemplateField { id: "f1".into(), label: "Tag / Badge".into(), kind: "text".into(), layer_id: "yt_badge".into(), variable_name: "tag".into(), default_value: "NEW VIDEO".into(), image_fit: "cover".into() },
                TemplateField { id: "f2".into(), label: "Main Title".into(), kind: "text".into(), layer_id: "yt_title".into(), variable_name: "title".into(), default_value: "HOW TO BUILD\nFAST APPS".into(), image_fit: "cover".into() },
                TemplateField { id: "f3".into(), label: "Subtitle".into(), kind: "text".into(), layer_id: "yt_sub".into(), variable_name: "subtitle".into(), default_value: "Step-by-step tutorial".into(), image_fit: "cover".into() },
                TemplateField { id: "f4".into(), label: "Accent Color".into(), kind: "color".into(), layer_id: "yt_accent".into(), variable_name: "accent".into(), default_value: "#6366F1".into(), image_fit: "cover".into() },
            ],
            embedded_assets: HashMap::new(),
        },
        // 2. Discord Banner (960x540)
        VTemplate {
            schema_version: 1,
            id: "builtin_discord_banner".to_string(),
            name: "Discord Banner".to_string(),
            category: "Gaming".to_string(),
            builtin: true,
            snapshot: ProjectSnapshot {
                schema_version: CURRENT_SCHEMA_VERSION,
                project_id: "tpl_discord".to_string(),
                project_name: "Discord Banner".to_string(),
                canvas_width: 960,
                canvas_height: 540,
                transparent_bg: false,
                bg_color: [24, 24, 37, 255],
                bg_gradient: Some(BgGradient {
                    from_color: [88, 101, 242, 255],
                    to_color: [17, 24, 39, 255],
                    angle: 120.0,
                }),
                layers: vec![
                    make_shape_layer("dc_box", "Card", "rectangle", 56, 56, 848, 428, [15, 23, 42, 200], 24),
                    make_text_layer("dc_title", "Server Name", "{{server}}", 110, 150, 74.0, [255, 255, 255, 255], true),
                    make_text_layer("dc_sub", "Tagline", "{{tagline}}", 110, 270, 34.0, [165, 180, 252, 255], false),
                ],
                groups: Vec::new(),
            },
            fields: vec![
                TemplateField { id: "f1".into(), label: "Server Name".into(), kind: "text".into(), layer_id: "dc_title".into(), variable_name: "server".into(), default_value: "CRAFTERS COMMUNITY".into(), image_fit: "cover".into() },
                TemplateField { id: "f2".into(), label: "Tagline".into(), kind: "text".into(), layer_id: "dc_sub".into(), variable_name: "tagline".into(), default_value: "Creators · Developers · Artists".into(), image_fit: "cover".into() },
                TemplateField { id: "f3".into(), label: "Card Color".into(), kind: "color".into(), layer_id: "dc_box".into(), variable_name: "card_color".into(), default_value: "#0F172A".into(), image_fit: "cover".into() },
            ],
            embedded_assets: HashMap::new(),
        },
        // 3. Simple Logo (1024x1024)
        VTemplate {
            schema_version: 1,
            id: "builtin_simple_logo".to_string(),
            name: "Simple Brand Logo".to_string(),
            category: "Logo".to_string(),
            builtin: true,
            snapshot: ProjectSnapshot {
                schema_version: CURRENT_SCHEMA_VERSION,
                project_id: "tpl_logo".to_string(),
                project_name: "Brand Logo".to_string(),
                canvas_width: 1024,
                canvas_height: 1024,
                transparent_bg: true,
                bg_color: [0, 0, 0, 0],
                bg_gradient: None,
                layers: vec![
                    make_shape_layer("lg_circle", "Emblem", "circle", 162, 112, 700, 700, [99, 102, 241, 255], 0),
                    make_text_layer("lg_mono", "Monogram", "{{monogram}}", 340, 300, 260.0, [255, 255, 255, 255], true),
                    make_text_layer("lg_name", "Brand Name", "{{brand}}", 240, 840, 72.0, [255, 255, 255, 255], true),
                ],
                groups: Vec::new(),
            },
            fields: vec![
                TemplateField { id: "f1".into(), label: "Monogram".into(), kind: "text".into(), layer_id: "lg_mono".into(), variable_name: "monogram".into(), default_value: "V".into(), image_fit: "cover".into() },
                TemplateField { id: "f2".into(), label: "Brand Name".into(), kind: "text".into(), layer_id: "lg_name".into(), variable_name: "brand".into(), default_value: "VELTRAX".into(), image_fit: "cover".into() },
                TemplateField { id: "f3".into(), label: "Emblem Color".into(), kind: "color".into(), layer_id: "lg_circle".into(), variable_name: "emblem_color".into(), default_value: "#6366F1".into(), image_fit: "cover".into() },
            ],
            embedded_assets: HashMap::new(),
        },
        // 4. Instagram Post (1080x1080)
        VTemplate {
            schema_version: 1,
            id: "builtin_ig_post".to_string(),
            name: "Instagram Quote Post".to_string(),
            category: "Social".to_string(),
            builtin: true,
            snapshot: ProjectSnapshot {
                schema_version: CURRENT_SCHEMA_VERSION,
                project_id: "tpl_ig".to_string(),
                project_name: "Instagram Post".to_string(),
                canvas_width: 1080,
                canvas_height: 1080,
                transparent_bg: false,
                bg_color: [17, 24, 39, 255],
                bg_gradient: Some(BgGradient {
                    from_color: [236, 72, 153, 255],
                    to_color: [99, 102, 241, 255],
                    angle: 45.0,
                }),
                layers: vec![
                    make_shape_layer("ig_card", "Card", "rectangle", 90, 90, 900, 900, [13, 13, 18, 235], 36),
                    make_text_layer("ig_quote", "Quote", "{{quote}}", 160, 280, 68.0, [255, 255, 255, 255], true),
                    make_text_layer("ig_handle", "Handle", "{{handle}}", 160, 820, 36.0, [165, 180, 252, 255], false),
                ],
                groups: Vec::new(),
            },
            fields: vec![
                TemplateField { id: "f1".into(), label: "Headline / Quote".into(), kind: "text".into(), layer_id: "ig_quote".into(), variable_name: "quote".into(), default_value: "Design fast.\nShip faster.".into(), image_fit: "cover".into() },
                TemplateField { id: "f2".into(), label: "Handle".into(), kind: "text".into(), layer_id: "ig_handle".into(), variable_name: "handle".into(), default_value: "@veltrax.studios".into(), image_fit: "cover".into() },
                TemplateField { id: "f3".into(), label: "Card Color".into(), kind: "color".into(), layer_id: "ig_card".into(), variable_name: "card_color".into(), default_value: "#0D0D12".into(), image_fit: "cover".into() },
            ],
            embedded_assets: HashMap::new(),
        },
        // 5. Twitch Panel (320x160)
        VTemplate {
            schema_version: 1,
            id: "builtin_twitch_panel".to_string(),
            name: "Twitch Panel".to_string(),
            category: "Gaming".to_string(),
            builtin: true,
            snapshot: ProjectSnapshot {
                schema_version: CURRENT_SCHEMA_VERSION,
                project_id: "tpl_twitch".to_string(),
                project_name: "Twitch Panel".to_string(),
                canvas_width: 320,
                canvas_height: 160,
                transparent_bg: true,
                bg_color: [0, 0, 0, 0],
                bg_gradient: None,
                layers: vec![
                    make_shape_layer("tw_bg", "Panel BG", "rectangle", 8, 8, 304, 144, [145, 70, 255, 255], 20),
                    make_text_layer("tw_title", "Panel Label", "{{label}}", 36, 48, 42.0, [255, 255, 255, 255], true),
                ],
                groups: Vec::new(),
            },
            fields: vec![
                TemplateField { id: "f1".into(), label: "Panel Label".into(), kind: "text".into(), layer_id: "tw_title".into(), variable_name: "label".into(), default_value: "ABOUT ME".into(), image_fit: "cover".into() },
                TemplateField { id: "f2".into(), label: "Panel Color".into(), kind: "color".into(), layer_id: "tw_bg".into(), variable_name: "panel_color".into(), default_value: "#9146FF".into(), image_fit: "cover".into() },
            ],
            embedded_assets: HashMap::new(),
        },
    ]
}

fn render_template_preview_data_url(snap: &ProjectSnapshot) -> Option<String> {
    let tmp = std::env::temp_dir().join(format!("veltrax-vtpl-{}.png", now_unix_millis()));
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
    let thumb = img.resize_exact(nw, nh, FilterType::Triangle);
    let mut buf = Vec::new();
    thumb
        .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
        .ok()?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&buf);
    Some(format!("data:image/png;base64,{}", b64))
}

pub fn load_all_vtemplates() -> Result<Vec<(VTemplate, String)>, String> {
    let mut list: Vec<(VTemplate, String)> = builtin_templates()
        .into_iter()
        .map(|t| (t, String::new()))
        .collect();

    let dir = ensure_subdir("templates")?;
    for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("vtemplate") {
            if let Ok(raw) = std::fs::read_to_string(&path) {
                if let Ok(mut tpl) = serde_json::from_str::<VTemplate>(&raw) {
                    tpl.builtin = false;
                    list.push((tpl, path.to_string_lossy().to_string()));
                }
            }
        }
    }
    Ok(list)
}

pub fn apply_fields_to_snapshot(
    tpl: &VTemplate,
    values: &HashMap<String, String>,
    copy_to_project_id: Option<&str>,
) -> ProjectSnapshot {
    let mut snap = tpl.snapshot.clone();
    // Apply field overrides
    for field in &tpl.fields {
        let val = values
            .get(&field.variable_name)
            .or_else(|| values.get(&field.id))
            .cloned()
            .unwrap_or_else(|| field.default_value.clone());

        if let Some(layer) = snap.layers.iter_mut().find(|l| l.id == field.layer_id) {
            match field.kind.as_str() {
                "text" => {
                    if let LayerKind::Text3D { content, .. } = &mut layer.kind {
                        if content.contains(&format!("{{{{{}}}}}", field.variable_name)) {
                            *content = content.replace(&format!("{{{{{}}}}}", field.variable_name), &val);
                        } else {
                            *content = val.clone();
                        }
                    }
                }
                "color" => {
                    let rgba = parse_hex_color(&val);
                    match &mut layer.kind {
                        LayerKind::Shape { fill_color, .. } => *fill_color = rgba,
                        LayerKind::Text3D { color, .. } => *color = rgba,
                        LayerKind::Gradient { from_color, .. } => *from_color = rgba,
                        _ => {}
                    }
                }
                "image" => {
                    if !val.trim().is_empty() && Path::new(&val).is_file() {
                        let stored = if let Some(pid) = copy_to_project_id {
                            copy_asset_into_project(pid, &val).unwrap_or_else(|_| val.clone())
                        } else {
                            val.clone()
                        };
                        if let Ok(img) = image::open(&stored) {
                            let (iw, ih) = (img.width().max(1) as f32, img.height().max(1) as f32);
                            let tw = layer.bounds_width.max(1) as f32;
                            let th = layer.bounds_height.max(1) as f32;
                            if field.image_fit == "contain" {
                                layer.scale = (tw / iw).min(th / ih);
                            } else {
                                layer.scale = (tw / iw).max(th / ih);
                            }
                        }
                        if let LayerKind::Image { source_path, .. } = &mut layer.kind {
                            *source_path = stored;
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // Also substitute any remaining {{var}} in all Text3D layers
    for layer in &mut snap.layers {
        if let LayerKind::Text3D { content, .. } = &mut layer.kind {
            for (k, v) in values {
                *content = content.replace(&format!("{{{{{}}}}}", k), v);
            }
            for field in &tpl.fields {
                if field.kind == "text" {
                    let fallback = values
                        .get(&field.variable_name)
                        .unwrap_or(&field.default_value);
                    *content = content.replace(&format!("{{{{{}}}}}", field.variable_name), fallback);
                }
            }
        }
    }
    snap
}

#[tauri::command]
pub fn list_all_templates() -> Result<Vec<TemplateSummary>, String> {
    let all = load_all_vtemplates()?;
    let thumbs_dir = ensure_subdir("thumbnails")?;
    let mut out = Vec::new();
    for (tpl, file_path) in all {
        let mut preview_snap = apply_fields_to_snapshot(&tpl, &HashMap::new(), None);
        let cache_png = thumbs_dir.join(format!("tpl_{}.png", tpl.id));
        let preview_data_url = if cache_png.is_file() {
            std::fs::read(&cache_png).ok().map(|b| {
                format!(
                    "data:image/png;base64,{}",
                    base64::engine::general_purpose::STANDARD.encode(&b)
                )
            })
        } else {
            let url = render_template_preview_data_url(&preview_snap);
            if let Some(ref u) = url {
                if let Some((_, b64)) = u.split_once(',') {
                    if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(b64) {
                        let _ = atomic_write_bytes(&cache_png, &bytes);
                    }
                }
            }
            url
        };
        let _ = &mut preview_snap;
        out.push(TemplateSummary {
            id: tpl.id,
            name: tpl.name,
            category: tpl.category,
            builtin: tpl.builtin,
            file_path,
            canvas_width: tpl.snapshot.canvas_width,
            canvas_height: tpl.snapshot.canvas_height,
            fields: tpl.fields,
            preview_data_url,
        });
    }
    Ok(out)
}

#[tauri::command]
pub fn save_current_as_vtemplate(
    state: tauri::State<'_, SharedProjectState>,
    name: String,
    category: String,
    fields: Vec<TemplateField>,
) -> Result<TemplateSummary, String> {
    if name.trim().is_empty() {
        return Err("Template name cannot be empty.".into());
    }
    let s = state.lock().map_err(|e| e.to_string())?;
    if !s.has_project {
        return Err("No active project.".into());
    }
    let snap = s.current.clone();
    drop(s);

    let id = format!("user_{}", now_unix_millis());
    let mut embedded_assets = HashMap::new();
    for layer in &snap.layers {
        if let LayerKind::Image { source_path, .. } = &layer.kind {
            if let Ok(bytes) = std::fs::read(source_path) {
                embedded_assets.insert(
                    layer.id.clone(),
                    base64::engine::general_purpose::STANDARD.encode(&bytes),
                );
            }
        }
    }

    let tpl = VTemplate {
        schema_version: 1,
        id: id.clone(),
        name: name.trim().to_string(),
        category: if category.trim().is_empty() {
            "Custom".to_string()
        } else {
            category.trim().to_string()
        },
        builtin: false,
        snapshot: snap.clone(),
        fields: fields.clone(),
        embedded_assets,
    };

    let dir = ensure_subdir("templates")?;
    let safe: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let dest = dir.join(format!("{}_{}.vtemplate", safe, now_unix_millis()));
    let json = serde_json::to_string_pretty(&tpl).map_err(|e| e.to_string())?;
    atomic_write_str(&dest, &json)?;

    let preview_data_url = render_template_preview_data_url(&snap);
    Ok(TemplateSummary {
        id,
        name: tpl.name,
        category: tpl.category,
        builtin: false,
        file_path: dest.to_string_lossy().to_string(),
        canvas_width: snap.canvas_width,
        canvas_height: snap.canvas_height,
        fields,
        preview_data_url,
    })
}

fn apply_brand_kit_to_snapshot(
    snap: &mut ProjectSnapshot,
    tpl: &VTemplate,
    brand_kit_id: Option<&str>,
) {
    let Some(kid) = brand_kit_id.filter(|s| !s.trim().is_empty()) else {
        return;
    };
    let store = crate::brand::load_brand_store();
    let Some(kit) = store.kits.iter().find(|k| k.id == kid) else {
        return;
    };
    let primary_hex = kit.colors.first().map(|c| c.hex.as_str());
    let primary_font = kit.fonts.first().map(|f| f.as_str());
    let primary_logo = kit.logos.first().map(|l| l.path.as_str());

    for layer in &mut snap.layers {
        match &mut layer.kind {
            LayerKind::Text3D { font_style, .. } => {
                if let Some(f) = primary_font {
                    *font_style = f.to_string();
                }
            }
            LayerKind::Shape { fill_color, .. } => {
                // Apply primary brand color to color-editable shape fields
                if tpl.fields.iter().any(|f| f.kind == "color" && f.layer_id == layer.id) {
                    if let Some(hex) = primary_hex {
                        *fill_color = parse_hex_color(hex);
                    }
                }
            }
            LayerKind::Image { source_path, .. } => {
                if tpl.fields.iter().any(|f| f.kind == "image" && f.layer_id == layer.id) {
                    if let Some(lp) = primary_logo {
                        if Path::new(lp).is_file() {
                            *source_path = lp.to_string();
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

#[tauri::command]
pub fn preview_vtemplate_filled(
    template_id: String,
    values: HashMap<String, String>,
    brand_kit_id: Option<String>,
) -> Result<String, String> {
    let all = load_all_vtemplates()?;
    let (tpl, _) = all
        .into_iter()
        .find(|(t, _)| t.id == template_id)
        .ok_or_else(|| format!("Template '{}' not found.", template_id))?;
    let mut snap = apply_fields_to_snapshot(&tpl, &values, None);
    apply_brand_kit_to_snapshot(&mut snap, &tpl, brand_kit_id.as_deref());
    render_template_preview_data_url(&snap)
        .ok_or_else(|| "Failed to render template preview.".to_string())
}

#[tauri::command]
pub fn create_project_from_vtemplate(
    state: tauri::State<'_, SharedProjectState>,
    template_id: String,
    values: HashMap<String, String>,
    project_name: Option<String>,
    brand_kit_id: Option<String>,
) -> Result<ProjectResponse, String> {
    let all = load_all_vtemplates()?;
    let (tpl, _) = all
        .into_iter()
        .find(|(t, _)| t.id == template_id)
        .ok_or_else(|| format!("Template '{}' not found.", template_id))?;

    let pid = format!("proj_{}", now_unix_millis());
    let mut snap = apply_fields_to_snapshot(&tpl, &values, Some(&pid));
    apply_brand_kit_to_snapshot(&mut snap, &tpl, brand_kit_id.as_deref());
    snap.project_id = pid.clone();
    snap.schema_version = CURRENT_SCHEMA_VERSION;
    snap.project_name = project_name
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| tpl.name.clone());

    // Extract embedded assets if any image layer points to a missing path
    if !tpl.embedded_assets.is_empty() {
        let assets_dir = crate::storage::project_assets_dir(&pid)?;
        for layer in &mut snap.layers {
            if let LayerKind::Image { source_path, .. } = &mut layer.kind {
                if !Path::new(source_path.as_str()).is_file() {
                    if let Some(b64) = tpl.embedded_assets.get(&layer.id) {
                        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(b64) {
                            let dest = assets_dir.join(format!("{}.png", layer.id));
                            if atomic_write_bytes(&dest, &bytes).is_ok() {
                                *source_path = dest.to_string_lossy().to_string();
                            }
                        }
                    }
                }
            }
        }
    }

    let proj_dir = ensure_subdir("projects")?;
    let slug: String = snap
        .project_name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let proj_path = proj_dir.join(format!("{}_{}.veltrax", slug.trim_matches('_'), pid));
    let json = serde_json::to_string_pretty(&snap).map_err(|e| e.to_string())?;
    atomic_write_str(&proj_path, &json)?;

    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.undo_stack.clear();
    s.redo_stack.clear();
    s.has_project = true;
    s.is_in_editor = true;
    s.project_file_path = Some(proj_path.to_string_lossy().to_string());
    s.dirty = false;
    s.current = snap;
    write_session_lock();
    let resp = render_and_respond(&mut s)?;
    let _ = record_recent_project(&proj_path.to_string_lossy(), &s.current, None);
    Ok(resp)
}

#[tauri::command]
pub fn export_vtemplate_file(template_id: String, dest_path: String) -> Result<String, String> {
    let all = load_all_vtemplates()?;
    let (mut tpl, _) = all
        .into_iter()
        .find(|(t, _)| t.id == template_id)
        .ok_or_else(|| format!("Template '{}' not found.", template_id))?;
    tpl.builtin = false;
    for layer in &tpl.snapshot.layers {
        if let LayerKind::Image { source_path, .. } = &layer.kind {
            if !tpl.embedded_assets.contains_key(&layer.id) {
                if let Ok(bytes) = std::fs::read(source_path) {
                    tpl.embedded_assets.insert(
                        layer.id.clone(),
                        base64::engine::general_purpose::STANDARD.encode(&bytes),
                    );
                }
            }
        }
    }
    let json = serde_json::to_string_pretty(&tpl).map_err(|e| e.to_string())?;
    atomic_write_str(Path::new(&dest_path), &json)?;
    Ok(dest_path)
}

#[tauri::command]
pub fn import_vtemplate_file(src_path: String) -> Result<TemplateSummary, String> {
    let raw = std::fs::read_to_string(&src_path)
        .map_err(|e| format!("Failed to read '{}': {}", src_path, e))?;
    let mut tpl: VTemplate =
        serde_json::from_str(&raw).map_err(|e| format!("Invalid .vtemplate file: {}", e))?;
    tpl.builtin = false;
    tpl.id = format!("imp_{}", now_unix_millis());

    let dir = ensure_subdir("templates")?;
    let dest = dir.join(format!("{}.vtemplate", tpl.id));
    let json = serde_json::to_string_pretty(&tpl).map_err(|e| e.to_string())?;
    atomic_write_str(&dest, &json)?;

    let preview_data_url = render_template_preview_data_url(&tpl.snapshot);
    Ok(TemplateSummary {
        id: tpl.id,
        name: tpl.name,
        category: tpl.category,
        builtin: false,
        file_path: dest.to_string_lossy().to_string(),
        canvas_width: tpl.snapshot.canvas_width,
        canvas_height: tpl.snapshot.canvas_height,
        fields: tpl.fields,
        preview_data_url,
    })
}

#[tauri::command]
pub fn delete_vtemplate(template_id: String) -> Result<(), String> {
    let all = load_all_vtemplates()?;
    for (tpl, path) in all {
        if tpl.id == template_id && !tpl.builtin && !path.is_empty() {
            let p = Path::new(&path);
            if p.is_file() {
                std::fs::remove_file(p).map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}
