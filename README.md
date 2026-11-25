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
