# Vexel — Crafters Apex

Ultra-fast, lightweight desktop image editor built with Rust + Tauri v2 + Svelte 5.

## Stack
- **Backend / image engine:** Rust (`image` v0.25) via Tauri v2 commands
- **Frontend:** Svelte 5 + Vite 6
- **Desktop shell:** Tauri v2 (Windows, Asset Protocol enabled)
- **AI bridge:** MCP-ready for Claude Desktop integration

## Features
- Non-destructive edit history (Undo)
- Brightness & Contrast adjustments
- Rotate 90°, 180°, Flip H/V
- Resize (Lanczos3)
- Crop
- Quick Filters: Grayscale, Blur, Invert
- Export to PNG / JPEG / WebP
- Recent Projects per session
- MCP workspace prepared for Claude Desktop

## Development
> **Note:** Rust compilation runs in CircleCI (cloud). Do not run `cargo build` / `npm run tauri build` locally.

```bash
# Install JS dependencies (first time)
npm install

# Start Vite dev server only (no Rust compilation)
npm run dev
```

## CI/CD
Builds are handled by CircleCI on the `win/default` executor.  
Artifacts are stored in `src-tauri/target/release/bundle`.

## Project structure
```
.circleci/config.yml       CircleCI Windows pipeline
src/                       Svelte 5 frontend
  App.svelte               Main UI (Dashboard / Editor / MCP)
  app.css                  Design tokens + global styles
  main.js                  Svelte mount entry point
src-tauri/
  src/
    main.rs                Tauri bootstrap
    tools.rs               Rust image commands
  Cargo.toml
  tauri.conf.json
  capabilities/default.json
  icons/
index.html
vite.config.js
svelte.config.js
package.json
```
