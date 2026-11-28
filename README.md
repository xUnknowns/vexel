<div align="center">

<img src="src-tauri/icons/icon.png" width="96" alt="Veltrax Studios logo" />

# Veltrax Studios

### *The image editor built for people who don't design — powered by AI.*

**by [Crafters Apex](https://github.com/xUnknowns)**

[![License: Apache 2.0](https://img.shields.io/badge/License-Apache_2.0-6366F1.svg)](LICENSE)
[![Tauri v2](https://img.shields.io/badge/Tauri-v2-24C8DB?logo=tauri)](https://tauri.app)
[![Svelte 5](https://img.shields.io/badge/Svelte-5-FF3E00?logo=svelte)](https://svelte.dev)
[![Rust](https://img.shields.io/badge/Rust-CE4A07?logo=rust)](https://rust-lang.org)
[![CI](https://img.shields.io/badge/CI-CircleCI-343434?logo=circleci)](https://circleci.com)

</div>

---

## What is Veltrax Studios?

You don't need a design degree to make something look great.

**Veltrax Studios** is an ultra-fast, free and open-source desktop image editor for Windows, built from the ground up for the person who *just needs it to work*. No overwhelming menus. No subscription walls. No bloat. Just a clean, dark workspace where your images transform in milliseconds, powered by a native **Rust** processing engine under the hood.

But Veltrax Studios is more than a simple editor.

Its central philosophy is the seamless union of **human creativity and artificial intelligence**. Through the **Model Context Protocol (MCP)**, Veltrax Studios connects directly to Claude AI — letting you describe what you want in plain language and watch the editor carry it out. Want to remove noise, adjust the mood, or apply a complex filter sequence? Just ask. Veltrax Studios' MCP bridge translates your words into precise, non-destructive image operations without you writing a single line of code or knowing what "curves" means.

> **Built for the non-designer. Powered by AI. Free forever.**

---

## Core Philosophy

```
Human intent → Claude AI (MCP) → Rust image engine → Pixel-perfect result
```

| Pillar | What it means for you |
|---|---|
| 🎯 **Zero design knowledge required** | Intuitive controls with sane defaults — just open and edit |
| 🤖 **AI as your co-pilot** | Describe edits in plain English; Claude drives the Rust engine for you |
| ⚡ **Native performance** | Rust processes every pixel locally — no cloud upload, no lag |
| 🔓 **Truly free** | Apache 2.0 licensed. Fork it, ship it, build on it |
| 🧰 **Expandable toolset** | Brightness, contrast, crop, resize, rotate, filters — and growing |

---

## Features

- 🖼️ **Non-destructive edit history** — Undo any step instantly
- ☀️ **Brightness & Contrast** — Slider-driven, applied natively in Rust
- 🔄 **Rotate** 90° / 180° and **Flip** H / V
- ✂️ **Crop** to any region
- 📐 **Resize** with Lanczos3 for sharp downscaling
- 🎨 **Quick Filters** — Grayscale, Blur, Invert
- 💾 **Export** to PNG, JPEG, WebP
- 📂 **Recent Projects** per session
- 🤖 **MCP Workspace** — ready to wire Claude Desktop to every edit command

---

## Stack

| Layer | Technology |
|---|---|
| Desktop shell | Tauri v2 (Windows, Asset Protocol) |
| Frontend | Svelte 5 + Vite 6 |
| Image engine | Rust — `image` v0.25, Lanczos3, non-destructive temp files |
| AI bridge | Model Context Protocol (MCP) — Claude Desktop integration |
| CI/CD | CircleCI `win/default` executor → `.exe` artifact |

---

## Development

> **Note:** Rust compilation (`cargo build` / `npm run tauri build`) runs in CircleCI. Don't compile locally on a low-resource machine — push to `main` and let the pipeline produce the `.exe` for you.

```bash
# Install JS dependencies (first time)
npm install

# Start the Vite dev server only (no Rust compilation needed)
npm run dev
```

## Beta pública, privacidad y soporte

- El Dashboard muestra la versión de `src-tauri/Cargo.toml` con la etiqueta **Beta**. Al detectar un número distinto de la última versión vista, aparece el breve panel **Novedades**; al cerrarlo, la versión vista se conserva en `settings.json`.
- **Enviar sugerencia / reportar error** copia al portapapeles un bloque local con versión, sistema operativo, resolución y hasta cinco categorías/horas de error. El texto evita incluir mensajes arbitrarios, nombres de usuario, rutas o credenciales. Después abre `FEEDBACK_URL` en `src-tauri/src/support.rs`, actualmente un placeholder HTTPS que debe reemplazarse. La app no adjunta ni envía automáticamente el diagnóstico.
- El log se guarda en `%APPDATA%\\Veltrax Studios\\logs\\app.log`, con tres rotaciones de hasta 1 MiB cada una. Registra pánicos Rust y excepciones/promesas no controladas del frontend; los mensajes quedan solo en el equipo y el bloque de feedback expone únicamente categorías genéricas. El panic hook intenta persistir el último snapshot de proyecto en el área local de recuperación; no transmite telemetría.
- El uso corriente y el procesamiento de imágenes son locales. No existe envío de imágenes a servidores. **AI Remove BG** necesita internet solo si se solicita y el modelo U2-Net aún no está almacenado; la descarga se hace desde GitHub a `%APPDATA%\\Veltrax Studios\\models\\` con escritura atómica. Una vez descargado, la inferencia se ejecuta localmente. La herramienta MCP responde a la petición explícita del cliente MCP y no sube archivos a servicios de Veltrax.
- El proyecto no tiene integrado el updater de Tauri. El punto de extensión queda identificado en la configuración de bundle y los comandos de soporte; para habilitar actualizaciones firmadas faltan el plugin `tauri-plugin-updater`, configuración del updater en `tauri.conf.json`, endpoint/manifest de releases y la clave privada de firma protegida fuera del repositorio (más la clave pública configurada en la app). No se incorporó una clave o feed ficticio.
- El MCP también expone `get_app_version`, `get_feedback_diagnostics`, `open_feedback_site` y `record_local_error`, con descripciones y schemas tipados. La herramienta de registro solo escribe un mensaje explícitamente indicado en el log local (con límite de longitud); la herramienta de apertura solo lanza el navegador. Ninguna de ellas sube datos.

### Headless CLI

The Windows executable can render a template without opening the editor. Use a built-in template ID/name or a `.vtemplate` file:

```powershell
veltrax-studios.exe --render builtin_youtube_thumb --vars vars.json --out out\thumb.png --size 1280x720
veltrax-studios.exe --batch builtin_youtube_thumb --data rows.csv --out out\batch --pattern "{title}_{index}" --format webp
```

`vars.json` is a JSON object keyed by template variable name. Batch input accepts CSV with a header row or a JSON array of objects. Supported raster formats are PNG, JPEG and WebP; `--quality` accepts 1–100. `--size WIDTHxHEIGHT` fits the design proportionally and pads the remaining canvas rather than stretching it. Existing output files are left unchanged and reported as errors. Use `veltrax-studios.exe --help` for the full syntax. The existing `--mcp` entry point continues to use standard input/output pipes.

The MCP server also exposes `render_template` for a single `.vtemplate` render and `render_template_batch` for CSV/JSON rows. These create output files without editing the active project's layers; `add_smart_text` remains the separate tool for inserting variable text into the active project.

### MCP project, template and library tools

The MCP catalog also includes `create_project`, `export_image`, `save_project`, `list_projects`, `list_templates`, `create_from_template`, `save_as_template`, `batch_render`, `get_brand_kit`, `apply_brand_kit`, `list_library` and `list_fonts`. `batch_render` is an alias for the existing `render_template_batch` flow; it does not create a second renderer. `apply_brand_kit` applies the selected profile's primary color to shape layers, first font to text layers and first logo to image layers. `inspect_project` returns the complete serialized project snapshot (including its properties and layers) together with the JPEG preview.

The Editor gate remains in effect for layer edits, `inspect_project`, `save_project`, `save_as_template` and `apply_brand_kit`. Read-only `list_*` tools, `get_brand_kit` and template batch rendering work from the Dashboard. Creating a blank project or creating one from a template is also allowed there; the app automatically navigates to the Editor after MCP reports the new project. `export_image` is available from the Dashboard only when a project is already loaded in the session. Project saves retain the existing `.veltrax` JSON format and existing files are not migrated just by using MCP; exported images are written through a temporary file and do not overwrite an existing destination.

---
## CI/CD

Every push to `main` triggers a CircleCI Windows pipeline that:

1. Installs Node.js LTS + Rust stable
2. Caches the Cargo registry
3. Runs `npm run tauri build`
4. Stores the installer bundle under **Artifacts → `veltrax-studios-windows-bundle`**

---

## Project Structure

```
Veltrax-Studios/
├── .circleci/config.yml              CircleCI Windows pipeline
├── src/
│   ├── App.svelte                    Full UI — Dashboard / Editor / MCP view
│   ├── app.css                       Design tokens (dark studio theme)
│   └── main.js                       Svelte 5 entry point
├── src-tauri/
│   ├── src/
│   │   ├── main.rs                   Tauri bootstrap + command registration
│   │   └── tools.rs                  Rust image commands (6 operations)
│   ├── Cargo.toml
│   ├── tauri.conf.json               Asset Protocol enabled
│   └── capabilities/default.json
├── index.html
├── vite.config.js
├── svelte.config.js
└── package.json
```

---

## Roadmap

- [ ] MCP sidecar — Claude Desktop JSON-RPC bridge
- [ ] Layers and blend modes
- [ ] AI-powered background removal
- [ ] Batch processing
- [ ] Plugin system for community tools
- [ ] macOS and Linux builds

---

## License

```
Copyright 2024 Crafters Apex

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
```

---

<div align="center">

Made with ❤️ and Rust by **Crafters Apex**

*If Veltrax Studios saved you time, leave a ⭐ — it means the world.*

</div>
