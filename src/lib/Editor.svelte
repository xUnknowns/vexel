<script>
  import { invoke, convertFileSrc } from '@tauri-apps/api/core';
  import {
    ArrowLeft, FolderOpen, Download, Undo2, Redo2,
    SlidersHorizontal, Sparkles, RotateCcw, ImageIcon, Bot,
    ChevronLeft, ChevronRight, Columns2, Crop, Maximize2,
    RotateCw, FlipHorizontal, FlipVertical, RefreshCcw
  } from 'lucide-svelte';

  let {
    history       = [],
    historyIndex  = -1,
    currentPath   = null,
    canUndo       = false,
    canRedo       = false,
    imgWidth      = 0,
    imgHeight     = 0,
    projectName   = 'Untitled Design',
    makeTempPath,
    onUndo,
    onRedo,
    onPushHistory,
    onGoHome,
    onExport,
    onProjectNameChange
  } = $props();

  // ── Editor local state ───────────────────────────────────────────────────
  let activeTool      = $state(null);     // 'adjust' | 'filters' | 'transform' | 'image' | 'mcp'
  let drawerOpen      = $state(false);
  let isProcessing    = $state(false);
  let statusMsg       = $state('Ready');
  let errorMsg        = $state('');
  let comparing       = $state(false);
  let cacheBuster     = $state(0);
  let zoom            = $state(1.0);
  let canvasArea;     // bind:this

  // Adjust tool
  let brightness = $state(0);
  let contrast   = $state(0);
  let saturation = $state(0);

  // Transform tool
  let cropX = $state(0), cropY = $state(0), cropW = $state(400), cropH = $state(400);
  let resW = $state(800), resH = $state(600);

  // Derived
  let previewUrl  = $derived(
    currentPath
      ? (comparing && history.length > 0
          ? `${convertFileSrc(history[0])}?v=orig`
          : `${convertFileSrc(currentPath)}?v=${cacheBuster}`)
      : ''
  );
  let stepLabel   = $derived(`Step ${historyIndex + 1} / ${history.length}`);
  let zoomPct     = $derived(Math.round(zoom * 100));

  // ── Tool rail definition ──────────────────────────────────────────────
  const tools = [
    { id: 'adjust',    label: 'Adjust',    Icon: SlidersHorizontal },
    { id: 'filters',   label: 'Filters',   Icon: Sparkles          },
    { id: 'transform', label: 'Transform', Icon: RotateCcw         },
    { id: 'image',     label: 'Image',     Icon: ImageIcon         },
    { id: 'mcp',       label: 'MCP AI',    Icon: Bot               },
  ];

  const filters = [
    { id: 'grayscale', label: 'Grayscale', style: 'background: linear-gradient(135deg,#555,#aaa);' },
    { id: 'noir',      label: 'Noir',      style: 'background: linear-gradient(135deg,#111,#444);' },
    { id: 'warm',      label: 'Warm',      style: 'background: linear-gradient(135deg,#b45309,#f97316);' },
    { id: 'cool',      label: 'Cool',      style: 'background: linear-gradient(135deg,#1d4ed8,#38bdf8);' },
    { id: 'blur',      label: 'Blur',      style: 'background: linear-gradient(135deg,#4f46e5,#818cf8); filter:blur(3px);' },
    { id: 'invert',    label: 'Invert',    style: 'background: linear-gradient(135deg,#e0f2fe,#1e1b4b);' },
  ];

  // ── Helpers ──────────────────────────────────────────────────────────
  function selectTool(id) {
    if (activeTool === id) {
      drawerOpen = !drawerOpen;
    } else {
      activeTool = id;
      drawerOpen = true;
    }
  }

  function onImageLoad(e) {
    const img = e.currentTarget;
    // set initial resize/crop defaults from natural dims (use imgWidth/imgHeight from parent)
    resW = imgWidth || img.naturalWidth;
    resH = imgHeight || img.naturalHeight;
    cropW = Math.max(1, Math.floor((imgWidth || img.naturalWidth) / 2));
    cropH = Math.max(1, Math.floor((imgHeight || img.naturalHeight) / 2));
    fitToScreen();
  }

  function fitToScreen() {
    if (!canvasArea || !imgWidth || !imgHeight) return;
    const { clientWidth: cw, clientHeight: ch } = canvasArea;
    const scale = Math.min((cw - 80) / imgWidth, (ch - 80) / imgHeight);
    zoom = Math.max(0.05, Math.min(scale, 3));
  }

  // Wheel zoom via action (non-passive)
  function wheelZoom(node) {
    const handler = (e) => {
      e.preventDefault();
      const delta = e.deltaY > 0 ? -0.08 : 0.08;
      zoom = Math.max(0.05, Math.min(3.0, zoom + delta));
    };
    node.addEventListener('wheel', handler, { passive: false });
    return { destroy: () => node.removeEventListener('wheel', handler) };
  }

  // Global keyboard shortcuts
  function handleKeydown(e) {
    if ((e.ctrlKey || e.metaKey) && e.key === 'z') { e.preventDefault(); onUndo(); }
    if ((e.ctrlKey || e.metaKey) && e.key === 'y') { e.preventDefault(); onRedo(); }
    if (e.key === 'Escape') drawerOpen = false;
  }

  // ── Rust command runner ───────────────────────────────────────────────
  async function runCmd(commandName, buildArgs, label, w, h) {
    if (!currentPath || isProcessing) return;
    isProcessing = true;
    errorMsg  = '';
    statusMsg = `${label}…`;
    try {
      const dest = await makeTempPath(currentPath);
      const args = buildArgs(currentPath, dest);
      const result = await invoke(commandName, args);
      onPushHistory(result, w, h);
      cacheBuster += 1;
      statusMsg = `${label} applied`;
    } catch (e) {
      errorMsg  = String(e);
      statusMsg = `${label} failed`;
    } finally {
      isProcessing = false;
    }
  }

  // ── Tool actions ──────────────────────────────────────────────────────
  function applyAdjust() {
    runCmd(
      'adjust_brightness_contrast_saturation',
      (s, d) => ({ sourcePath: s, destPath: d, brightness: Number(brightness), contrast: Number(contrast), saturation: Number(saturation) }),
      'Adjust'
    ).then(() => { brightness = 0; contrast = 0; saturation = 0; });
  }
  function resetAdjust() { brightness = 0; contrast = 0; saturation = 0; }

  function applyFilter(filterId, label) {
    runCmd('apply_filter', (s, d) => ({ sourcePath: s, destPath: d, filter: filterId }), label);
  }

  function applyTransform(action, label) {
    runCmd('transform_image', (s, d) => ({ sourcePath: s, destPath: d, action }), label);
  }

  function applyCrop() {
    runCmd(
      'crop_image',
      (s, d) => ({ sourcePath: s, destPath: d, x: Number(cropX), y: Number(cropY), width: Number(cropW), height: Number(cropH) }),
      'Crop'
    );
  }

  function applyResize() {
    const nw = Number(resW);
    const nh = Number(resH);
    runCmd(
      'resize_image',
      (s, d) => ({ sourcePath: s, destPath: d, newWidth: nw, newHeight: nh }),
      'Resize',
      nw,
      nh
    );
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="editor-shell">

  <!-- ── Top Bar (48px) ───────────────────────────────────────── -->
  <header class="top-bar">
    <!-- Left -->
    <div class="tb-left">
      <button class="btn-ghost icon-btn" onclick={onGoHome} title="Back to Home">
        <ArrowLeft size={15} strokeWidth={2} />
        <span>Home</span>
      </button>

      <div class="tb-divider"></div>

      <button class="btn-ghost icon-btn sm" onclick={onUndo} disabled={!canUndo} title="Undo (Ctrl+Z)">
        <Undo2 size={14} strokeWidth={2} /> Undo
      </button>
      <button class="btn-ghost icon-btn sm" onclick={onRedo} disabled={!canRedo} title="Redo (Ctrl+Y)">
        <Redo2 size={14} strokeWidth={2} /> Redo
      </button>
    </div>

    <!-- Center -->
    <div class="tb-center">
      <input
        class="project-name"
        type="text"
        value={projectName}
        oninput={(e) => onProjectNameChange(e.currentTarget.value)}
        spellcheck="false"
      />
      {#if imgWidth}
        <span class="dim-label">{imgWidth} × {imgHeight} px</span>
      {/if}
    </div>

    <!-- Right -->
    <div class="tb-right">
      {#if currentPath}
        <button
          class="btn-ghost icon-btn sm compare-btn"
          onmousedown={() => (comparing = true)}
          onmouseup={() => (comparing = false)}
          onmouseleave={() => (comparing = false)}
          title="Hold to compare with original"
        >
          <Columns2 size={14} strokeWidth={2} /> Compare
        </button>
      {/if}

      <button
        class="btn-primary icon-btn"
        onclick={onExport}
        disabled={!currentPath || isProcessing}
        title="Export image"
      >
        <Download size={14} strokeWidth={2} /> Export
      </button>
    </div>
  </header>

  <!-- ── Editor Body ───────────────────────────────────────────── -->
  <div class="editor-body">

    <!-- ── Left Tool Rail (68px) ─────────────────────────── -->
    <aside class="tool-rail">
      {#each tools as t}
        <button
          class="rail-btn"
          class:active={activeTool === t.id && drawerOpen}
          onclick={() => selectTool(t.id)}
          title={t.label}
        >
          <svelte:component this={t.Icon} size={20} strokeWidth={1.75} />
          <span>{t.label}</span>
        </button>
      {/each}
    </aside>

    <!-- ── Collapsible Tool Drawer ────────────────────────── -->
    <div class="tool-drawer" class:open={drawerOpen && activeTool}>

      {#if drawerOpen && activeTool}
        <!-- Drawer content -->
        <div class="drawer-inner">
          <div class="drawer-title">
            {tools.find(t => t.id === activeTool)?.label}
          </div>

          <!-- ADJUST ── -->
          {#if activeTool === 'adjust'}
            <div class="panel">
              <div class="slider-row">
                <label class="slider-label">
                  <span>Brightness</span>
                  <span class="val">{brightness}</span>
                </label>
                <input type="range" min="-100" max="100" step="1" bind:value={brightness} disabled={!currentPath} />
              </div>

              <div class="slider-row">
                <label class="slider-label">
                  <span>Contrast</span>
                  <span class="val">{contrast}</span>
                </label>
                <input type="range" min="-100" max="100" step="1" bind:value={contrast} disabled={!currentPath} />
              </div>

              <div class="slider-row">
                <label class="slider-label">
                  <span>Saturation</span>
                  <span class="val">{saturation}</span>
                </label>
                <input type="range" min="-100" max="100" step="1" bind:value={saturation} disabled={!currentPath} />
              </div>

              <div class="btn-row">
                <button class="btn-ghost fw" onclick={resetAdjust} disabled={!currentPath}>Reset</button>
                <button class="btn-primary fw" onclick={applyAdjust} disabled={!currentPath || isProcessing}>Apply</button>
              </div>
            </div>

          <!-- FILTERS ── -->
          {:else if activeTool === 'filters'}
            <div class="panel">
              <!-- Original "tile" -->
              <button class="filter-card" onclick={() => {}} title="Original — no filter applied">
                <div class="filter-thumb" style="background: conic-gradient(from 90deg at 40% 40%, #6366f1, #ec4899, #f97316, #6366f1);"></div>
                <span>Original</span>
              </button>

              {#each filters as f}
                <button
                  class="filter-card"
                  onclick={() => applyFilter(f.id, f.label)}
                  disabled={!currentPath || isProcessing}
                  title="Apply {f.label} filter"
                >
                  <div class="filter-thumb" style={f.style}></div>
                  <span>{f.label}</span>
                </button>
              {/each}
            </div>

          <!-- TRANSFORM ── -->
          {:else if activeTool === 'transform'}
            <div class="panel">
              <div class="section-title">Rotate & Flip</div>
              <div class="btn-grid-2">
                <button class="btn-ghost" onclick={() => applyTransform('rotate90','Rotate 90°')} disabled={!currentPath || isProcessing}>
                  <RotateCw size={14} strokeWidth={2} /> 90°
                </button>
                <button class="btn-ghost" onclick={() => applyTransform('rotate180','Rotate 180°')} disabled={!currentPath || isProcessing}>
                  <RefreshCcw size={14} strokeWidth={2} /> 180°
                </button>
                <button class="btn-ghost" onclick={() => applyTransform('flip_h','Flip H')} disabled={!currentPath || isProcessing}>
                  <FlipHorizontal size={14} strokeWidth={2} /> Flip H
                </button>
                <button class="btn-ghost" onclick={() => applyTransform('flip_v','Flip V')} disabled={!currentPath || isProcessing}>
                  <FlipVertical size={14} strokeWidth={2} /> Flip V
                </button>
              </div>

              <div class="section-title" style="margin-top:16px;">Crop</div>
              <div class="input-grid-2">
                <label class="field"><span>X</span> <input type="number" min="0" bind:value={cropX} disabled={!currentPath} /></label>
                <label class="field"><span>Y</span> <input type="number" min="0" bind:value={cropY} disabled={!currentPath} /></label>
                <label class="field"><span>W</span> <input type="number" min="1" bind:value={cropW} disabled={!currentPath} /></label>
                <label class="field"><span>H</span> <input type="number" min="1" bind:value={cropH} disabled={!currentPath} /></label>
              </div>
              <button class="btn-ghost fw" onclick={applyCrop} disabled={!currentPath || isProcessing}>
                <Crop size={13} strokeWidth={2} /> Apply Crop
              </button>

              <div class="section-title" style="margin-top:16px;">Resize</div>
              <div class="input-grid-2">
                <label class="field"><span>W px</span> <input type="number" min="1" bind:value={resW} disabled={!currentPath} /></label>
                <label class="field"><span>H px</span> <input type="number" min="1" bind:value={resH} disabled={!currentPath} /></label>
              </div>
              <button class="btn-ghost fw" onclick={applyResize} disabled={!currentPath || isProcessing}>Apply Resize</button>
            </div>

          <!-- IMAGE ── -->
          {:else if activeTool === 'image'}
            <div class="panel">
              <p class="panel-note">Replace the current canvas or open a new image.</p>
              <button class="btn-ghost fw" disabled={isProcessing}
                onclick={async () => {
                  const { open: openDlg } = await import('@tauri-apps/plugin-dialog');
                  const sel = await openDlg({ multiple: false, filters: [{ name: 'Images', extensions: ['png','jpg','jpeg','webp','bmp'] }] });
                  if (sel) { const { invoke: inv } = await import('@tauri-apps/api/core'); const dims = await inv('get_image_dimensions', { path: sel }); onPushHistory(sel, dims.width, dims.height); cacheBuster += 1; }
                }}
              >
                <FolderOpen size={13} strokeWidth={2} /> Open / Replace
              </button>
            </div>

          <!-- MCP AI ── -->
          {:else if activeTool === 'mcp'}
            <div class="panel">
              <div class="mcp-status">
                <span class="status-dot standby"></span>
                <span>Bridge: Standby</span>
              </div>
              <p class="panel-note">
                MCP bridge to Claude Desktop is not yet active. Configure it from
                <strong>Home → AI (MCP)</strong>.
              </p>
              <button class="btn-ghost fw" disabled>MCP — Coming Soon</button>
            </div>
          {/if}
        </div>
      {/if}

      <!-- Toggle arrow on the right edge of drawer -->
      <button
        class="drawer-toggle"
        onclick={() => { drawerOpen = !drawerOpen; }}
        title={drawerOpen ? 'Collapse panel' : 'Expand panel'}
      >
        {#if drawerOpen}
          <ChevronLeft size={14} strokeWidth={2} />
        {:else}
          <ChevronRight size={14} strokeWidth={2} />
        {/if}
      </button>
    </div>

    <!-- ── Canvas Area ────────────────────────────────────── -->
    <div class="canvas-area" bind:this={canvasArea} use:wheelZoom>
      {#if currentPath}
        <div
          class="canvas-frame"
          style="width:{imgWidth * zoom}px; height:{imgHeight * zoom}px;"
        >
          <img
            src={previewUrl}
            alt="Canvas"
            width={imgWidth * zoom}
            height={imgHeight * zoom}
            style="display:block; width:100%; height:100%; object-fit:fill;"
            onload={onImageLoad}
            draggable="false"
          />
          {#if isProcessing}
            <div class="processing-overlay">
              <span>Processing in Rust…</span>
            </div>
          {/if}
        </div>
      {:else}
        <div class="empty-canvas">
          <ImageIcon size={42} strokeWidth={1.1} />
          <h3>No Image Loaded</h3>
          <p>Go back to Home and open an image or create a canvas.</p>
          <button class="btn-ghost" onclick={onGoHome}>
            <ArrowLeft size={14} strokeWidth={2} /> Back to Home
          </button>
        </div>
      {/if}

      <!-- ── Bottom Bar ──────────────────────────────────── -->
      <div class="bottom-bar">
        <span class="step-pill">
          {stepLabel}
          {#if errorMsg}
            <span class="err-dot" title={errorMsg}>⚠</span>
          {/if}
        </span>

        <div class="zoom-controls">
          <button class="zoom-btn" onclick={() => zoom = Math.max(0.05, zoom - 0.1)} title="Zoom out">−</button>
          <input
            type="range"
            min="5"
            max="300"
            step="1"
            value={zoomPct}
            oninput={(e) => zoom = Number(e.currentTarget.value) / 100}
            style="width:100px;"
          />
          <span class="zoom-pct">{zoomPct}%</span>
          <button class="zoom-btn" onclick={() => zoom = Math.min(3.0, zoom + 0.1)} title="Zoom in">+</button>
          <button class="zoom-btn" onclick={fitToScreen} title="Fit to screen">
            <Maximize2 size={12} strokeWidth={2} />
          </button>
        </div>
      </div>
    </div>

  </div><!-- /editor-body -->
</div><!-- /editor-shell -->

<style>
  /* ── Shell ──────────────────────────────────────────────── */
  .editor-shell {
    display: flex;
    flex-direction: column;
    height: 100vh;
    width: 100vw;
    overflow: hidden;
    background: var(--bg-canvas);
  }

  /* ── Top Bar ────────────────────────────────────────────── */
  .top-bar {
    height: 48px;
    flex-shrink: 0;
    background: var(--bg-sidebar);
    border-bottom: 1px solid var(--border);
    display: flex;
    align-items: center;
    padding: 0 12px;
    gap: 8px;
  }

  .tb-left, .tb-right {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: 1;
  }
  .tb-right { justify-content: flex-end; }
  .tb-center {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-shrink: 0;
  }

  .tb-divider {
    width: 1px;
    height: 20px;
    background: var(--border);
    margin: 0 2px;
  }

  .icon-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    white-space: nowrap;
  }
  .icon-btn.sm { font-size: 12px; padding: 5px 10px; }

  .project-name {
    background: transparent;
    border: 1px solid transparent;
    border-radius: var(--r-sm);
    color: var(--text-primary);
    font-size: 13px;
    font-weight: 600;
    padding: 4px 8px;
    max-width: 200px;
    text-align: center;
    width: auto;
    min-width: 120px;
  }
  .project-name:hover {
    border-color: var(--border);
    background: var(--bg-input);
  }
  .project-name:focus {
    border-color: var(--accent);
    background: var(--bg-input);
    outline: none;
  }

  .dim-label {
    font-size: 11px;
    color: var(--text-muted);
    background: var(--bg-elevated);
    padding: 3px 8px;
    border-radius: 999px;
    border: 1px solid var(--border);
    white-space: nowrap;
  }

  .compare-btn { opacity: 0.75; }
  .compare-btn:hover { opacity: 1; }

  /* ── Editor Body ────────────────────────────────────────── */
  .editor-body {
    flex: 1;
    display: flex;
    min-height: 0;
    overflow: hidden;
  }

  /* ── Left Tool Rail ─────────────────────────────────────── */
  .tool-rail {
    width: 68px;
    flex-shrink: 0;
    background: var(--bg-sidebar);
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 10px 0;
    gap: 2px;
  }

  .rail-btn {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    width: 56px;
    padding: 10px 4px;
    border-radius: var(--r-sm);
    background: transparent;
    border: 1px solid transparent;
    color: var(--text-secondary);
    font-size: 10px;
    font-weight: 500;
  }
  .rail-btn:hover:not(:disabled) {
    background: var(--bg-elevated);
    color: var(--text-primary);
    border-color: var(--border);
  }
  .rail-btn.active {
    background: var(--accent-subtle);
    color: var(--text-primary);
    border-color: var(--border-active);
  }

  /* ── Tool Drawer ────────────────────────────────────────── */
  .tool-drawer {
    position: relative;
    width: 0;
    flex-shrink: 0;
    overflow: visible;
    background: var(--bg-card);
    border-right: 1px solid var(--border);
    transition: width 0.22s cubic-bezier(0.4,0,0.2,1);
    display: flex;
  }
  .tool-drawer.open {
    width: 268px;
    overflow: hidden;
  }

  .drawer-inner {
    width: 268px;
    flex-shrink: 0;
    padding: 16px 14px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .drawer-title {
    font-size: 12px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-secondary);
    padding-bottom: 10px;
    border-bottom: 1px solid var(--border);
  }

  .drawer-toggle {
    position: absolute;
    right: -14px;
    top: 50%;
    transform: translateY(-50%);
    width: 14px;
    height: 48px;
    background: var(--bg-elevated);
    border: 1px solid var(--border);
    border-left: none;
    border-radius: 0 6px 6px 0;
    display: grid;
    place-items: center;
    color: var(--text-secondary);
    z-index: 10;
    padding: 0;
  }
  .drawer-toggle:hover:not(:disabled) {
    background: var(--bg-card-hover);
    color: var(--text-primary);
    transform: translateY(-50%) scale(1.0);
  }

  /* ── Panel internals ───────────────────────────────────── */
  .panel {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .slider-row  { display: flex; flex-direction: column; gap: 6px; }
  .slider-label {
    display: flex;
    justify-content: space-between;
    font-size: 12px;
    color: var(--text-secondary);
  }
  .val { font-variant-numeric: tabular-nums; color: var(--text-primary); }

  .btn-row { display: flex; gap: 8px; }
  .fw { width: 100%; display: flex; align-items: center; justify-content: center; gap: 6px; }

  .section-title {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-secondary);
    margin-bottom: 2px;
  }

  .btn-grid-2 {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px;
  }
  .btn-grid-2 button {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 5px;
    font-size: 12px;
  }

  .input-grid-2 {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px;
    margin-bottom: 6px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 11px;
    color: var(--text-secondary);
  }

  /* Filters */
  .filter-card {
    display: flex;
    align-items: center;
    gap: 10px;
    background: var(--bg-elevated);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    padding: 8px 10px;
    color: var(--text-secondary);
    font-size: 12px;
    font-weight: 500;
  }
  .filter-card:hover:not(:disabled) {
    border-color: var(--border-active);
    color: var(--text-primary);
    background: var(--bg-card-hover);
  }
  .filter-thumb {
    width: 36px;
    height: 24px;
    border-radius: 4px;
    flex-shrink: 0;
  }

  /* MCP panel */
  .mcp-status {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--text-secondary);
  }
  .status-dot { width: 7px; height: 7px; border-radius: 50%; flex-shrink: 0; }
  .status-dot.standby { background: #f59e0b; box-shadow: 0 0 6px rgba(245,158,11,0.5); }

  .panel-note {
    font-size: 12px;
    color: var(--text-secondary);
    line-height: 1.55;
  }
  .panel-note strong { color: var(--text-primary); }

  /* ── Canvas Area ────────────────────────────────────────── */
  .canvas-area {
    flex: 1;
    min-width: 0;
    min-height: 0;
    position: relative;
    overflow: auto;
    display: flex;
    align-items: center;
    justify-content: center;
    background:
      radial-gradient(circle at center, #161920 0%, #0d0f12 100%);
    background-size: 100% 100%;
  }

  /* Subtle checker for transparency indicator */
  .canvas-area::before {
    content: '';
    position: absolute;
    inset: 0;
    background-image:
      linear-gradient(45deg, #1a1d26 25%, transparent 25%),
      linear-gradient(-45deg, #1a1d26 25%, transparent 25%),
      linear-gradient(45deg, transparent 75%, #1a1d26 75%),
      linear-gradient(-45deg, transparent 75%, #1a1d26 75%);
    background-size: 24px 24px;
    background-position: 0 0, 0 12px, 12px -12px, -12px 0px;
    opacity: 0.18;
    pointer-events: none;
  }

  .canvas-frame {
    position: relative;
    flex-shrink: 0;
    border-radius: 4px;
    overflow: hidden;
    box-shadow:
      0 0 0 1px rgba(255,255,255,0.06),
      0 24px 60px rgba(0,0,0,0.6);
  }

  .processing-overlay {
    position: absolute;
    inset: 0;
    background: rgba(13,15,18,0.7);
    backdrop-filter: blur(4px);
    display: grid;
    place-items: center;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-secondary);
  }

  .empty-canvas {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
    text-align: center;
    color: var(--text-secondary);
    max-width: 320px;
  }
  .empty-canvas h3 { font-size: 17px; color: var(--text-primary); }
  .empty-canvas p  { font-size: 13px; }

  /* ── Bottom Bar ─────────────────────────────────────────── */
  .bottom-bar {
    position: absolute;
    bottom: 14px;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 14px;
    background: var(--bg-elevated);
    border: 1px solid var(--border);
    border-radius: 999px;
    padding: 6px 14px;
    pointer-events: auto;
  }

  .step-pill {
    font-size: 12px;
    color: var(--text-secondary);
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .err-dot { color: #f87171; font-size: 11px; }

  .zoom-controls {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .zoom-btn {
    width: 22px; height: 22px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    background: var(--bg-card-hover);
    border: 1px solid var(--border);
    color: var(--text-secondary);
    font-size: 14px;
    line-height: 1;
    padding: 0;
  }
  .zoom-btn:hover:not(:disabled) {
    background: var(--accent-subtle);
    border-color: var(--border-active);
    color: var(--text-primary);
  }
  .zoom-pct {
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    color: var(--text-primary);
    min-width: 36px;
    text-align: center;
  }
</style>
