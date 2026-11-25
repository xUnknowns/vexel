<script>
  import { invoke, convertFileSrc } from '@tauri-apps/api/core';
  import { open, save } from '@tauri-apps/plugin-dialog';
  import {
    ArrowLeft, Download, Undo2, Redo2, Save,
    Layers, Shapes, Type, SlidersHorizontal, Sparkles, Bot,
    ChevronLeft, ChevronRight, Maximize2, Eye, EyeOff,
    Lock, Unlock, Copy, Trash2, ArrowUp, ArrowDown,
    ImagePlus, Eraser, Plug, AlignCenterHorizontal, AlignCenterVertical
  } from 'lucide-svelte';
  import ExportModal from './ExportModal.svelte';

  let {
    project = null,
    cacheBuster = 0,
    mcpIsActive = false,
    onProjectUpdate,
    onGoHome,
  } = $props();

  // ── UI State ─────────────────────────────────────────────────────────────
  let activeTool = $state('layers');
  let drawerOpen = $state(true);
  let selectedLayerId = $state(null);
  let isProcessing = $state(false);
  let statusMsg = $state('Ready');
  let errorMsg = $state('');
  let showExportModal = $state(false);
  let zoom = $state(1.0);
  let canvasArea;

  // ── Derived Project Fields ───────────────────────────────────────────────
  let hasProject = $derived(Boolean(project && project.preview_path));
  let layers = $derived(project?.layers ?? []);
  let canvasW = $derived(project?.canvas_width ?? 1280);
  let canvasH = $derived(project?.canvas_height ?? 720);
  let transparentBg = $derived(project?.transparent_bg ?? false);
  let previewUrl = $derived(
    project?.preview_path ? `${convertFileSrc(project.preview_path)}?v=${cacheBuster}` : ''
  );
  let selectedLayer = $derived(layers.find((l) => l.id === selectedLayerId) ?? null);
  let zoomPct = $derived(Math.round(zoom * 100));

  // Auto-select top layer when layers change and current selection disappears
  $effect(() => {
    if (layers.length > 0 && (!selectedLayerId || !layers.some((l) => l.id === selectedLayerId))) {
      selectedLayerId = layers[layers.length - 1].id;
    } else if (layers.length === 0) {
      selectedLayerId = null;
    }
  });

  // ── Tool Inputs Synced with Selected Layer ───────────────────────────────
  let layerX = $state(0);
  let layerY = $state(0);
  let layerScale = $state(1.0);
  let layerRot = $state(0);
  let layerOpacity = $state(1.0);
  let layerBlend = $state('normal');

  let fxBrightness = $state(0);
  let fxContrast = $state(0);
  let fxSaturation = $state(0);
  let fxBlur = $state(0);
  let fxFilter = $state('original');

  let outEnabled = $state(false);
  let outColor = $state('#FFFFFF');
  let outThickness = $state(6);

  $effect(() => {
    if (selectedLayer) {
      layerX = selectedLayer.x;
      layerY = selectedLayer.y;
      layerScale = selectedLayer.scale;
      layerRot = selectedLayer.rotation;
      layerOpacity = selectedLayer.opacity;
      layerBlend = selectedLayer.blend_mode;
      fxBrightness = selectedLayer.effects.brightness;
      fxContrast = selectedLayer.effects.contrast;
      fxSaturation = selectedLayer.effects.saturation;
      fxBlur = selectedLayer.effects.blur;
      fxFilter = selectedLayer.effects.filter;
      outEnabled = selectedLayer.outline.enabled;
      outThickness = selectedLayer.outline.thickness;
      const [r, g, b] = selectedLayer.outline.color;
      outColor = `#${[r, g, b].map((v) => v.toString(16).padStart(2, '0')).join('')}`;
    }
  });

  // ── Text & Shape & BG Removal Inputs ─────────────────────────────────────
  let textContent = $state('VELTRAX');
  let fontStyle = $state('bold_display');
  let fontSize = $state(96);
  let textColor = $state('#FFFFFF');
  let textDepth = $state(8);
  let strokeColor = $state('#000000');
  let strokeWidth = $state(3);
  let textX = $state(80);
  let textY = $state(80);

  let shapeColor = $state('#6366F1');
  let bgTargetHex = $state('#FFFFFF');
  let bgTolerance = $state(35);
  let connectResult = $state('');

  // ── Drag-to-move layer on canvas ─────────────────────────────────────────
  let isDragging = $state(false);
  let dragStartMouse = { x: 0, y: 0 };
  let dragStartLayer = { x: 0, y: 0 };

  function onCanvasMouseDown(e) {
    if (!selectedLayer || selectedLayer.locked || isProcessing) return;
    isDragging = true;
    dragStartMouse = { x: e.clientX, y: e.clientY };
    dragStartLayer = { x: selectedLayer.x, y: selectedLayer.y };
  }
  function onCanvasMouseMove(e) {
    if (!isDragging) return;
    layerX = Math.round(dragStartLayer.x + (e.clientX - dragStartMouse.x) / zoom);
    layerY = Math.round(dragStartLayer.y + (e.clientY - dragStartMouse.y) / zoom);
  }
  function onCanvasMouseUp() {
    if (!isDragging) return;
    isDragging = false;
    if (selectedLayer && (layerX !== selectedLayer.x || layerY !== selectedLayer.y)) {
      commitTransform();
    }
  }

  // ── Rail & Presets ───────────────────────────────────────────────────────
  const tools = [
    { id: 'layers', label: 'Layers', Icon: Layers },
    { id: 'elements', label: 'Elements', Icon: Shapes },
    { id: 'text', label: 'Text & 3D', Icon: Type },
    { id: 'adjust', label: 'Adjust', Icon: SlidersHorizontal },
    { id: 'outline', label: 'Outline', Icon: Sparkles },
    { id: 'mcp', label: 'MCP AI', Icon: Bot },
  ];

  const fontStyles = [
    { id: 'bold_display', label: 'Bold Display' },
    { id: 'modern_sans', label: 'Modern Sans' },
    { id: 'classic_serif', label: 'Classic Serif' },
    { id: 'tech_mono', label: 'Tech Mono' },
    { id: 'script', label: 'Script' },
  ];

  const filters = [
    { id: 'original', label: 'Original', style: 'background:conic-gradient(from 90deg,#6366f1,#ec4899,#f97316,#6366f1);' },
    { id: 'grayscale', label: 'Grayscale', style: 'background:linear-gradient(135deg,#555,#aaa);' },
    { id: 'noir', label: 'Noir', style: 'background:linear-gradient(135deg,#111,#444);' },
    { id: 'warm', label: 'Warm', style: 'background:linear-gradient(135deg,#b45309,#f97316);' },
    { id: 'cool', label: 'Cool', style: 'background:linear-gradient(135deg,#1d4ed8,#38bdf8);' },
    { id: 'invert', label: 'Invert', style: 'background:linear-gradient(135deg,#e0f2fe,#1e1b4b);' },
  ];

  const quickColors = ['#FFFFFF', '#000000', '#EF4444', '#F97316', '#EAB308', '#10B981', '#6366F1'];

  // ── Helpers ──────────────────────────────────────────────────────────────
  function selectTool(id) {
    if (activeTool === id) drawerOpen = !drawerOpen;
    else {
      activeTool = id;
      drawerOpen = true;
    }
  }

  function fitToScreen() {
    if (!canvasArea || !canvasW || !canvasH) return;
    const { clientWidth: cw, clientHeight: ch } = canvasArea;
    zoom = Math.max(0.05, Math.min((cw - 80) / canvasW, (ch - 80) / canvasH, 3));
  }

  function wheelZoom(node) {
    const h = (e) => {
      e.preventDefault();
      zoom = Math.max(0.05, Math.min(3.0, zoom + (e.deltaY > 0 ? -0.08 : 0.08)));
    };
    node.addEventListener('wheel', h, { passive: false });
    return { destroy: () => node.removeEventListener('wheel', h) };
  }

  async function runCmd(cmd, args, label) {
    if (isProcessing) return;
    isProcessing = true;
    errorMsg = '';
    statusMsg = `${label}…`;
    try {
      const resp = await invoke(cmd, args);
      onProjectUpdate(resp);
      statusMsg = `${label} done`;
      return resp;
    } catch (e) {
      errorMsg = String(e);
      statusMsg = `${label} failed`;
    } finally {
      isProcessing = false;
    }
  }

  // ── Actions ──────────────────────────────────────────────────────────────
  function handleKeydown(e) {
    if ((e.ctrlKey || e.metaKey) && e.key === 'z') {
      e.preventDefault();
      if (project?.can_undo) runCmd('undo_action', {}, 'Undo');
    }
    if ((e.ctrlKey || e.metaKey) && e.key === 'y') {
      e.preventDefault();
      if (project?.can_redo) runCmd('redo_action', {}, 'Redo');
    }
    if (e.key === 'Escape') showExportModal = false;
  }

  async function saveVeltraxProject() {
    if (!hasProject) return;
    const dest = await save({
      title: 'Save Veltrax Project',
      defaultPath: `${project.project_name || 'project'}.veltrax`,
      filters: [{ name: 'Veltrax Project', extensions: ['veltrax'] }],
    });
    if (dest) {
      await invoke('save_project_file', { destPath: dest });
      statusMsg = 'Project saved';
    }
  }

  async function addOverlayImage() {
    const sel = await open({
      multiple: false,
      title: 'Add Image / Icon Overlay',
      filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp', 'bmp'] }],
    });
    if (typeof sel === 'string' && sel) {
      const resp = await runCmd('add_image_overlay_layer', { imagePath: sel, x: 40, y: 40, scale: 1.0 }, 'Add Overlay');
      if (resp?.layers?.length) selectedLayerId = resp.layers[resp.layers.length - 1].id;
    }
  }

  async function addShape(shapeType) {
    const resp = await runCmd(
      'add_shape_layer',
      {
        shapeType,
        width: shapeType === 'badge' ? 320 : 240,
        height: shapeType === 'badge' ? 90 : 240,
        fillHex: shapeColor,
        borderRadius: shapeType === 'rectangle' ? 20 : 0,
        x: Math.round(canvasW / 2 - 120),
        y: Math.round(canvasH / 2 - 120),
      },
      `Add ${shapeType}`
    );
    if (resp?.layers?.length) selectedLayerId = resp.layers[resp.layers.length - 1].id;
  }

  async function addTextLayer() {
    const resp = await runCmd(
      'add_text_layer',
      {
        content: textContent,
        fontStyle,
        size: Number(fontSize),
        colorHex: textColor,
        depth3d: Number(textDepth),
        strokeHex: strokeColor,
        strokeWidth: Number(strokeWidth),
        x: Number(textX),
        y: Number(textY),
      },
      'Add 3D Text'
    );
    if (resp?.layers?.length) selectedLayerId = resp.layers[resp.layers.length - 1].id;
  }

  function commitTransform() {
    if (!selectedLayer) return;
    runCmd(
      'update_layer_transform',
      {
        layerId: selectedLayer.id,
        x: Number(layerX),
        y: Number(layerY),
        scale: Number(layerScale),
        rotation: Number(layerRot),
        opacity: Number(layerOpacity),
        blendMode: layerBlend,
      },
      'Transform Layer'
    );
  }

  function commitEffects(overrideFilter = null) {
    if (!selectedLayer) return;
    const f = overrideFilter ?? fxFilter;
    fxFilter = f;
    runCmd(
      'update_layer_effects',
      {
        layerId: selectedLayer.id,
        brightness: Number(fxBrightness),
        contrast: Number(fxContrast),
        saturation: Number(fxSaturation),
        blur: Number(fxBlur),
        filter: f,
      },
      'Update Effects'
    );
  }

  function commitOutline() {
    if (!selectedLayer) return;
    runCmd(
      'set_layer_outline',
      {
        layerId: selectedLayer.id,
        enabled: outEnabled,
        colorHex: outColor,
        thickness: Number(outThickness),
      },
      'Update Outline'
    );
  }

  function removeBg() {
    if (!selectedLayer || selectedLayer.kind.type !== 'Image') return;
    runCmd(
      'remove_layer_background_color',
      { layerId: selectedLayer.id, targetHex: bgTargetHex, tolerance: Number(bgTolerance) },
      'Remove Background'
    );
  }

  async function connectClaude() {
    try {
      connectResult = await invoke('connect_claude_desktop');
    } catch (e) {
      connectResult = `Error: ${e}`;
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} onmouseup={onCanvasMouseUp} onmousemove={onCanvasMouseMove} />

<div class="editor-shell">
  <!-- ── Top Bar ────────────────────────────────────────────────────────── -->
  <header class="top-bar">
    <div class="tb-left">
      <button class="btn-ghost icon-btn" onclick={onGoHome} title="Back to Home">
        <ArrowLeft size={14} strokeWidth={2} /> Home
      </button>
      <div class="tb-divider"></div>
      <button class="btn-ghost icon-btn sm" onclick={saveVeltraxProject} disabled={!hasProject} title="Save .veltrax project">
        <Save size={13} strokeWidth={2} /> Save
      </button>
      <button class="btn-ghost icon-btn sm" onclick={() => runCmd('undo_action', {}, 'Undo')} disabled={!project?.can_undo || isProcessing} title="Undo (Ctrl+Z)">
        <Undo2 size={13} strokeWidth={2} /> Undo
      </button>
      <button class="btn-ghost icon-btn sm" onclick={() => runCmd('redo_action', {}, 'Redo')} disabled={!project?.can_redo || isProcessing} title="Redo (Ctrl+Y)">
        <Redo2 size={13} strokeWidth={2} /> Redo
      </button>
    </div>

    <div class="tb-center">
      <input
        class="project-name"
        type="text"
        value={project?.project_name ?? 'Untitled Design'}
        onchange={(e) => invoke('rename_project', { name: e.currentTarget.value })}
        spellcheck="false"
      />
      {#if hasProject}
        <span class="dim-label">{canvasW} × {canvasH} px</span>
      {/if}
    </div>

    <div class="tb-right">
      <button class="btn-primary icon-btn" onclick={() => (showExportModal = true)} disabled={!hasProject || isProcessing}>
        <Download size={14} strokeWidth={2} /> Export
      </button>
    </div>
  </header>

  <!-- ── Body ───────────────────────────────────────────────────────────── -->
  <div class="editor-body">
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

    <div class="tool-drawer" class:open={drawerOpen && activeTool}>
      {#if drawerOpen && activeTool}
        <div class="drawer-inner">
          <div class="drawer-title">{tools.find((t) => t.id === activeTool)?.label}</div>

          <!-- ══ LAYERS ══ -->
          {#if activeTool === 'layers'}
            <div class="panel">
              {#if layers.length === 0}
                <p class="panel-note">No layers yet. Add an image overlay, 3D text, or shape from the left rail.</p>
              {:else}
                <div class="layer-list">
                  {#each [...layers].reverse() as l, revIdx}
                    {@const realIdx = layers.length - 1 - revIdx}
                    <div
                      class="layer-item"
                      class:selected={l.id === selectedLayerId}
                      onclick={() => (selectedLayerId = l.id)}
                      role="button"
                      tabindex="0"
                    >
                      <span class="layer-name">{l.name}</span>
                      <div class="layer-actions" onclick={(e) => e.stopPropagation()} role="presentation">
                        <button class="mini-btn" onclick={() => runCmd('reorder_layer', { layerId: l.id, newIndex: realIdx + 1 }, 'Move Up')} disabled={realIdx === layers.length - 1} title="Move Up"><ArrowUp size={12} /></button>
                        <button class="mini-btn" onclick={() => runCmd('reorder_layer', { layerId: l.id, newIndex: Math.max(0, realIdx - 1) }, 'Move Down')} disabled={realIdx === 0} title="Move Down"><ArrowDown size={12} /></button>
                        <button class="mini-btn" onclick={() => runCmd('toggle_layer_visibility', { layerId: l.id }, 'Toggle Visibility')} title="Visibility">
                          {#if l.visible}<Eye size={12} />{:else}<EyeOff size={12} />{/if}
                        </button>
                        <button class="mini-btn" onclick={() => runCmd('toggle_layer_lock', { layerId: l.id }, 'Toggle Lock')} title="Lock">
                          {#if l.locked}<Lock size={12} />{:else}<Unlock size={12} />{/if}
                        </button>
                        <button class="mini-btn" onclick={() => runCmd('duplicate_layer', { layerId: l.id }, 'Duplicate')} title="Duplicate"><Copy size={12} /></button>
                        <button class="mini-btn danger" onclick={() => runCmd('delete_layer', { layerId: l.id }, 'Delete')} title="Delete"><Trash2 size={12} /></button>
                      </div>
                    </div>
                  {/each}
                </div>
              {/if}

              {#if selectedLayer}
                <div class="divider"></div>
                <div class="section-title">Selected Layer Transform</div>
                <div class="slider-row">
                  <div class="slider-label"><span>Opacity</span><span class="mono">{Math.round(layerOpacity * 100)}%</span></div>
                  <input type="range" min="0" max="1" step="0.05" bind:value={layerOpacity} />
                </div>
                <div class="field-block">
                  <label class="field-lbl">Blend Mode</label>
                  <select class="select-input" bind:value={layerBlend}>
                    <option value="normal">Normal</option>
                    <option value="multiply">Multiply</option>
                    <option value="screen">Screen</option>
                    <option value="overlay">Overlay</option>
                  </select>
                </div>
                <div class="input-grid-2">
                  <label class="field"><span>X</span><input type="number" bind:value={layerX} /></label>
                  <label class="field"><span>Y</span><input type="number" bind:value={layerY} /></label>
                  <label class="field"><span>Scale</span><input type="number" min="0.05" max="8" step="0.05" bind:value={layerScale} /></label>
                  <label class="field"><span>Rotate °</span><input type="number" step="1" bind:value={layerRot} /></label>
                </div>
                <button class="btn-primary fw" onclick={commitTransform} disabled={isProcessing || selectedLayer.locked}>
                  Apply Transform
                </button>
              {/if}
            </div>

          <!-- ══ ELEMENTS & IMAGES ══ -->
          {:else if activeTool === 'elements'}
            <div class="panel">
              <button class="btn-primary fw" onclick={addOverlayImage} disabled={!hasProject || isProcessing}>
                <ImagePlus size={14} strokeWidth={2} /> Add Image / Icon Overlay
              </button>

              <div class="divider"></div>
              <div class="section-title">Quick Shapes</div>
              <div class="color-row">
                <div class="color-preview" style="background:{shapeColor};"></div>
                <input type="text" bind:value={shapeColor} maxlength="7" />
              </div>
              <div class="btn-grid-3">
                <button class="btn-ghost" onclick={() => addShape('rectangle')} disabled={!hasProject || isProcessing}>Rect</button>
                <button class="btn-ghost" onclick={() => addShape('circle')} disabled={!hasProject || isProcessing}>Circle</button>
                <button class="btn-ghost" onclick={() => addShape('badge')} disabled={!hasProject || isProcessing}>Badge</button>
              </div>

              <div class="divider"></div>
              <div class="section-title">Remove Background Color</div>
              <p class="panel-note">Removes solid color from the selected Image layer to create clean PNG transparency.</p>
              <div class="color-row">
                <div class="color-preview" style="background:{bgTargetHex};"></div>
                <input type="text" bind:value={bgTargetHex} maxlength="7" />
              </div>
              <div class="slider-row">
                <div class="slider-label"><span>Tolerance</span><span class="mono">{bgTolerance}</span></div>
                <input type="range" min="1" max="150" step="1" bind:value={bgTolerance} />
              </div>
              <button
                class="btn-ghost fw"
                onclick={removeBg}
                disabled={!selectedLayer || selectedLayer.kind.type !== 'Image' || isProcessing}
              >
                <Eraser size={13} strokeWidth={2} /> Remove Background
              </button>
            </div>

          <!-- ══ TEXT & 3D ══ -->
          {:else if activeTool === 'text'}
            <div class="panel">
              <div class="field-block">
                <label class="field-lbl">Text Content</label>
                <textarea class="text-area" bind:value={textContent} rows="2" disabled={!hasProject}></textarea>
              </div>

              <div class="field-block">
                <label class="field-lbl">Font Style</label>
                <div class="font-grid">
                  {#each fontStyles as fs}
                    <button class="font-btn" class:selected={fontStyle === fs.id} onclick={() => (fontStyle = fs.id)}>{fs.label}</button>
                  {/each}
                </div>
              </div>

              <div class="input-grid-2">
                <label class="field"><span>Size (px)</span><input type="number" min="12" max="400" bind:value={fontSize} /></label>
                <label class="field"><span>3D Depth</span><input type="number" min="0" max="30" bind:value={textDepth} /></label>
              </div>

              <div class="input-grid-2">
                <label class="field"><span>Fill Hex</span><input type="text" bind:value={textColor} maxlength="7" /></label>
                <label class="field"><span>Stroke Hex</span><input type="text" bind:value={strokeColor} maxlength="7" /></label>
              </div>

              <div class="quick-colors">
                {#each quickColors as c}
                  <button class="color-swatch" style="background:{c};" onclick={() => (textColor = c)} title={c}></button>
                {/each}
              </div>

              <div class="btn-grid-2">
                <button class="btn-ghost" onclick={() => (textX = Math.max(0, Math.round(canvasW * 0.25)))}>
                  <AlignCenterHorizontal size={12} /> Center H
                </button>
                <button class="btn-ghost" onclick={() => (textY = Math.max(0, Math.round(canvasH * 0.4)))}>
                  <AlignCenterVertical size={12} /> Center V
                </button>
              </div>

              <div class="input-grid-2">
                <label class="field"><span>X px</span><input type="number" bind:value={textX} /></label>
                <label class="field"><span>Y px</span><input type="number" bind:value={textY} /></label>
              </div>

              <button class="btn-primary fw" onclick={addTextLayer} disabled={!hasProject || isProcessing || !textContent.trim()}>
                Add 3D Text Layer
              </button>
            </div>

          <!-- ══ ADJUST & FILTERS ══ -->
          {:else if activeTool === 'adjust'}
            <div class="panel">
              {#if !selectedLayer}
                <p class="panel-note">Select a layer to adjust brightness, contrast, saturation, blur, or filters.</p>
              {:else}
                <div class="slider-row">
                  <div class="slider-label"><span>Brightness</span><span class="mono">{fxBrightness}</span></div>
                  <input type="range" min="-100" max="100" step="1" bind:value={fxBrightness} />
                </div>
                <div class="slider-row">
                  <div class="slider-label"><span>Contrast</span><span class="mono">{fxContrast}</span></div>
                  <input type="range" min="-100" max="100" step="1" bind:value={fxContrast} />
                </div>
                <div class="slider-row">
                  <div class="slider-label"><span>Saturation</span><span class="mono">{fxSaturation}</span></div>
                  <input type="range" min="-100" max="100" step="1" bind:value={fxSaturation} />
                </div>
                <div class="slider-row">
                  <div class="slider-label"><span>Blur</span><span class="mono">{fxBlur}</span></div>
                  <input type="range" min="0" max="20" step="0.5" bind:value={fxBlur} />
                </div>
                <button class="btn-primary fw" onclick={() => commitEffects()} disabled={isProcessing}>
                  Apply Adjustments
                </button>

                <div class="divider"></div>
                <div class="section-title">Non-Destructive Filters</div>
                {#each filters as f}
                  <button
                    class="filter-card"
                    class:selected={fxFilter === f.id}
                    onclick={() => commitEffects(f.id)}
                    disabled={isProcessing}
                  >
                    <div class="filter-thumb" style={f.style}></div>
                    <span>{f.label}</span>
                  </button>
                {/each}
              {/if}
            </div>

          <!-- ══ OUTLINE & GLOW ══ -->
          {:else if activeTool === 'outline'}
            <div class="panel">
              {#if !selectedLayer}
                <p class="panel-note">Select a layer (PNG cutout, text, or shape) to add an outer border.</p>
              {:else}
                <label class="checkbox-row">
                  <input type="checkbox" bind:checked={outEnabled} />
                  <span>Enable Layer Outline</span>
                </label>
                <div class="color-row">
                  <div class="color-preview" style="background:{outColor};"></div>
                  <input type="text" bind:value={outColor} maxlength="7" />
                </div>
                <div class="quick-colors">
                  {#each quickColors as c}
                    <button class="color-swatch" style="background:{c};" onclick={() => (outColor = c)} title={c}></button>
                  {/each}
                </div>
                <div class="slider-row">
                  <div class="slider-label"><span>Thickness</span><span class="mono">{outThickness} px</span></div>
                  <input type="range" min="1" max="24" step="1" bind:value={outThickness} />
                </div>
                <button class="btn-primary fw" onclick={commitOutline} disabled={isProcessing}>
                  Apply Outline
                </button>
              {/if}
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
                    Standby — Open a project to enable AI
                  {/if}
                </span>
              </div>
              <p class="panel-note">
                Claude Desktop can inspect layers, add 3D text, remove backgrounds, and compose thumbnails in real time.
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

      <button
        class="drawer-toggle"
        onclick={() => (drawerOpen = !drawerOpen)}
        title={drawerOpen ? 'Collapse panel' : 'Expand panel'}
      >
        {#if drawerOpen}<ChevronLeft size={13} strokeWidth={2} />{:else}<ChevronRight size={13} strokeWidth={2} />{/if}
      </button>
    </div>

    <!-- ── Interactive Canvas ─────────────────────────────────────────────── -->
    <div class="canvas-area" bind:this={canvasArea} use:wheelZoom>
      {#if hasProject}
        <div
          class="canvas-frame"
          class:transparent-checker={transparentBg}
          style="width:{canvasW * zoom}px; height:{canvasH * zoom}px; cursor:{selectedLayer && !selectedLayer.locked ? (isDragging ? 'grabbing' : 'grab') : 'default'};"
          onmousedown={onCanvasMouseDown}
          role="presentation"
        >
          <img
            src={previewUrl}
            alt="Veltrax Studios Canvas"
            width={canvasW * zoom}
            height={canvasH * zoom}
            style="display:block; width:100%; height:100%; object-fit:fill;"
            onload={fitToScreen}
            draggable="false"
          />
          {#if isProcessing}
            <div class="processing-overlay">Rendering layers in Rust…</div>
          {/if}
        </div>
      {:else}
        <div class="empty-canvas">
          <Layers size={42} strokeWidth={1.1} />
          <h3>No Active Project</h3>
          <p>Return Home to open an image or create a transparent PNG canvas.</p>
          <button class="btn-ghost" onclick={onGoHome}><ArrowLeft size={13} strokeWidth={2} /> Back to Home</button>
        </div>
      {/if}

      <div class="bottom-bar">
        <span class="step-pill">
          {layers.length} layers · {project?.undo_count ?? 0} undo
          {#if errorMsg}<span class="err-dot" title={errorMsg}>!</span>{:else}· {statusMsg}{/if}
        </span>
        <div class="zoom-controls">
          <button class="zoom-btn" onclick={() => (zoom = Math.max(0.05, zoom - 0.1))}>−</button>
          <input
            type="range" min="10" max="300" step="1"
            value={zoomPct}
            oninput={(e) => (zoom = Number(e.currentTarget.value) / 100)}
            style="width:90px;"
          />
          <span class="zoom-pct">{zoomPct}%</span>
          <button class="zoom-btn" onclick={() => (zoom = Math.min(3.0, zoom + 0.1))}>+</button>
          <button class="zoom-btn" onclick={fitToScreen} title="Fit to screen"><Maximize2 size={11} strokeWidth={2} /></button>
        </div>
      </div>
    </div>
  </div>

  <ExportModal
    show={showExportModal}
    {hasProject}
    projectName={project?.project_name ?? 'export'}
    imgWidth={canvasW}
    imgHeight={canvasH}
    {transparentBg}
    onClose={() => (showExportModal = false)}
  />
</div>

<style>
  .editor-shell { display:flex; flex-direction:column; height:100vh; width:100vw; overflow:hidden; background:var(--bg-canvas); }
  .top-bar { height:48px; flex-shrink:0; background:var(--bg-sidebar); border-bottom:1px solid var(--border); display:flex; align-items:center; padding:0 12px; gap:8px; }
  .tb-left,.tb-right { display:flex; align-items:center; gap:6px; flex:1; }
  .tb-right { justify-content:flex-end; }
  .tb-center { display:flex; align-items:center; gap:10px; flex-shrink:0; }
  .tb-divider { width:1px; height:20px; background:var(--border); margin:0 2px; }
  .icon-btn { display:flex; align-items:center; gap:5px; white-space:nowrap; }
  .icon-btn.sm { font-size:12px; padding:5px 10px; }
  .project-name { background:transparent; border:1px solid transparent; border-radius:var(--r-sm); color:var(--text-primary); font-size:13px; font-weight:600; padding:4px 8px; max-width:200px; text-align:center; min-width:120px; }
  .project-name:hover { border-color:var(--border); background:var(--bg-input); }
  .project-name:focus { border-color:var(--accent); background:var(--bg-input); outline:none; }
  .dim-label { font-size:11px; color:var(--text-muted); background:var(--bg-elevated); padding:3px 8px; border-radius:999px; border:1px solid var(--border); }

  .editor-body { flex:1; display:flex; min-height:0; overflow:hidden; }
  .tool-rail { width:68px; flex-shrink:0; background:var(--bg-sidebar); border-right:1px solid var(--border); display:flex; flex-direction:column; align-items:center; padding:10px 0; gap:2px; }
  .rail-btn { display:flex; flex-direction:column; align-items:center; gap:4px; width:58px; padding:9px 4px; border-radius:var(--r-sm); background:transparent; border:1px solid transparent; color:var(--text-secondary); font-size:9px; font-weight:500; }
  .rail-btn:hover:not(:disabled) { background:var(--bg-elevated); color:var(--text-primary); border-color:var(--border); }
  .rail-btn.active { background:var(--accent-subtle); color:var(--text-primary); border-color:var(--border-active); }

  .tool-drawer { position:relative; width:0; flex-shrink:0; overflow:visible; background:var(--bg-card); border-right:1px solid var(--border); transition:width 0.22s cubic-bezier(0.4,0,0.2,1); display:flex; }
  .tool-drawer.open { width:276px; overflow:hidden; }
  .drawer-inner { width:276px; flex-shrink:0; padding:14px 13px; overflow-y:auto; display:flex; flex-direction:column; gap:12px; }
  .drawer-title { font-size:11px; font-weight:700; text-transform:uppercase; letter-spacing:0.08em; color:var(--text-secondary); padding-bottom:10px; border-bottom:1px solid var(--border); }
  .drawer-toggle { position:absolute; right:-14px; top:50%; transform:translateY(-50%); width:14px; height:48px; background:var(--bg-elevated); border:1px solid var(--border); border-left:none; border-radius:0 6px 6px 0; display:grid; place-items:center; color:var(--text-secondary); z-index:10; padding:0; }

  .panel { display:flex; flex-direction:column; gap:10px; }
  .divider { height:1px; background:var(--border); margin:4px 0; }
  .section-title { font-size:10px; font-weight:700; text-transform:uppercase; letter-spacing:0.06em; color:var(--text-muted); }
  .fw { width:100%; display:flex; align-items:center; justify-content:center; gap:6px; }
  .panel-note { font-size:12px; color:var(--text-secondary); line-height:1.5; }

  /* Layers list */
  .layer-list { display:flex; flex-direction:column; gap:6px; max-height:240px; overflow-y:auto; }
  .layer-item { display:flex; align-items:center; justify-content:space-between; gap:6px; padding:8px 9px; border-radius:var(--r-sm); background:var(--bg-elevated); border:1.5px solid var(--border); cursor:pointer; }
  .layer-item.selected { border-color:var(--accent); background:var(--accent-subtle); }
  .layer-name { font-size:12px; font-weight:500; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .layer-actions { display:flex; gap:2px; flex-shrink:0; }
  .mini-btn { width:22px; height:22px; display:grid; place-items:center; border-radius:4px; background:transparent; color:var(--text-secondary); padding:0; }
  .mini-btn:hover:not(:disabled) { background:var(--bg-card-hover); color:var(--text-primary); }
  .mini-btn.danger:hover:not(:disabled) { color:#f87171; }

  .slider-row { display:flex; flex-direction:column; gap:5px; }
  .slider-label { display:flex; justify-content:space-between; font-size:11px; color:var(--text-secondary); }
  .mono { font-variant-numeric:tabular-nums; color:var(--text-primary); }
  .input-grid-2 { display:grid; grid-template-columns:1fr 1fr; gap:6px; }
  .btn-grid-2 { display:grid; grid-template-columns:1fr 1fr; gap:6px; }
  .btn-grid-3 { display:grid; grid-template-columns:repeat(3,1fr); gap:6px; }
  .btn-grid-2 button, .btn-grid-3 button { display:flex; align-items:center; justify-content:center; gap:4px; font-size:11px; }
  .field { display:flex; flex-direction:column; gap:3px; font-size:10px; color:var(--text-secondary); }
  .field-block { display:flex; flex-direction:column; gap:5px; }
  .field-lbl { font-size:11px; font-weight:600; color:var(--text-secondary); text-transform:uppercase; letter-spacing:0.05em; }
  .select-input { background:var(--bg-input); color:var(--text-primary); border:1px solid var(--border-input); border-radius:var(--r-sm); padding:7px 9px; font-size:12px; outline:none; }

  .text-area { font-family:var(--font); font-size:13px; resize:vertical; min-height:52px; background:var(--bg-input); color:var(--text-primary); border:1px solid var(--border-input); border-radius:var(--r-sm); padding:7px 9px; outline:none; width:100%; }
  .font-grid { display:grid; grid-template-columns:1fr 1fr; gap:5px; }
  .font-btn { padding:7px 6px; border-radius:var(--r-sm); background:var(--bg-elevated); border:1.5px solid var(--border); color:var(--text-secondary); font-size:11px; }
  .font-btn.selected { background:var(--accent-subtle); border-color:var(--accent); color:var(--text-primary); font-weight:600; }

  .color-row { display:flex; align-items:center; gap:8px; }
  .color-preview { width:28px; height:28px; border-radius:6px; border:1px solid var(--border); flex-shrink:0; }
  .quick-colors { display:flex; gap:6px; flex-wrap:wrap; }
  .color-swatch { width:20px; height:20px; border-radius:50%; border:2px solid var(--border); padding:0; }

  .filter-card { display:flex; align-items:center; gap:10px; background:var(--bg-elevated); border:1.5px solid var(--border); border-radius:var(--r-sm); padding:8px 10px; color:var(--text-secondary); font-size:12px; font-weight:500; }
  .filter-card.selected { border-color:var(--accent); background:var(--accent-subtle); color:var(--text-primary); }
  .filter-thumb { width:34px; height:22px; border-radius:4px; flex-shrink:0; }

  .checkbox-row { display:flex; align-items:center; gap:8px; font-size:12px; color:var(--text-primary); cursor:pointer; }
  .checkbox-row input { width:auto; accent-color:var(--accent); }

  .mcp-status-row { display:flex; align-items:center; gap:9px; font-size:12px; color:var(--text-secondary); }
  .status-dot { width:8px; height:8px; border-radius:50%; flex-shrink:0; }
  .status-dot.active { background:#10b981; box-shadow:0 0 8px rgba(16,185,129,0.6); }
  .status-dot.standby { background:#f59e0b; box-shadow:0 0 6px rgba(245,158,11,0.4); }
  .connect-result { font-size:11px; color:#34d399; line-height:1.5; white-space:pre-wrap; }

  /* Canvas */
  .canvas-area { flex:1; min-width:0; min-height:0; position:relative; overflow:auto; display:flex; align-items:center; justify-content:center; }
  .canvas-frame { position:relative; flex-shrink:0; border-radius:4px; overflow:hidden; box-shadow:0 0 0 1px rgba(255,255,255,0.08),0 24px 60px rgba(0,0,0,0.6); }
  .canvas-frame.transparent-checker {
    background-image:
      linear-gradient(45deg, #252933 25%, transparent 25%),
      linear-gradient(-45deg, #252933 25%, transparent 25%),
      linear-gradient(45deg, transparent 75%, #252933 75%),
      linear-gradient(-45deg, transparent 75%, #252933 75%);
    background-size: 20px 20px;
    background-position: 0 0, 0 10px, 10px -10px, -10px 0;
    background-color: #161920;
  }
  .processing-overlay { position:absolute; inset:0; background:rgba(13,15,18,0.7); backdrop-filter:blur(4px); display:grid; place-items:center; font-size:13px; font-weight:600; color:var(--text-secondary); }
  .empty-canvas { display:flex; flex-direction:column; align-items:center; gap:14px; text-align:center; color:var(--text-secondary); max-width:320px; }
  .empty-canvas h3 { font-size:17px; color:var(--text-primary); }
  .empty-canvas p { font-size:13px; }
  .empty-canvas button { display:flex; align-items:center; gap:6px; }

  .bottom-bar { position:absolute; bottom:14px; left:50%; transform:translateX(-50%); display:flex; align-items:center; gap:14px; background:var(--bg-elevated); border:1px solid var(--border); border-radius:999px; padding:6px 14px; }
  .step-pill { font-size:12px; color:var(--text-secondary); display:flex; align-items:center; gap:6px; white-space:nowrap; }
  .err-dot { color:#f87171; font-weight:700; }
  .zoom-controls { display:flex; align-items:center; gap:5px; }
  .zoom-btn { width:22px; height:22px; display:grid; place-items:center; border-radius:50%; background:var(--bg-card-hover); border:1px solid var(--border); color:var(--text-secondary); font-size:14px; line-height:1; padding:0; }
  .zoom-pct { font-size:12px; font-variant-numeric:tabular-nums; color:var(--text-primary); min-width:34px; text-align:center; }
</style>
