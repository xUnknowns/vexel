<script>
  import { invoke, convertFileSrc } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import {
    ArrowLeft, Download, Undo2, Redo2, FolderOpen,
    SlidersHorizontal, Sparkles, RotateCcw, ImageIcon, Bot, Type,
    ChevronLeft, ChevronRight, Columns2, Crop, Maximize2,
    RotateCw, FlipHorizontal, FlipVertical, RefreshCcw, Plug
  } from 'lucide-svelte';
  import ExportModal from './ExportModal.svelte';

  let {
    history       = [],
    historyIndex  = -1,
    currentPath   = null,
    canUndo       = false,
    canRedo       = false,
    imgWidth      = 0,
    imgHeight     = 0,
    projectName   = 'Untitled Design',
    mcpIsActive   = false,
    makeTempPath,
    onUndo,
    onRedo,
    onPushHistory,
    onGoHome,
    onProjectNameChange,
  } = $props();

  // ── Editor state ───────────────────────────────────────────────────────
  let activeTool        = $state(null);
  let drawerOpen        = $state(false);
  let isProcessing      = $state(false);
  let statusMsg         = $state('Ready');
  let errorMsg          = $state('');
  let comparing         = $state(false);
  let cacheBuster       = $state(0);
  let zoom              = $state(1.0);
  let showExportModal   = $state(false);
  let canvasArea;

  // ── Filter: non-destructive base tracking ──────────────────────────────
  // filterBasePath is the image state *before* the current filter session.
  // All filter applications use this as source so filters don't stack.
  let filterBasePath    = $state(null);

  // ── Adjust sliders ──────────────────────────────────────────────────────
  let brightness = $state(0), contrast = $state(0), saturation = $state(0);

  // ── Transform / resize / crop inputs ──────────────────────────────────
  let cropX = $state(0), cropY = $state(0), cropW = $state(400), cropH = $state(400);
  let resW  = $state(800), resH = $state(600);

  // ── Text tool state ────────────────────────────────────────────────────
  let textContent  = $state('Your text here');
  let fontStyle    = $state('modern_sans');
  let fontSize     = $state(48);
  let textColorHex = $state('#FFFFFF');
  let textX        = $state(40);
  let textY        = $state(80);

  const fontStyles = [
    { id: 'modern_sans',   label: 'Modern Sans',   sample: 'Aa' },
    { id: 'classic_serif', label: 'Classic Serif',  sample: 'Aa' },
    { id: 'tech_mono',     label: 'Tech Mono',      sample: 'Aa' },
    { id: 'bold_display',  label: 'Bold Display',   sample: 'Aa' },
    { id: 'script',        label: 'Script',          sample: 'Aa' },
  ];
  const quickColors = ['#FFFFFF','#000000','#EF4444','#F97316','#6366F1','#10B981'];

  // ── Derived ────────────────────────────────────────────────────────────
  let previewUrl = $derived(
    currentPath
      ? (comparing && history.length > 0
          ? `${convertFileSrc(history[0])}?v=orig`
          : `${convertFileSrc(currentPath)}?v=${cacheBuster}`)
      : ''
  );
  let stepLabel  = $derived(`Step ${historyIndex + 1} / ${history.length}`);
  let zoomPct    = $derived(Math.round(zoom * 100));
  let hasImage   = $derived(Boolean(currentPath));

  const tools = [
    { id: 'adjust',    label: 'Adjust',    Icon: SlidersHorizontal },
    { id: 'filters',   label: 'Filters',   Icon: Sparkles          },
    { id: 'transform', label: 'Transform', Icon: RotateCcw         },
    { id: 'text',      label: 'Text',      Icon: Type              },
    { id: 'image',     label: 'Image',     Icon: ImageIcon         },
    { id: 'mcp',       label: 'MCP AI',    Icon: Bot               },
  ];

  const filters = [
    { id: 'grayscale', label: 'Grayscale', style: 'background:linear-gradient(135deg,#555,#aaa);' },
    { id: 'noir',      label: 'Noir',      style: 'background:linear-gradient(135deg,#111,#444);' },
    { id: 'warm',      label: 'Warm',      style: 'background:linear-gradient(135deg,#b45309,#f97316);' },
    { id: 'cool',      label: 'Cool',      style: 'background:linear-gradient(135deg,#1d4ed8,#38bdf8);' },
    { id: 'blur',      label: 'Blur',      style: 'background:linear-gradient(135deg,#4f46e5,#818cf8);filter:blur(3px);' },
    { id: 'invert',    label: 'Invert',    style: 'background:linear-gradient(135deg,#e0f2fe,#1e1b4b);' },
  ];

  // ── MCP event listener — Rust notifies Svelte after remote edit ────────
  $effect(() => {
    const unlisten = listen('mcp-image-updated', (event) => {
      const newPath = event.payload;
      onPushHistory(newPath, null, null);
      filterBasePath = null; // reset filter base after MCP edit
      cacheBuster += 1;
      statusMsg = 'Canvas updated via MCP';
    });
    return () => { unlisten.then(fn => fn()); };
  });

  // ── Keyboard shortcuts ─────────────────────────────────────────────────
  function handleKeydown(e) {
    if ((e.ctrlKey || e.metaKey) && e.key === 'z') { e.preventDefault(); onUndo(); filterBasePath = null; }
    if ((e.ctrlKey || e.metaKey) && e.key === 'y') { e.preventDefault(); onRedo(); filterBasePath = null; }
    if (e.key === 'Escape') { drawerOpen = false; showExportModal = false; }
  }

  // ── Tool rail selection ────────────────────────────────────────────────
  function selectTool(id) {
    if (activeTool === id) { drawerOpen = !drawerOpen; }
    else { activeTool = id; drawerOpen = true; }
  }

  // ── Canvas helpers ─────────────────────────────────────────────────────
  function onImageLoad(e) {
    resW = imgWidth  || e.currentTarget.naturalWidth;
    resH = imgHeight || e.currentTarget.naturalHeight;
    cropW = Math.max(1, Math.floor((imgWidth  || e.currentTarget.naturalWidth)  / 2));
    cropH = Math.max(1, Math.floor((imgHeight || e.currentTarget.naturalHeight) / 2));
    textX = 40;
    textY = Math.max(1, Math.round((imgHeight || e.currentTarget.naturalHeight) * 0.1));
    fitToScreen();
  }
  function fitToScreen() {
    if (!canvasArea || !imgWidth || !imgHeight) return;
    const { clientWidth: cw, clientHeight: ch } = canvasArea;
    zoom = Math.max(0.05, Math.min((cw - 80) / imgWidth, (ch - 80) / imgHeight, 3));
  }
  function wheelZoom(node) {
    const h = (e) => {
      e.preventDefault();
      zoom = Math.max(0.05, Math.min(3.0, zoom + (e.deltaY > 0 ? -0.08 : 0.08)));
    };
    node.addEventListener('wheel', h, { passive: false });
    return { destroy: () => node.removeEventListener('wheel', h) };
  }

  // ── Generic command runner ─────────────────────────────────────────────
  async function runCmd(commandName, buildArgs, label, w, h) {
    if (!currentPath || isProcessing) return;
    isProcessing = true; errorMsg = ''; statusMsg = `${label}…`;
    try {
      const dest = await makeTempPath(currentPath);
      const result = await invoke(commandName, buildArgs(currentPath, dest));
      onPushHistory(result, w, h);
      cacheBuster += 1;
      statusMsg = `${label} applied`;
    } catch (e) {
      errorMsg  = String(e);
      statusMsg = `${label} failed`;
    } finally { isProcessing = false; }
  }

  // ── Filter commands (non-destructive: always from base) ────────────────
  async function handleFilterClick(filterId, label) {
    if (!hasImage || isProcessing) return;
    // Save base on first filter of this session
    if (!filterBasePath) filterBasePath = currentPath;
    const source = filterBasePath;
    isProcessing = true; errorMsg = ''; statusMsg = `${label}…`;
    try {
      const dest   = await makeTempPath(source);
      const result = await invoke('apply_filter', { sourcePath: source, destPath: dest, filter: filterId });
      onPushHistory(result);
      cacheBuster += 1;
      statusMsg = `${label} applied`;
    } catch (e) {
      errorMsg  = String(e);
      statusMsg = `${label} failed`;
    } finally { isProcessing = false; }
  }

  function handleOriginalFilter() {
    if (!filterBasePath || !hasImage) return;
    // Push the saved base as a new history entry to restore clean state
    onPushHistory(filterBasePath);
    cacheBuster += 1;
    filterBasePath = null;
    statusMsg = 'Restored original';
  }

  // ── Adjust ─────────────────────────────────────────────────────────────
  function applyAdjust() {
    filterBasePath = null; // non-filter edit resets filter session
    runCmd(
      'adjust_brightness_contrast_saturation',
      (s, d) => ({ sourcePath: s, destPath: d, brightness: Number(brightness), contrast: Number(contrast), saturation: Number(saturation) }),
      'Adjust'
    ).then(() => { brightness = 0; contrast = 0; saturation = 0; });
  }
  function resetAdjust() { brightness = 0; contrast = 0; saturation = 0; }

  // ── Transform ──────────────────────────────────────────────────────────
  function applyTransform(action, label) {
    filterBasePath = null;
    runCmd('transform_image', (s,d) => ({ sourcePath:s, destPath:d, action }), label);
  }
  function applyCrop() {
    filterBasePath = null;
    runCmd('crop_image', (s,d) => ({ sourcePath:s, destPath:d, x:Number(cropX), y:Number(cropY), width:Math.max(1,Number(cropW)), height:Math.max(1,Number(cropH)) }), 'Crop');
  }
  function applyResize() {
    filterBasePath = null;
    const nw = Math.max(1, Number(resW)), nh = Math.max(1, Number(resH));
    runCmd('resize_image', (s,d) => ({ sourcePath:s, destPath:d, newWidth:nw, newHeight:nh }), 'Resize', nw, nh);
  }

  // ── Text ───────────────────────────────────────────────────────────────
  function applyText() {
    filterBasePath = null;
    runCmd(
      'add_text_layer',
      (s,d) => ({ sourcePath:s, destPath:d, text:textContent, fontStyle, fontSize:Number(fontSize), colorHex:textColorHex, posX:Number(textX), posY:Number(textY) }),
      'Add Text'
    );
  }
  function setTextAlign(preset) {
    if (preset === 'top')    { textY = Math.max(1, Math.round(imgHeight * 0.1)); }
    if (preset === 'center') { textY = Math.round(imgHeight * 0.5); }
    if (preset === 'bottom') { textY = Math.min(imgHeight - 10, Math.round(imgHeight * 0.85)); }
  }

  // ── MCP helpers ────────────────────────────────────────────────────────
  let connectResult = $state('');
  async function connectClaude() {
    try {
      connectResult = await invoke('connect_claude_desktop');
    } catch (e) { connectResult = `Error: ${e}`; }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="editor-shell">

  <!-- ── Top Bar (48px) ──────────────────────────────────────── -->
  <header class="top-bar">
    <div class="tb-left">
      <button class="btn-ghost icon-btn" onclick={onGoHome} title="Back to Home (keeps session)">
        <ArrowLeft size={14} strokeWidth={2} /> Home
      </button>
      <div class="tb-divider"></div>
      <button class="btn-ghost icon-btn sm" onclick={() => { onUndo(); filterBasePath = null; }} disabled={!canUndo} title="Undo (Ctrl+Z)">
        <Undo2 size={13} strokeWidth={2} /> Undo
      </button>
      <button class="btn-ghost icon-btn sm" onclick={() => { onRedo(); filterBasePath = null; }} disabled={!canRedo} title="Redo (Ctrl+Y)">
        <Redo2 size={13} strokeWidth={2} /> Redo
      </button>
    </div>

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

    <div class="tb-right">
      {#if hasImage}
        <button
          class="btn-ghost icon-btn sm"
          onmousedown={() => (comparing = true)}
          onmouseup={() => (comparing = false)}
          onmouseleave={() => (comparing = false)}
          title="Hold to compare with original"
        >
          <Columns2 size={13} strokeWidth={2} /> Compare
        </button>
      {/if}
      <button
        class="btn-primary icon-btn"
        onclick={() => (showExportModal = true)}
        disabled={!hasImage || isProcessing}
        title="Export image"
      >
        <Download size={14} strokeWidth={2} /> Export
      </button>
    </div>
  </header>

  <!-- ── Editor Body ─────────────────────────────────────────── -->
  <div class="editor-body">

    <!-- Tool Rail (68px) -->
    <aside class="tool-rail">
      {#each tools as t}
        <button
          class="rail-btn"
          class:active={activeTool === t.id && drawerOpen}
          onclick={() => selectTool(t.id)}
          title={t.label}
        >
          <svelte:component this={t.Icon} size={19} strokeWidth={1.75} />
          <span>{t.label}</span>
        </button>
      {/each}
    </aside>

    <!-- Collapsible Tool Drawer -->
    <div class="tool-drawer" class:open={drawerOpen && activeTool}>
      {#if drawerOpen && activeTool}
        <div class="drawer-inner">
          <div class="drawer-title">
            {tools.find(t => t.id === activeTool)?.label}
          </div>

          <!-- ══ ADJUST ══ -->
          {#if activeTool === 'adjust'}
            <div class="panel">
              {#each [['Brightness', brightness, (v) => brightness = v], ['Contrast', contrast, (v) => contrast = v], ['Saturation', saturation, (v) => saturation = v]] as [lbl, val, setter]}
                <div class="slider-row">
                  <div class="slider-label"><span>{lbl}</span><span class="mono">{val}</span></div>
                  <input type="range" min="-100" max="100" step="1" value={val} oninput={(e) => setter(Number(e.currentTarget.value))} disabled={!hasImage} />
                </div>
              {/each}
              <div class="btn-row">
                <button class="btn-ghost fw" onclick={resetAdjust} disabled={!hasImage}>Reset</button>
                <button class="btn-primary fw" onclick={applyAdjust} disabled={!hasImage || isProcessing}>Apply</button>
              </div>
            </div>

          <!-- ══ FILTERS ══ -->
          {:else if activeTool === 'filters'}
            <div class="panel">
              <!-- Original — restores clean base -->
              <button
                class="filter-card"
                onclick={handleOriginalFilter}
                disabled={!filterBasePath || isProcessing}
                title="Restore image before filters"
              >
                <div class="filter-thumb" style="background:conic-gradient(from 90deg at 40% 40%,#6366f1,#ec4899,#f97316,#6366f1);"></div>
                <span>Original</span>
                {#if filterBasePath}<span class="filter-hint">click to restore</span>{/if}
              </button>

              {#each filters as f}
                <button
                  class="filter-card"
                  onclick={() => handleFilterClick(f.id, f.label)}
                  disabled={!hasImage || isProcessing}
                  title="Apply {f.label}"
                >
                  <div class="filter-thumb" style={f.style}></div>
                  <span>{f.label}</span>
                </button>
              {/each}

              {#if filterBasePath}
                <p class="filter-info">Applying from pre-filter state — switch freely without stacking.</p>
              {/if}
            </div>

          <!-- ══ TRANSFORM ══ -->
          {:else if activeTool === 'transform'}
            <div class="panel">
              <div class="section-title">Rotate & Flip</div>
              <div class="btn-grid-2">
                <button class="btn-ghost" onclick={() => applyTransform('rotate90','Rotate 90°')} disabled={!hasImage||isProcessing}><RotateCw size={13} strokeWidth={2}/>90°</button>
                <button class="btn-ghost" onclick={() => applyTransform('rotate180','Rotate 180°')} disabled={!hasImage||isProcessing}><RefreshCcw size={13} strokeWidth={2}/>180°</button>
                <button class="btn-ghost" onclick={() => applyTransform('flip_h','Flip H')} disabled={!hasImage||isProcessing}><FlipHorizontal size={13} strokeWidth={2}/>Flip H</button>
                <button class="btn-ghost" onclick={() => applyTransform('flip_v','Flip V')} disabled={!hasImage||isProcessing}><FlipVertical size={13} strokeWidth={2}/>Flip V</button>
              </div>

              <div class="section-title" style="margin-top:14px;">Crop</div>
              <div class="input-grid-2">
                <label class="field"><span>X</span><input type="number" min="0" bind:value={cropX} disabled={!hasImage}/></label>
                <label class="field"><span>Y</span><input type="number" min="0" bind:value={cropY} disabled={!hasImage}/></label>
                <label class="field"><span>W</span><input type="number" min="1" bind:value={cropW} disabled={!hasImage}/></label>
                <label class="field"><span>H</span><input type="number" min="1" bind:value={cropH} disabled={!hasImage}/></label>
              </div>
              <button class="btn-ghost fw" onclick={applyCrop} disabled={!hasImage||isProcessing}><Crop size={12} strokeWidth={2}/>Apply Crop</button>

              <div class="section-title" style="margin-top:14px;">Resize</div>
              <div class="input-grid-2">
                <label class="field"><span>W px</span><input type="number" min="1" bind:value={resW} disabled={!hasImage}/></label>
                <label class="field"><span>H px</span><input type="number" min="1" bind:value={resH} disabled={!hasImage}/></label>
              </div>
              <button class="btn-ghost fw" onclick={applyResize} disabled={!hasImage||isProcessing}>Apply Resize</button>
            </div>

          <!-- ══ TEXT ══ -->
          {:else if activeTool === 'text'}
            <div class="panel">
              <div class="field-block">
                <label class="field-lbl">Text Content</label>
                <textarea class="text-area" bind:value={textContent} rows="2" placeholder="Your text here" disabled={!hasImage}></textarea>
              </div>

              <div class="field-block">
                <label class="field-lbl">Font Style</label>
                <div class="font-grid">
                  {#each fontStyles as fs}
                    <button
                      class="font-btn"
                      class:selected={fontStyle === fs.id}
                      onclick={() => (fontStyle = fs.id)}
                      disabled={!hasImage}
                    >
                      {fs.label}
                    </button>
                  {/each}
                </div>
              </div>

              <div class="field-block">
                <label class="field-lbl">Font Size</label>
                <input type="number" min="8" max="400" bind:value={fontSize} disabled={!hasImage} />
              </div>

              <div class="field-block">
                <label class="field-lbl">Text Color</label>
                <div class="color-row">
                  <div class="color-preview" style="background:{textColorHex};"></div>
                  <input type="text" bind:value={textColorHex} placeholder="#FFFFFF" maxlength="7" disabled={!hasImage} style="width:90px;" />
                </div>
                <div class="quick-colors">
                  {#each quickColors as c}
                    <button class="color-swatch" style="background:{c};" onclick={() => (textColorHex = c)} title={c}></button>
                  {/each}
                </div>
              </div>

              <div class="field-block">
                <label class="field-lbl">Position</label>
                <div class="input-grid-2">
                  <label class="field"><span>X px</span><input type="number" bind:value={textX} disabled={!hasImage} /></label>
                  <label class="field"><span>Y px</span><input type="number" bind:value={textY} disabled={!hasImage} /></label>
                </div>
                <div class="btn-grid-3" style="margin-top:6px;">
                  <button class="btn-ghost" onclick={() => setTextAlign('top')}    disabled={!hasImage}>Top</button>
                  <button class="btn-ghost" onclick={() => setTextAlign('center')} disabled={!hasImage}>Center</button>
                  <button class="btn-ghost" onclick={() => setTextAlign('bottom')} disabled={!hasImage}>Bottom</button>
                </div>
              </div>

              <button class="btn-primary fw" onclick={applyText} disabled={!hasImage || isProcessing || !textContent.trim()}>
                Apply Text to Image
              </button>
            </div>

          <!-- ══ IMAGE ══ -->
          {:else if activeTool === 'image'}
            <div class="panel">
              <p class="panel-note">Replace the current canvas or open a new image file.</p>
              <button class="btn-ghost fw" disabled={isProcessing}
                onclick={async () => {
                  const { open: od } = await import('@tauri-apps/plugin-dialog');
                  const sel = await od({ multiple: false, filters: [{ name: 'Images', extensions: ['png','jpg','jpeg','webp','bmp'] }] });
                  if (!sel) return;
                  const dims = await invoke('get_image_dimensions', { path: sel });
                  onPushHistory(sel, dims.width, dims.height);
                  filterBasePath = null; cacheBuster += 1;
                }}
              >
                <FolderOpen size={13} strokeWidth={2} /> Open / Replace Image
              </button>
            </div>

          <!-- ══ MCP AI ══ -->
          {:else if activeTool === 'mcp'}
            <div class="panel">
              <div class="mcp-status-row">
                <span class="status-dot" class:active={mcpIsActive} class:standby={!mcpIsActive}></span>
                <span>
                  {#if mcpIsActive}
                    <strong style="color:var(--text-primary)">Active &amp; Ready</strong>
                  {:else}
                    Standby — Open an image to enable AI
                  {/if}
                </span>
              </div>
              <p class="panel-note">
                Claude can see and edit your canvas when an image is open in the Editor.
              </p>
              <button class="btn-ghost fw" onclick={connectClaude}>
                <Plug size={13} strokeWidth={2} /> Connect to Claude Desktop
              </button>
              {#if connectResult}
                <p class="connect-result">{connectResult}</p>
              {/if}
            </div>
          {/if}

        </div>
      {/if}

      <!-- Drawer toggle arrow -->
      <button
        class="drawer-toggle"
        onclick={() => { drawerOpen = !drawerOpen; }}
        title={drawerOpen ? 'Collapse panel' : 'Expand panel'}
      >
        {#if drawerOpen}<ChevronLeft size={13} strokeWidth={2}/>{:else}<ChevronRight size={13} strokeWidth={2}/>{/if}
      </button>
    </div>

    <!-- ── Canvas Area ───────────────────────────────────────── -->
    <div class="canvas-area" bind:this={canvasArea} use:wheelZoom>
      {#if hasImage}
        <div class="canvas-frame" style="width:{imgWidth * zoom}px; height:{imgHeight * zoom}px;">
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
            <div class="processing-overlay">Processing in Rust…</div>
          {/if}
          {#if comparing && history.length > 0}
            <div class="compare-badge">Original</div>
          {/if}
        </div>
      {:else}
        <div class="empty-canvas">
          <ImageIcon size={42} strokeWidth={1.1} />
          <h3>No Image Loaded</h3>
          <p>Go back to Home and open an image or create a canvas.</p>
          <button class="btn-ghost" onclick={onGoHome}><ArrowLeft size={13} strokeWidth={2}/>Back to Home</button>
        </div>
      {/if}

      <!-- Bottom Bar -->
      <div class="bottom-bar">
        <span class="step-pill">
          {stepLabel}
          {#if errorMsg}<span class="err-dot" title={errorMsg}>⚠</span>{/if}
        </span>
        <div class="zoom-controls">
          <button class="zoom-btn" onclick={() => zoom = Math.max(0.05, zoom - 0.1)}>−</button>
          <input
            type="range" min="5" max="300" step="1"
            value={zoomPct}
            oninput={(e) => zoom = Number(e.currentTarget.value) / 100}
            style="width:90px;"
          />
          <span class="zoom-pct">{zoomPct}%</span>
          <button class="zoom-btn" onclick={() => zoom = Math.min(3.0, zoom + 0.1)}>+</button>
          <button class="zoom-btn" onclick={fitToScreen} title="Fit to screen"><Maximize2 size={11} strokeWidth={2}/></button>
        </div>
      </div>
    </div>
  </div>

  <!-- Export Modal -->
  <ExportModal
    show={showExportModal}
    {currentPath}
    {projectName}
    {imgWidth}
    {imgHeight}
    onClose={() => (showExportModal = false)}
  />
</div>

<style>
  .editor-shell { display:flex; flex-direction:column; height:100vh; width:100vw; overflow:hidden; background:var(--bg-canvas); }

  /* Top Bar */
  .top-bar { height:48px; flex-shrink:0; background:var(--bg-sidebar); border-bottom:1px solid var(--border); display:flex; align-items:center; padding:0 12px; gap:8px; }
  .tb-left,.tb-right { display:flex; align-items:center; gap:6px; flex:1; }
  .tb-right { justify-content:flex-end; }
  .tb-center { display:flex; align-items:center; gap:10px; flex-shrink:0; }
  .tb-divider { width:1px; height:20px; background:var(--border); margin:0 2px; }
  .icon-btn  { display:flex; align-items:center; gap:5px; white-space:nowrap; }
  .icon-btn.sm { font-size:12px; padding:5px 10px; }
  .project-name { background:transparent; border:1px solid transparent; border-radius:var(--r-sm); color:var(--text-primary); font-size:13px; font-weight:600; padding:4px 8px; max-width:200px; text-align:center; min-width:120px; }
  .project-name:hover  { border-color:var(--border); background:var(--bg-input); }
  .project-name:focus  { border-color:var(--accent); background:var(--bg-input); outline:none; }
  .dim-label { font-size:11px; color:var(--text-muted); background:var(--bg-elevated); padding:3px 8px; border-radius:999px; border:1px solid var(--border); }

  /* Editor Body */
  .editor-body { flex:1; display:flex; min-height:0; overflow:hidden; }

  /* Tool Rail */
  .tool-rail { width:68px; flex-shrink:0; background:var(--bg-sidebar); border-right:1px solid var(--border); display:flex; flex-direction:column; align-items:center; padding:10px 0; gap:2px; }
  .rail-btn { display:flex; flex-direction:column; align-items:center; gap:4px; width:56px; padding:9px 4px; border-radius:var(--r-sm); background:transparent; border:1px solid transparent; color:var(--text-secondary); font-size:9px; font-weight:500; }
  .rail-btn:hover:not(:disabled) { background:var(--bg-elevated); color:var(--text-primary); border-color:var(--border); }
  .rail-btn.active { background:var(--accent-subtle); color:var(--text-primary); border-color:var(--border-active); }

  /* Drawer */
  .tool-drawer { position:relative; width:0; flex-shrink:0; overflow:visible; background:var(--bg-card); border-right:1px solid var(--border); transition:width 0.22s cubic-bezier(0.4,0,0.2,1); display:flex; }
  .tool-drawer.open { width:270px; overflow:hidden; }
  .drawer-inner { width:270px; flex-shrink:0; padding:14px 13px; overflow-y:auto; display:flex; flex-direction:column; gap:12px; }
  .drawer-title { font-size:11px; font-weight:700; text-transform:uppercase; letter-spacing:0.08em; color:var(--text-secondary); padding-bottom:10px; border-bottom:1px solid var(--border); }
  .drawer-toggle { position:absolute; right:-14px; top:50%; transform:translateY(-50%); width:14px; height:48px; background:var(--bg-elevated); border:1px solid var(--border); border-left:none; border-radius:0 6px 6px 0; display:grid; place-items:center; color:var(--text-secondary); z-index:10; padding:0; }
  .drawer-toggle:hover:not(:disabled) { background:var(--bg-card-hover); color:var(--text-primary); transform:translateY(-50%) scale(1); }

  /* Panel internals */
  .panel { display:flex; flex-direction:column; gap:10px; }
  .slider-row { display:flex; flex-direction:column; gap:5px; }
  .slider-label { display:flex; justify-content:space-between; font-size:11px; color:var(--text-secondary); }
  .mono { font-variant-numeric:tabular-nums; color:var(--text-primary); }
  .btn-row { display:flex; gap:8px; }
  .fw { width:100%; display:flex; align-items:center; justify-content:center; gap:6px; }
  .section-title { font-size:10px; font-weight:700; text-transform:uppercase; letter-spacing:0.06em; color:var(--text-muted); }
  .btn-grid-2 { display:grid; grid-template-columns:1fr 1fr; gap:6px; }
  .btn-grid-2 button,.btn-grid-3 button { display:flex; align-items:center; justify-content:center; gap:4px; font-size:11px; }
  .btn-grid-3 { display:grid; grid-template-columns:repeat(3,1fr); gap:6px; }
  .input-grid-2 { display:grid; grid-template-columns:1fr 1fr; gap:6px; }
  .field { display:flex; flex-direction:column; gap:3px; font-size:10px; color:var(--text-secondary); }

  /* Filters */
  .filter-card { display:flex; align-items:center; gap:10px; background:var(--bg-elevated); border:1px solid var(--border); border-radius:var(--r-sm); padding:8px 10px; color:var(--text-secondary); font-size:12px; font-weight:500; }
  .filter-card:hover:not(:disabled) { border-color:var(--border-active); color:var(--text-primary); background:var(--bg-card-hover); }
  .filter-thumb { width:36px; height:24px; border-radius:4px; flex-shrink:0; }
  .filter-hint  { font-size:10px; color:var(--accent); margin-left:auto; }
  .filter-info  { font-size:11px; color:var(--text-muted); font-style:italic; }

  /* Text tool */
  .field-block { display:flex; flex-direction:column; gap:5px; }
  .field-lbl   { font-size:11px; font-weight:600; color:var(--text-secondary); text-transform:uppercase; letter-spacing:0.05em; }
  .text-area {
    font-family:var(--font); font-size:13px; resize:vertical; min-height:52px;
    background:var(--bg-input); color:var(--text-primary); border:1px solid var(--border-input);
    border-radius:var(--r-sm); padding:7px 9px; outline:none; width:100%;
  }
  .text-area:focus { border-color:var(--accent); }
  .font-grid { display:grid; grid-template-columns:1fr 1fr; gap:5px; }
  .font-btn { padding:7px 6px; border-radius:var(--r-sm); background:var(--bg-elevated); border:1.5px solid var(--border); color:var(--text-secondary); font-size:11px; }
  .font-btn:hover:not(:disabled)  { border-color:var(--border-active); color:var(--text-primary); }
  .font-btn.selected { background:var(--accent-subtle); border-color:var(--accent); color:var(--text-primary); font-weight:600; }
  .color-row { display:flex; align-items:center; gap:8px; }
  .color-preview { width:28px; height:28px; border-radius:6px; border:1px solid var(--border); flex-shrink:0; }
  .quick-colors { display:flex; gap:6px; flex-wrap:wrap; }
  .color-swatch { width:22px; height:22px; border-radius:50%; border:2px solid var(--border); padding:0; cursor:pointer; }
  .color-swatch:hover:not(:disabled) { transform:scale(1.2) translateY(-1px); border-color:var(--accent); }

  /* MCP */
  .mcp-status-row { display:flex; align-items:center; gap:9px; font-size:12px; color:var(--text-secondary); }
  .status-dot { width:8px; height:8px; border-radius:50%; flex-shrink:0; }
  .status-dot.active  { background:#10b981; box-shadow:0 0 8px rgba(16,185,129,0.6); }
  .status-dot.standby { background:#f59e0b; box-shadow:0 0 6px rgba(245,158,11,0.4); }
  .connect-result { font-size:11px; color:#34d399; line-height:1.5; }
  .panel-note { font-size:12px; color:var(--text-secondary); line-height:1.55; }

  /* Canvas area */
  .canvas-area { flex:1; min-width:0; min-height:0; position:relative; overflow:auto; display:flex; align-items:center; justify-content:center; }
  .canvas-area::before { content:''; position:absolute; inset:0; background-image:linear-gradient(45deg,#1a1d26 25%,transparent 25%),linear-gradient(-45deg,#1a1d26 25%,transparent 25%),linear-gradient(45deg,transparent 75%,#1a1d26 75%),linear-gradient(-45deg,transparent 75%,#1a1d26 75%); background-size:24px 24px; background-position:0 0,0 12px,12px -12px,-12px 0; opacity:0.14; pointer-events:none; }
  .canvas-frame { position:relative; flex-shrink:0; border-radius:4px; overflow:hidden; box-shadow:0 0 0 1px rgba(255,255,255,0.06),0 24px 60px rgba(0,0,0,0.6); }
  .canvas-frame img { display:block; }
  .processing-overlay { position:absolute; inset:0; background:rgba(13,15,18,0.7); backdrop-filter:blur(4px); display:grid; place-items:center; font-size:13px; font-weight:600; color:var(--text-secondary); }
  .compare-badge { position:absolute; top:10px; left:10px; background:rgba(0,0,0,0.65); border:1px solid rgba(255,255,255,0.15); border-radius:999px; padding:3px 10px; font-size:11px; color:#f1f3f8; }
  .empty-canvas { display:flex; flex-direction:column; align-items:center; gap:14px; text-align:center; color:var(--text-secondary); max-width:320px; }
  .empty-canvas h3 { font-size:17px; color:var(--text-primary); }
  .empty-canvas p  { font-size:13px; }
  .empty-canvas button { display:flex; align-items:center; gap:6px; }

  /* Bottom bar */
  .bottom-bar { position:absolute; bottom:14px; left:50%; transform:translateX(-50%); display:flex; align-items:center; gap:14px; background:var(--bg-elevated); border:1px solid var(--border); border-radius:999px; padding:6px 14px; pointer-events:auto; }
  .step-pill { font-size:12px; color:var(--text-secondary); display:flex; align-items:center; gap:4px; }
  .err-dot { color:#f87171; font-size:11px; }
  .zoom-controls { display:flex; align-items:center; gap:5px; }
  .zoom-btn { width:22px; height:22px; display:grid; place-items:center; border-radius:50%; background:var(--bg-card-hover); border:1px solid var(--border); color:var(--text-secondary); font-size:14px; line-height:1; padding:0; }
  .zoom-btn:hover:not(:disabled) { background:var(--accent-subtle); border-color:var(--border-active); color:var(--text-primary); }
  .zoom-pct { font-size:12px; font-variant-numeric:tabular-nums; color:var(--text-primary); min-width:34px; text-align:center; }
</style>
