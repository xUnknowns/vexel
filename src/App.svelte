<script>
  import { invoke, convertFileSrc } from '@tauri-apps/api/core';
  import { tempDir, join } from '@tauri-apps/api/path';
  import { open, save } from '@tauri-apps/plugin-dialog';

  let activeView = $state('dashboard');
  let history = $state([]);
  let historyIndex = $state(-1);
  let recentProjects = $state([]);
  let isProcessing = $state(false);
  let statusMessage = $state('Ready');
  let errorMessage = $state('');
  let cacheBuster = $state(0);

  // Image dimensions
  let imgWidth = $state(0);
  let imgHeight = $state(0);

  // Adjustment controls
  let brightness = $state(0);
  let contrast = $state(0);

  // Resize controls
  let resizeWidth = $state(800);
  let resizeHeight = $state(600);

  // Crop controls
  let cropX = $state(0);
  let cropY = $state(0);
  let cropWidth = $state(400);
  let cropHeight = $state(400);

  let currentPath = $derived(
    historyIndex >= 0 && historyIndex < history.length ? history[historyIndex] : null
  );

  let previewUrl = $derived(
    currentPath ? `${convertFileSrc(currentPath)}?v=${cacheBuster}` : ''
  );

  let canUndo = $derived(historyIndex > 0 && !isProcessing);
  let hasImage = $derived(Boolean(currentPath));

  function getFileName(filePath) {
    if (!filePath) return '';
    const parts = filePath.split(/[/\\]/);
    return parts[parts.length - 1] || filePath;
  }

  function getExtension(filePath) {
    const name = getFileName(filePath);
    const dot = name.lastIndexOf('.');
    return dot !== -1 ? name.slice(dot + 1).toLowerCase() : 'png';
  }

  async function nextTempPath(sourcePath) {
    const dir = await tempDir();
    const ext = getExtension(sourcePath);
    const safeExt = ['png', 'jpg', 'jpeg', 'webp', 'bmp'].includes(ext) ? ext : 'png';
    const stamp = `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
    return await join(dir, `vexel-${stamp}.${safeExt}`);
  }

  function addToRecent(filePath) {
    const name = getFileName(filePath);
    const filtered = recentProjects.filter((item) => item.path !== filePath);
    recentProjects = [
      { path: filePath, name, openedAt: new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }) },
      ...filtered,
    ].slice(0, 8);
  }

  function loadImageSession(filePath) {
    history = [filePath];
    historyIndex = 0;
    brightness = 0;
    contrast = 0;
    errorMessage = '';
    statusMessage = `Loaded ${getFileName(filePath)}`;
    cacheBuster += 1;
    addToRecent(filePath);
    activeView = 'editor';
  }

  async function handleOpenImage() {
    errorMessage = '';
    try {
      const selected = await open({
        multiple: false,
        title: 'Open Image',
        filters: [
          {
            name: 'Images',
            extensions: ['png', 'jpg', 'jpeg', 'webp', 'bmp', 'gif', 'tiff'],
          },
        ],
      });
      if (typeof selected === 'string' && selected.length > 0) {
        loadImageSession(selected);
      }
    } catch (err) {
      errorMessage = `Failed to open file dialog: ${err}`;
    }
  }

  function handleUndo() {
    if (!canUndo) return;
    historyIndex -= 1;
    brightness = 0;
    contrast = 0;
    errorMessage = '';
    statusMessage = `Reverted to step ${historyIndex + 1}`;
    cacheBuster += 1;
  }

  async function handleExport() {
    if (!currentPath || isProcessing) return;
    errorMessage = '';
    try {
      const dest = await save({
        title: 'Export Image',
        defaultPath: `vexel-export.${getExtension(currentPath)}`,
        filters: [
          { name: 'PNG Image', extensions: ['png'] },
          { name: 'JPEG Image', extensions: ['jpg', 'jpeg'] },
          { name: 'WebP Image', extensions: ['webp'] },
        ],
      });
      if (!dest) return;
      isProcessing = true;
      statusMessage = 'Exporting image...';
      await invoke('export_file', { sourcePath: currentPath, destPath: dest });
      statusMessage = `Exported to ${getFileName(dest)}`;
    } catch (err) {
      errorMessage = String(err);
      statusMessage = 'Export failed';
    } finally {
      isProcessing = false;
    }
  }

  async function runEditCommand(commandName, argsBuilder, label) {
    if (!currentPath || isProcessing) return;
    isProcessing = true;
    errorMessage = '';
    statusMessage = `${label}...`;
    try {
      const destPath = await nextTempPath(currentPath);
      const args = argsBuilder(currentPath, destPath);
      const resultPath = await invoke(commandName, args);
      history = [...history.slice(0, historyIndex + 1), resultPath];
      historyIndex = history.length - 1;
      cacheBuster += 1;
      statusMessage = `${label} applied`;
    } catch (err) {
      errorMessage = String(err);
      statusMessage = `${label} failed`;
    } finally {
      isProcessing = false;
    }
  }

  function applyBrightnessContrast() {
    runEditCommand(
      'adjust_brightness_contrast',
      (sourcePath, destPath) => ({
        sourcePath,
        destPath,
        brightness: Number(brightness),
        contrast: Number(contrast),
      }),
      'Brightness & Contrast'
    ).then(() => {
      brightness = 0;
      contrast = 0;
    });
  }

  function applyTransform(action, label) {
    runEditCommand(
      'transform_image',
      (sourcePath, destPath) => ({ sourcePath, destPath, action }),
      label
    );
  }

  function applyResize() {
    runEditCommand(
      'resize_image',
      (sourcePath, destPath) => ({
        sourcePath,
        destPath,
        newWidth: Math.max(1, Number(resizeWidth)),
        newHeight: Math.max(1, Number(resizeHeight)),
      }),
      'Resize'
    );
  }

  function applyCrop() {
    runEditCommand(
      'crop_image',
      (sourcePath, destPath) => ({
        sourcePath,
        destPath,
        x: Math.max(0, Number(cropX)),
        y: Math.max(0, Number(cropY)),
        width: Math.max(1, Number(cropWidth)),
        height: Math.max(1, Number(cropHeight)),
      }),
      'Crop'
    );
  }

  function applyFilter(filter, label) {
    runEditCommand(
      'apply_filter',
      (sourcePath, destPath) => ({ sourcePath, destPath, filter }),
      label
    );
  }

  function onImageLoad(event) {
    const img = event.currentTarget;
    imgWidth = img.naturalWidth;
    imgHeight = img.naturalHeight;
    resizeWidth = img.naturalWidth;
    resizeHeight = img.naturalHeight;
    cropX = 0;
    cropY = 0;
    cropWidth = Math.max(1, Math.floor(img.naturalWidth / 2));
    cropHeight = Math.max(1, Math.floor(img.naturalHeight / 2));
  }
</script>

<div class="app-shell">
  <!-- Fixed Left Sidebar -->
  <aside class="sidebar">
    <div class="brand">
      <div class="brand-mark">V</div>
      <div class="brand-text">
        <span class="brand-name">VEXEL</span>
        <span class="brand-sub">Crafters Apex</span>
      </div>
    </div>

    <nav class="nav-list" aria-label="Primary Navigation">
      <button
        class="nav-btn"
        class:active={activeView === 'editor'}
        onclick={() => (activeView = 'editor')}
        title="Open Image Editor"
      >
        <span class="nav-icon">🎨</span>
        <span>Editor</span>
      </button>

      <button
        class="nav-btn"
        class:active={activeView === 'dashboard'}
        onclick={() => (activeView = 'dashboard')}
        title="Open Dashboard"
      >
        <span class="nav-icon">🏠</span>
        <span>Dashboard</span>
      </button>

      <button
        class="nav-btn"
        class:active={activeView === 'mcp'}
        onclick={() => (activeView = 'mcp')}
        title="Configure Claude MCP Bridge"
      >
        <span class="nav-icon">🤖</span>
        <span>Connect AI (MCP)</span>
      </button>
    </nav>

    <div class="sidebar-footer">
      <div class="engine-badge">
        <span class="dot"></span>
        <span>Rust Engine Ready</span>
      </div>
    </div>
  </aside>

  <!-- Main Workspace -->
  <main class="workspace">
    {#if activeView === 'dashboard'}
      <section class="dashboard-view">
        <div class="bento-card hero-card">
          <div class="hero-content">
            <span class="pill">Studio v0.1 · Windows</span>
            <h1>Welcome to Vexel</h1>
            <p>
              Ultra-fast, lightweight desktop image editing powered by a native Rust engine and
              ready for Claude AI via Model Context Protocol.
            </p>
            <div class="hero-actions">
              <button class="btn-accent" onclick={handleOpenImage}>+ Open Image</button>
              {#if hasImage}
                <button class="btn-secondary" onclick={() => (activeView = 'editor')}>
                  Resume Current Session
                </button>
              {/if}
            </div>
          </div>
        </div>

        <div class="bento-grid">
          <div class="bento-card feature-card">
            <h3>Native Rust Core</h3>
            <p>Zero-bloat pixel transformations, filters, resizing, and exports on local hardware.</p>
          </div>
          <div class="bento-card feature-card">
            <h3>Asset Protocol Canvas</h3>
            <p>Direct local file streaming keeps memory overhead minimal on large images.</p>
          </div>
          <div class="bento-card feature-card">
            <h3>MCP Ready</h3>
            <p>Built from the ground up to connect seamlessly with Claude Desktop workflows.</p>
          </div>
        </div>

        <div class="bento-card recent-card">
          <div class="section-header">
            <h2>Recent Projects</h2>
            <span class="muted-count">{recentProjects.length} in session</span>
          </div>

          {#if recentProjects.length === 0}
            <div class="empty-recent">
              <p>No recent projects in this session yet. Click <strong>+ Open Image</strong> to begin.</p>
            </div>
          {:else}
            <div class="recent-list">
              {#each recentProjects as item}
                <button class="recent-item" onclick={() => loadImageSession(item.path)}>
                  <div class="recent-info">
                    <span class="recent-name">{item.name}</span>
                    <span class="recent-path">{item.path}</span>
                  </div>
                  <span class="recent-time">{item.openedAt}</span>
                </button>
              {/each}
            </div>
          {/if}
        </div>
      </section>
    {:else if activeView === 'editor'}
      <section class="editor-view">
        <header class="editor-topbar">
          <div class="topbar-left">
            <button class="btn-secondary" onclick={handleOpenImage} disabled={isProcessing}>
              Open
            </button>
            <button class="btn-secondary" onclick={handleUndo} disabled={!canUndo}>
              Undo
            </button>
            <button class="btn-accent" onclick={handleExport} disabled={!hasImage || isProcessing}>
              Export
            </button>
          </div>

          <div class="topbar-center">
            <span class="step-badge">
              {#if hasImage}
                Step {historyIndex + 1} / {history.length}
              {:else}
                0 steps
              {/if}
            </span>
            {#if hasImage && imgWidth > 0}
              <span class="dim-badge">{imgWidth} × {imgHeight} px</span>
            {/if}
          </div>

          <div class="topbar-right">
            <span class="status-text" class:error={Boolean(errorMessage)}>
              {errorMessage || statusMessage}
            </span>
          </div>
        </header>

        <div class="editor-body">
          <div class="canvas-stage">
            {#if hasImage}
              <div class="canvas-frame">
                <img
                  src={previewUrl}
                  alt="Active editing canvas"
                  onload={onImageLoad}
                />
                {#if isProcessing}
                  <div class="processing-overlay">
                    <span>Processing in Rust...</span>
                  </div>
                {/if}
              </div>
            {:else}
              <div class="empty-canvas">
                <h2>No Image Loaded</h2>
                <p>Open a PNG, JPEG, WebP, or BMP file to start editing.</p>
                <button class="btn-accent" onclick={handleOpenImage}>+ Open Image</button>
              </div>
            {/if}
          </div>

          <aside class="tool-panel">
            <div class="tool-section">
              <h3>Brightness &amp; Contrast</h3>
              <label class="slider-label">
                <span>Brightness</span>
                <span class="mono">{brightness}</span>
              </label>
              <input
                type="range"
                min="-100"
                max="100"
                step="1"
                bind:value={brightness}
                disabled={!hasImage || isProcessing}
              />

              <label class="slider-label">
                <span>Contrast</span>
                <span class="mono">{contrast}</span>
              </label>
              <input
                type="range"
                min="-100"
                max="100"
                step="1"
                bind:value={contrast}
                disabled={!hasImage || isProcessing}
              />

              <button
                class="btn-accent full-width"
                onclick={applyBrightnessContrast}
                disabled={!hasImage || isProcessing}
              >
                Apply Adjustments
              </button>
            </div>

            <div class="tool-section">
              <h3>Transform</h3>
              <div class="btn-grid-2">
                <button
                  class="btn-secondary"
                  onclick={() => applyTransform('rotate90', 'Rotate 90°')}
                  disabled={!hasImage || isProcessing}
                >
                  Rotate 90°
                </button>
                <button
                  class="btn-secondary"
                  onclick={() => applyTransform('rotate180', 'Rotate 180°')}
                  disabled={!hasImage || isProcessing}
                >
                  Rotate 180°
                </button>
                <button
                  class="btn-secondary"
                  onclick={() => applyTransform('flip_h', 'Flip Horizontal')}
                  disabled={!hasImage || isProcessing}
                >
                  Flip H
                </button>
                <button
                  class="btn-secondary"
                  onclick={() => applyTransform('flip_v', 'Flip Vertical')}
                  disabled={!hasImage || isProcessing}
                >
                  Flip V
                </button>
              </div>
            </div>

            <div class="tool-section">
              <h3>Resize</h3>
              <div class="input-grid-2">
                <label class="field">
                  <span>Width</span>
                  <input
                    type="number"
                    min="1"
                    bind:value={resizeWidth}
                    disabled={!hasImage || isProcessing}
                  />
                </label>
                <label class="field">
                  <span>Height</span>
                  <input
                    type="number"
                    min="1"
                    bind:value={resizeHeight}
                    disabled={!hasImage || isProcessing}
                  />
                </label>
              </div>
              <button
                class="btn-secondary full-width"
                onclick={applyResize}
                disabled={!hasImage || isProcessing}
              >
                Apply Resize
              </button>
            </div>

            <div class="tool-section">
              <h3>Crop</h3>
              <div class="input-grid-2">
                <label class="field">
                  <span>X</span>
                  <input type="number" min="0" bind:value={cropX} disabled={!hasImage || isProcessing} />
                </label>
                <label class="field">
                  <span>Y</span>
                  <input type="number" min="0" bind:value={cropY} disabled={!hasImage || isProcessing} />
                </label>
                <label class="field">
                  <span>Width</span>
                  <input
                    type="number"
                    min="1"
                    bind:value={cropWidth}
                    disabled={!hasImage || isProcessing}
                  />
                </label>
                <label class="field">
                  <span>Height</span>
                  <input
                    type="number"
                    min="1"
                    bind:value={cropHeight}
                    disabled={!hasImage || isProcessing}
                  />
                </label>
              </div>
              <button
                class="btn-secondary full-width"
                onclick={applyCrop}
                disabled={!hasImage || isProcessing}
              >
                Apply Crop
              </button>
            </div>

            <div class="tool-section">
              <h3>Quick Filters</h3>
              <div class="btn-grid-3">
                <button
                  class="btn-secondary"
                  onclick={() => applyFilter('grayscale', 'Grayscale')}
                  disabled={!hasImage || isProcessing}
                >
                  Grayscale
                </button>
                <button
                  class="btn-secondary"
                  onclick={() => applyFilter('blur', 'Blur')}
                  disabled={!hasImage || isProcessing}
                >
                  Blur
                </button>
                <button
                  class="btn-secondary"
                  onclick={() => applyFilter('invert', 'Invert')}
                  disabled={!hasImage || isProcessing}
                >
                  Invert
                </button>
              </div>
            </div>
          </aside>
        </div>
      </section>
    {:else if activeView === 'mcp'}
      <section class="mcp-view">
        <div class="bento-card mcp-hero">
          <span class="pill">Model Context Protocol</span>
          <h1>Connect AI (MCP)</h1>
          <p>
            This workspace is reserved for the local MCP bridge connecting Vexel's Rust image
            commands directly to Claude Desktop.
          </p>
          <div class="mcp-status-row">
            <span class="status-pill">Bridge Status: Standby</span>
            <span class="status-pill muted">Protocol: Local Stdio / JSON-RPC</span>
          </div>
        </div>

        <div class="bento-grid">
          <div class="bento-card">
            <h3>Exposed Rust Commands</h3>
            <ul class="mcp-list">
              <li><code>adjust_brightness_contrast</code></li>
              <li><code>transform_image</code></li>
              <li><code>crop_image</code></li>
              <li><code>resize_image</code></li>
              <li><code>apply_filter</code></li>
              <li><code>export_file</code></li>
            </ul>
          </div>

          <div class="bento-card">
            <h3>Claude Desktop Integration</h3>
            <p class="mcp-note">
              Once the MCP sidecar is enabled, you will be able to copy your
              <code>claude_desktop_config.json</code> snippet and grant Claude direct access to
              Vexel's non-destructive editing pipeline.
            </p>
            <button class="btn-secondary" disabled>Bridge Coming Soon</button>
          </div>
        </div>
      </section>
    {/if}
  </main>
</div>

<style>
  .app-shell {
    display: flex;
    height: 100vh;
    width: 100vw;
    overflow: hidden;
    background-color: var(--bg-canvas);
  }

  /* Sidebar */
  .sidebar {
    width: 236px;
    flex-shrink: 0;
    background-color: var(--bg-card);
    border-right: 1px solid var(--border-subtle);
    display: flex;
    flex-direction: column;
    padding: 20px 14px;
    gap: 24px;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 4px 8px;
  }

  .brand-mark {
    width: 36px;
    height: 36px;
    border-radius: 10px;
    background: linear-gradient(135deg, var(--accent-primary), #818cf8);
    display: grid;
    place-items: center;
    font-weight: 800;
    font-size: 18px;
    color: var(--text-primary);
    box-shadow: 0 6px 18px rgba(99, 102, 241, 0.35);
  }

  .brand-text {
    display: flex;
    flex-direction: column;
  }

  .brand-name {
    font-size: 16px;
    font-weight: 800;
    letter-spacing: 0.08em;
    color: var(--text-primary);
  }

  .brand-sub {
    font-size: 11px;
    color: var(--text-muted);
  }

  .nav-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .nav-btn {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 11px 14px;
    border-radius: var(--radius-button);
    border: 1px solid transparent;
    background: transparent;
    color: var(--text-secondary);
    font-size: 14px;
    font-weight: 500;
    text-align: left;
    cursor: pointer;
  }

  .nav-btn:hover {
    background-color: var(--bg-card-hover);
    color: var(--text-primary);
    border-color: var(--border-subtle);
  }

  .nav-btn.active {
    background-color: var(--accent-subtle);
    color: var(--text-primary);
    border-color: var(--border-hover);
    font-weight: 600;
  }

  .nav-icon {
    font-size: 16px;
  }

  .sidebar-footer {
    margin-top: auto;
    padding: 8px;
  }

  .engine-badge {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--text-secondary);
    background-color: var(--bg-elevated);
    padding: 8px 12px;
    border-radius: var(--radius-badge);
    border: 1px solid var(--border-subtle);
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background-color: #10b981;
    box-shadow: 0 0 8px rgba(16, 185, 129, 0.7);
  }

  /* Workspace */
  .workspace {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  /* Dashboard & MCP shared */
  .dashboard-view,
  .mcp-view {
    padding: 28px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: var(--gap-grid);
    height: 100%;
  }

  .pill {
    display: inline-block;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: #a5b4fc;
    background-color: var(--accent-subtle);
    padding: 5px 10px;
    border-radius: var(--radius-badge);
    margin-bottom: 12px;
  }

  .hero-card h1,
  .mcp-hero h1 {
    margin: 0 0 10px 0;
    font-size: 28px;
    font-weight: 700;
  }

  .hero-card p,
  .mcp-hero p {
    margin: 0 0 20px 0;
    color: var(--text-secondary);
    max-width: 620px;
    line-height: 1.55;
    font-size: 14px;
  }

  .hero-actions {
    display: flex;
    gap: 12px;
    flex-wrap: wrap;
  }

  .bento-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: var(--gap-grid);
  }

  .feature-card h3,
  .bento-card h3 {
    margin: 0 0 8px 0;
    font-size: 15px;
    font-weight: 600;
  }

  .feature-card p,
  .mcp-note {
    margin: 0 0 14px 0;
    color: var(--text-secondary);
    font-size: 13px;
    line-height: 1.5;
  }

  .section-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 14px;
  }

  .section-header h2 {
    margin: 0;
    font-size: 17px;
  }

  .muted-count {
    font-size: 12px;
    color: var(--text-muted);
  }

  .empty-recent p {
    margin: 0;
    color: var(--text-secondary);
    font-size: 13px;
  }

  .recent-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .recent-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    padding: 12px 14px;
    border-radius: var(--radius-button);
    border: 1px solid var(--border-subtle);
    background-color: var(--bg-elevated);
    color: var(--text-primary);
    cursor: pointer;
    text-align: left;
  }

  .recent-item:hover {
    border-color: var(--border-hover);
    background-color: var(--bg-card-hover);
  }

  .recent-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .recent-name {
    font-size: 13px;
    font-weight: 600;
  }

  .recent-path {
    font-size: 11px;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .recent-time {
    font-size: 12px;
    color: var(--text-secondary);
    flex-shrink: 0;
    margin-left: 12px;
  }

  /* Editor View */
  .editor-view {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }

  .editor-topbar {
    height: 58px;
    flex-shrink: 0;
    background-color: var(--bg-card);
    border-bottom: 1px solid var(--border-subtle);
    padding: 0 20px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }

  .topbar-left,
  .topbar-center {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .step-badge,
  .dim-badge {
    font-size: 12px;
    color: var(--text-secondary);
    background-color: var(--bg-elevated);
    border: 1px solid var(--border-subtle);
    padding: 5px 10px;
    border-radius: var(--radius-badge);
  }

  .topbar-right {
    min-width: 0;
    text-align: right;
  }

  .status-text {
    font-size: 12px;
    color: var(--text-secondary);
  }

  .status-text.error {
    color: #f87171;
  }

  .editor-body {
    flex: 1;
    min-height: 0;
    display: flex;
    overflow: hidden;
  }

  .canvas-stage {
    flex: 1;
    min-width: 0;
    min-height: 0;
    padding: 24px;
    display: grid;
    place-items: center;
    background-color: var(--bg-canvas);
    overflow: hidden;
  }

  .canvas-frame {
    position: relative;
    width: 100%;
    height: 100%;
    display: grid;
    place-items: center;
  }

  .canvas-frame img {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
    border-radius: 10px;
    border: 1px solid var(--border-subtle);
    box-shadow: 0 18px 40px rgba(0, 0, 0, 0.45);
  }

  .processing-overlay {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    background: rgba(17, 18, 22, 0.65);
    backdrop-filter: blur(4px);
    border-radius: 10px;
    font-size: 14px;
    font-weight: 600;
  }

  .empty-canvas {
    text-align: center;
    max-width: 360px;
  }

  .empty-canvas h2 {
    margin: 0 0 8px 0;
    font-size: 20px;
  }

  .empty-canvas p {
    margin: 0 0 18px 0;
    color: var(--text-secondary);
    font-size: 13px;
  }

  /* Right Tool Panel */
  .tool-panel {
    width: 300px;
    flex-shrink: 0;
    background-color: var(--bg-card);
    border-left: 1px solid var(--border-subtle);
    padding: 16px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .tool-section {
    background-color: var(--bg-elevated);
    border: 1px solid var(--border-subtle);
    border-radius: 12px;
    padding: 14px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .tool-section h3 {
    margin: 0;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary);
  }

  .slider-label {
    display: flex;
    justify-content: space-between;
    font-size: 12px;
    color: var(--text-secondary);
  }

  .mono {
    font-variant-numeric: tabular-nums;
    color: var(--text-primary);
  }

  input[type='range'] {
    width: 100%;
    accent-color: var(--accent-primary);
    cursor: pointer;
  }

  .btn-grid-2 {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }

  .btn-grid-3 {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 8px;
  }

  .input-grid-2 {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 11px;
    color: var(--text-secondary);
  }

  .field input {
    width: 100%;
    padding: 7px 8px;
    border-radius: 6px;
    border: 1px solid var(--border-subtle);
    background-color: var(--bg-card);
    color: var(--text-primary);
    font-size: 12px;
  }

  .full-width {
    width: 100%;
  }

  /* MCP View */
  .mcp-status-row {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
  }

  .status-pill {
    font-size: 12px;
    font-weight: 500;
    padding: 6px 12px;
    border-radius: var(--radius-badge);
    background-color: var(--accent-subtle);
    border: 1px solid var(--border-hover);
    color: var(--text-primary);
  }

  .status-pill.muted {
    background-color: var(--bg-elevated);
    border-color: var(--border-subtle);
    color: var(--text-secondary);
  }

  .mcp-list {
    margin: 0;
    padding-left: 18px;
    color: var(--text-secondary);
    font-size: 13px;
    line-height: 1.8;
  }

  code {
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 12px;
    color: #c7d2fe;
  }
</style>
