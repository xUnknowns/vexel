<script>
  import { invoke, convertFileSrc } from '@tauri-apps/api/core';
  import { open, save } from '@tauri-apps/plugin-dialog';
  import {
    ArrowLeft, Download, Undo2, Redo2, Save,
    Layers, Shapes, Type, SlidersHorizontal, Sparkles, Bot, Crop, Palette,
    ChevronLeft, ChevronRight, Maximize2, Eye, EyeOff,
    Lock, Unlock, Copy, Trash2, ArrowUp, ArrowDown,
    ImagePlus, Eraser, AlignCenterHorizontal, AlignCenterVertical,
    Stamp, Search, FlipHorizontal, FlipVertical, Upload, X,
    Grid3x3, Ruler, FolderPlus, Ungroup
  } from 'lucide-svelte';
  import ExportModal from './ExportModal.svelte';
  import CanvasGizmo from './CanvasGizmo.svelte';
  import McpPanel from './McpPanel.svelte';
  import ContextualBar from './ContextualBar.svelte';
  import InspectorPanel from './InspectorPanel.svelte';

  let {
    project = null,
    cacheBuster = 0,
    mcpIsActive = false,
    onProjectUpdate,
    onGoHome,
  } = $props();

  let activeTool = $state('layers');
  let drawerOpen = $state(true);
  let selectedLayerIds = $state([]);
  let selectedLayerId = $derived(selectedLayerIds[0] ?? null);
  let isProcessing = $state(false);
  let statusMsg = $state('Ready');
  let errorMsg = $state('');
  let showExportModal = $state(false);
  let zoom = $state(1.0);
  let canvasArea;
  let textInputEl;

  // ── Phase 2: Grid, Rulers, Guides & Clipboard ────────────────────────────
  let showGrid = $state(false);
  let showRulers = $state(true);
  let guides = $state([]); // { id, axis: 'h'|'v', pos: number }
  let clipboardLayers = $state([]);
  let groups = $derived(project?.groups ?? []);

  // ── Crop State ───────────────────────────────────────────────────────────
  let cropAspect = $state(null);
  let cropRect = $state({ x: 0, y: 0, width: 640, height: 360 });
  let cropMode = $derived(activeTool === 'crop' && drawerOpen);

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
  let isEditingText = $derived(selectedLayer?.kind?.type === 'Text3D');
  let zoomPct = $derived(Math.round(zoom * 100));

  $effect(() => {
    const validIds = selectedLayerIds.filter((id) => layers.some((l) => l.id === id));
    if (layers.length > 0 && validIds.length === 0 && selectedLayerIds.length === 0) {
      // Keep empty if user explicitly deselected, only auto-select on initial load if needed
    } else if (validIds.length !== selectedLayerIds.length) {
      selectedLayerIds = validIds;
    }
  });

  $effect(() => {
    if (cropMode && hasProject) {
      cropRect = {
        x: Math.round(canvasW * 0.1),
        y: Math.round(canvasH * 0.1),
        width: Math.max(40, Math.round(canvasW * 0.8)),
        height: Math.max(40, Math.round(canvasH * 0.8)),
      };
    }
  });

  // ── Selected Layer Transform / Effects / Outline ─────────────────────────
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

  // ── Text Controls (Live-synced when Text3D layer selected) ───────────────
  let textContent = $state('VELTRAX');
  let fontStyle = $state('bold_display');
  let fontSize = $state(96);
  let textColor = $state('#FFFFFF');
  let textDepth = $state(0);
  let strokeColor = $state('#000000');
  let strokeWidth = $state(0);
  let textX = $state(80);
  let textY = $state(80);

  function rgbaToHex([r, g, b]) {
    return `#${[r, g, b].map((v) => v.toString(16).padStart(2, '0')).join('').toUpperCase()}`;
  }

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
      outColor = rgbaToHex(selectedLayer.outline.color);

      if (selectedLayer.kind?.type === 'Text3D') {
        textContent = selectedLayer.kind.content;
        fontStyle = selectedLayer.kind.font_style;
        fontSize = selectedLayer.kind.size;
        textColor = rgbaToHex(selectedLayer.kind.color);
        textDepth = selectedLayer.kind.depth;
        strokeColor = rgbaToHex(selectedLayer.kind.stroke_color);
        strokeWidth = selectedLayer.kind.stroke_width;
      }
    }
  });

  let shapeColor = $state('#6366F1');
  let shapeWithBorder = $state(false);
  let shapeBorderColor = $state('#FFFFFF');
  let shapeBorderWidth = $state(4);
  let bgTargetHex = $state('#FFFFFF');
  let bgTolerance = $state(35);

  // ── Canvas Background ────────────────────────────────────────────────────
  let bgMode = $state('solid'); // 'solid' | 'gradient' | 'transparent'
  let bgSolidHex = $state('#1a1d26');
  let bgGradFromHex = $state('#1a1d26');
  let bgGradToHex = $state('#6366f1');
  let bgGradAngle = $state(135);

  // ── Watermark ────────────────────────────────────────────────────────────
  let wmText = $state('© Veltrax Studios');
  let wmOpacity = $state(0.4);
  let wmPosition = $state('bottom-right');
  let wmSize = $state(28);

  const gradientPresets = [
    { label: 'Dark Drama',  from: '#0f0c29', to: '#302b63' },
    { label: 'Fire',        from: '#f12711', to: '#f5af19' },
    { label: 'Ocean',       from: '#1a1a2e', to: '#16213e' },
    { label: 'Neon',        from: '#00f260', to: '#0575e6' },
    { label: 'Purple Haze', from: '#41295a', to: '#2F0743' },
    { label: 'Sunset',      from: '#f093fb', to: '#f5576c' },
    { label: 'Dark Blue',   from: '#0d0f12', to: '#1e3a5f' },
    { label: 'Midnight',    from: '#000000', to: '#434343' },
  ];

  async function applyCanvasBg() {
    if (bgMode === 'transparent') {
      await runCmd('set_canvas_background', { transparent: true }, 'Set Transparent BG');
    } else if (bgMode === 'gradient') {
      await runCmd('set_canvas_background', {
        fromHex: bgGradFromHex, toHex: bgGradToHex, angle: Number(bgGradAngle)
      }, 'Set Gradient BG');
    } else {
      await runCmd('set_canvas_background', { colorHex: bgSolidHex }, 'Set Solid BG');
    }
  }

  async function addGradientLayer() {
    const resp = await runCmd(
      'add_gradient_layer',
      {
        width: canvasW,
        height: canvasH,
        fromHex: bgGradFromHex,
        toHex: bgGradToHex,
        angle: Number(bgGradAngle),
        x: 0, y: 0,
      },
      'Add Gradient Layer'
    );
    if (resp?.layers?.length) selectedLayerIds = [resp.layers[resp.layers.length - 1].id];
  }

  async function addWatermark() {
    if (!wmText.trim()) return;
    const resp = await runCmd(
      'add_watermark_layer',
      {
        text: wmText,
        opacity: Number(wmOpacity),
        position: wmPosition,
        size: Number(wmSize),
      },
      'Add Watermark'
    );
    if (resp?.layers?.length) selectedLayerIds = [resp.layers[resp.layers.length - 1].id];
  }

  // ── Persistent User Elements Library (%APPDATA%) ─────────────────────────
  let userLibrary = $state([]);
  let librarySearch = $state('');
  let textSearch = $state('');

  let filteredLibrary = $derived(
    librarySearch.trim()
      ? userLibrary.filter((item) => item.name.toLowerCase().includes(librarySearch.trim().toLowerCase()))
      : userLibrary
  );

  let filteredTextPresets = $derived(
    textSearch.trim()
      ? textPresets.filter((tp) => tp.label.toLowerCase().includes(textSearch.trim().toLowerCase()))
      : textPresets
  );

  async function loadUserLibrary() {
    try {
      userLibrary = await invoke('get_user_library');
    } catch (e) {
      console.error('loadUserLibrary:', e);
    }
  }

  $effect(() => {
    loadUserLibrary();
  });

  async function uploadAndSaveElement() {
    const sel = await open({
      multiple: false,
      title: 'Upload & Save Element to Library',
      filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp', 'bmp'] }],
    });
    if (typeof sel === 'string' && sel) {
      try {
        const saved = await invoke('save_to_user_library', { sourcePath: sel });
        await loadUserLibrary();
        if (hasProject) {
          const resp = await runCmd('add_image_overlay_layer', { imagePath: saved.path, x: 40, y: 40, scale: 1.0 }, 'Add Element');
          if (resp?.layers?.length) selectedLayerIds = [resp.layers[resp.layers.length - 1].id];
        }
      } catch (e) {
        errorMsg = String(e);
      }
    }
  }

  async function addLibraryItemToCanvas(item) {
    if (!hasProject || isProcessing) return;
    const resp = await runCmd('add_image_overlay_layer', { imagePath: item.path, x: 40, y: 40, scale: 1.0 }, `Add ${item.name}`);
    if (resp?.layers?.length) selectedLayerIds = [resp.layers[resp.layers.length - 1].id];
  }

  async function deleteLibraryItem(e, id) {
    e.stopPropagation();
    try {
      await invoke('delete_from_user_library', { id });
      await loadUserLibrary();
    } catch (err) {
      errorMsg = String(err);
    }
  }

  async function quickAddTextTier(content, size, font, depth = 0, strokeW = 0) {
    if (!hasProject || isProcessing) return;
    const resp = await runCmd(
      'add_text_layer',
      {
        content,
        fontStyle: font,
        size,
        colorHex: textColor,
        depth3d: depth,
        strokeHex: strokeColor,
        strokeWidth: strokeW,
        x: Math.max(20, Math.round(canvasW * 0.2)),
        y: Math.max(20, Math.round(canvasH * 0.38)),
      },
      'Add Text'
    );
    if (resp?.layers?.length) selectedLayerIds = [resp.layers[resp.layers.length - 1].id];
  }

  // ── Rail & Presets ───────────────────────────────────────────────────────
  // ── Brand Kit & Live Debounced Updates ───────────────────────────────────
  let brandColors = $state(['#38BDF8', '#6366F1', '#EC4899', '#10B981', '#F59E0B']);
  let recentColors = $state(['#FFFFFF', '#000000']);
  let liveTimer = null;
  let lastFitKey = '';

  async function loadBrandKit() {
    try {
      const kit = await invoke('get_brand_kit');
      if (kit?.colors) brandColors = kit.colors;
      if (kit?.recent_colors) recentColors = kit.recent_colors;
    } catch (e) {
      console.error('loadBrandKit:', e);
    }
  }

  $effect(() => {
    loadBrandKit();
  });

  $effect(() => {
    if (hasProject) {
      const key = `${canvasW}x${canvasH}`;
      if (key !== lastFitKey && canvasArea) {
        lastFitKey = key;
        fitToScreen();
      }
    }
  });

  function trackRecentColor(hex) {
    if (!hex) return;
    const norm = hex.toUpperCase();
    recentColors = [norm, ...recentColors.filter((c) => c !== norm)].slice(0, 8);
    invoke('save_brand_kit', { colors: brandColors, recentColors }).catch(console.error);
  }

  function saveBrandColor(hex) {
    const norm = hex.toUpperCase();
    if (!brandColors.includes(norm)) {
      brandColors = [...brandColors, norm];
      invoke('save_brand_kit', { colors: brandColors, recentColors }).catch(console.error);
    }
  }

  function removeBrandColor(hex) {
    brandColors = brandColors.filter((c) => c !== hex);
    invoke('save_brand_kit', { colors: brandColors, recentColors }).catch(console.error);
  }

  function pushCheckpoint() {
    if (hasProject) invoke('push_undo_checkpoint').catch(console.error);
  }

  function scheduleLive(fn, immediate = false) {
    clearTimeout(liveTimer);
    if (immediate) {
      fn();
    } else {
      liveTimer = setTimeout(fn, 60);
    }
  }

  function liveUpdateTransform(patch, recordUndo = false) {
    if (!selectedLayer || selectedLayer.locked) return;
    const l = selectedLayer;
    scheduleLive(async () => {
      try {
        const resp = await invoke('update_layer_transform', {
          layerId: l.id,
          x: patch.x ?? l.x,
          y: patch.y ?? l.y,
          scale: patch.scale ?? l.scale,
          rotation: patch.rotation ?? l.rotation,
          opacity: patch.opacity ?? l.opacity,
          blendMode: patch.blend_mode ?? l.blend_mode,
          recordUndo,
        });
        onProjectUpdate(resp);
      } catch (e) {
        errorMsg = String(e);
      }
    }, recordUndo);
  }

  function liveUpdateText(patch, recordUndo = false) {
    if (!selectedLayer || selectedLayer.kind?.type !== 'Text3D') return;
    const l = selectedLayer;
    const k = l.kind;
    if (patch.color_hex) trackRecentColor(patch.color_hex);
    if (patch.stroke_hex) trackRecentColor(patch.stroke_hex);
    scheduleLive(async () => {
      try {
        const resp = await invoke('update_text_layer', {
          layerId: l.id,
          content: patch.content ?? k.content,
          fontStyle: patch.font_style ?? k.font_style,
          size: Number(patch.size ?? k.size),
          colorHex: patch.color_hex ?? rgbaToHex(k.color),
          depth3d: Number(patch.depth_3d ?? k.depth),
          strokeHex: patch.stroke_hex ?? rgbaToHex(k.stroke_color),
          strokeWidth: Number(patch.stroke_width ?? k.stroke_width),
          bold: patch.bold ?? k.bold ?? false,
          italic: patch.italic ?? k.italic ?? false,
          align: patch.align ?? k.align ?? 'left',
          lineHeight: Number(patch.line_height ?? k.line_height ?? 1.2),
          letterSpacing: Number(patch.letter_spacing ?? k.letter_spacing ?? 0),
          recordUndo,
        });
        onProjectUpdate(resp);
      } catch (e) {
        errorMsg = String(e);
      }
    }, recordUndo);
  }

  function liveUpdateShape(patch, recordUndo = false) {
    if (!selectedLayer || selectedLayer.kind?.type !== 'Shape') return;
    const l = selectedLayer;
    const k = l.kind;
    if (patch.fill_hex) trackRecentColor(patch.fill_hex);
    if (patch.border_hex) trackRecentColor(patch.border_hex);
    scheduleLive(async () => {
      try {
        const resp = await invoke('update_shape_layer', {
          layerId: l.id,
          width: Number(patch.width ?? k.width),
          height: Number(patch.height ?? k.height),
          fillHex: patch.fill_hex ?? rgbaToHex(k.fill_color),
          borderRadius: Number(patch.border_radius ?? k.border_radius),
          borderHex: patch.border_hex ?? rgbaToHex(k.border_color ?? [255, 255, 255, 255]),
          borderWidth: Number(patch.border_width ?? k.border_width ?? 0),
          recordUndo,
        });
        onProjectUpdate(resp);
      } catch (e) {
        errorMsg = String(e);
      }
    }, recordUndo);
  }

  function liveUpdateEffects(patch, recordUndo = false) {
    if (!selectedLayer) return;
    const l = selectedLayer;
    const fx = l.effects;
    scheduleLive(async () => {
      try {
        const resp = await invoke('update_layer_effects', {
          layerId: l.id,
          brightness: Number(patch.brightness ?? fx.brightness),
          contrast: Number(patch.contrast ?? fx.contrast),
          saturation: Number(patch.saturation ?? fx.saturation),
          blur: Number(patch.blur ?? fx.blur),
          filter: patch.filter ?? fx.filter,
          recordUndo,
        });
        onProjectUpdate(resp);
      } catch (e) {
        errorMsg = String(e);
      }
    }, recordUndo);
  }

  function liveUpdateOutline(patch, recordUndo = false) {
    if (!selectedLayer) return;
    const l = selectedLayer;
    const o = l.outline;
    if (patch.color_hex) trackRecentColor(patch.color_hex);
    scheduleLive(async () => {
      try {
        const resp = await invoke('set_layer_outline', {
          layerId: l.id,
          enabled: patch.enabled ?? o.enabled,
          colorHex: patch.color_hex ?? rgbaToHex(o.color),
          thickness: Number(patch.thickness ?? o.thickness),
          recordUndo,
        });
        onProjectUpdate(resp);
      } catch (e) {
        errorMsg = String(e);
      }
    }, recordUndo);
  }

  async function replaceImageLayer(layerId) {
    const sel = await open({
      multiple: false,
      title: 'Replace Image Layer',
      filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp', 'bmp'] }],
    });
    if (typeof sel === 'string' && sel) {
      await runCmd('replace_image_layer', { layerId, imagePath: sel }, 'Replace Image');
    }
  }

  function quickRemoveBg(layerId) {
    selectedLayerIds = [layerId];
    activeTool = 'elements';
    drawerOpen = true;
  }

  const tools = [
    { id: 'layers',     label: 'Layers',    Icon: Layers },
    { id: 'elements',   label: 'Elements',  Icon: Shapes },
    { id: 'text',       label: 'Text',      Icon: Type },
    { id: 'background', label: 'BG',        Icon: Palette },
    { id: 'crop',       label: 'Crop',      Icon: Crop },
    { id: 'mcp',        label: 'MCP AI',    Icon: Bot },
  ];

  const textPresets = [
    { label: 'Flat Text',      depth: 0,  strokeW: 0,  color: '#FFFFFF', font: 'bold_display' },
    { label: 'Outlined',       depth: 0,  strokeW: 4,  color: '#FFFFFF', font: 'bold_display' },
    { label: 'Soft Shadow',    depth: 3,  strokeW: 0,  color: '#FFFFFF', font: 'bold_display' },
    { label: '3D Gaming',      depth: 10, strokeW: 3,  color: '#FFD700', font: 'bold_display' },
    { label: 'Neon Glow',      depth: 0,  strokeW: 2,  color: '#00f5ff', font: 'bold_display' },
    { label: 'Retro Stroke',   depth: 2,  strokeW: 5,  color: '#FF6B35', font: 'bold_display' },
    { label: 'Tech Mono',      depth: 0,  strokeW: 1,  color: '#7CFC00', font: 'tech_mono' },
    { label: 'Classic Serif',  depth: 0,  strokeW: 0,  color: '#F5F5DC', font: 'classic_serif' },
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

  function selectTool(id) {
    if (activeTool === id) drawerOpen = !drawerOpen;
    else {
      activeTool = id;
      drawerOpen = true;
    }
  }

  function setCropAspectRatio(ratio) {
    cropAspect = ratio;
    if (ratio && hasProject) {
      let nw = Math.round(canvasW * 0.8);
      let nh = Math.round(nw / ratio);
      if (nh > canvasH) {
        nh = Math.round(canvasH * 0.8);
        nw = Math.round(nh * ratio);
      }
      cropRect = {
        x: Math.round((canvasW - nw) / 2),
        y: Math.round((canvasH - nh) / 2),
        width: nw,
        height: nh,
      };
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

  function handleKeydown(e) {
    const tag = e.target?.tagName;
    const inInput = tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT';
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'z') {
      if (inInput) return;
      e.preventDefault();
      if (project?.can_undo) runCmd('undo_action', {}, 'Undo');
    }
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'y') {
      if (inInput) return;
      e.preventDefault();
      if (project?.can_redo) runCmd('redo_action', {}, 'Redo');
    }
    if (!inInput && (e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'c') {
      const sel = layers.filter((l) => selectedLayerIds.includes(l.id));
      if (sel.length > 0) {
        clipboardLayers = sel;
        statusMsg = `Copied ${sel.length} layer(s)`;
      }
    }
    if (!inInput && (e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'x') {
      const sel = layers.filter((l) => selectedLayerIds.includes(l.id));
      if (sel.length > 0) {
        clipboardLayers = sel;
        runCmd('delete_layers', { layerIds: selectedLayerIds }, 'Cut Layers');
      }
    }
    if (!inInput && (e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'v') {
      if (clipboardLayers.length > 0 && hasProject) {
        e.preventDefault();
        runCmd('paste_layers_from_json', { layersJson: JSON.stringify(clipboardLayers) }, 'Paste Layers');
      }
    }
    if (e.key === 'Escape') showExportModal = false;
  }

  function handlePaste(e) {
    if (!hasProject) return;
    const tag = e.target?.tagName;
    if (tag === 'INPUT' || tag === 'TEXTAREA') return;
    const items = e.clipboardData?.items;
    if (!items) return;
    for (const item of items) {
      if (item.type.startsWith('image/')) {
        const file = item.getAsFile();
        if (file) {
          e.preventDefault();
          const reader = new FileReader();
          reader.onload = () => {
            if (typeof reader.result === 'string') {
              runCmd('add_image_bytes_layer', {
                filename: file.name || 'clipboard.png',
                dataBase64: reader.result,
                x: 60,
                y: 60,
              }, 'Paste Image');
            }
          };
          reader.readAsDataURL(file);
          return;
        }
      }
    }
  }

  function handleDrop(e) {
    e.preventDefault();
    if (!hasProject) return;
    const files = e.dataTransfer?.files;
    if (!files || files.length === 0) return;
    for (const file of files) {
      if (file.type.startsWith('image/') || /\.(png|jpe?g|webp|bmp)$/i.test(file.name)) {
        const reader = new FileReader();
        reader.onload = () => {
          if (typeof reader.result === 'string') {
            runCmd('add_image_bytes_layer', {
              filename: file.name,
              dataBase64: reader.result,
              x: 60,
              y: 60,
            }, `Drop ${file.name}`);
          }
        };
        reader.readAsDataURL(file);
      }
    }
  }

  function addGuide(axis, pos) {
    guides = [...guides, { id: `g-${Date.now()}-${Math.random()}`, axis, pos: Math.max(0, Math.round(pos)) }];
  }

  function toggleLayerSelect(e, id) {
    if (e.shiftKey) {
      selectedLayerIds = selectedLayerIds.includes(id)
        ? selectedLayerIds.filter((x) => x !== id)
        : [...selectedLayerIds, id];
    } else {
      selectedLayerIds = [id];
    }
  }

  function selectGroupLayers(groupId) {
    const ids = layers.filter((l) => l.group_id === groupId).map((l) => l.id);
    if (ids.length > 0) selectedLayerIds = ids;
  }

  function groupSelected() {
    if (selectedLayerIds.length < 1) return;
    runCmd('group_layers', { layerIds: selectedLayerIds, name: null, parentGroupId: null }, 'Group Layers');
  }

  function ungroupSelected() {
    const first = layers.find((l) => selectedLayerIds.includes(l.id) && l.group_id);
    if (first?.group_id) {
      runCmd('ungroup_layers', { groupId: first.group_id }, 'Ungroup');
    }
  }

  async function saveVeltraxProject() {
    if (!hasProject) return;
    let defaultDir = '';
    try {
      defaultDir = await invoke('get_default_projects_dir');
    } catch {
      defaultDir = '';
    }
    const fileName = `${project.project_name || 'project'}.veltrax`;
    const dest = await save({
      title: 'Save Veltrax Project',
      defaultPath: defaultDir ? `${defaultDir}\\${fileName}` : fileName,
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
      if (resp?.layers?.length) selectedLayerIds = [resp.layers[resp.layers.length - 1].id];
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
        borderHex: shapeWithBorder ? shapeBorderColor : null,
        borderWidth: shapeWithBorder ? shapeBorderWidth : 0,
        x: Math.round(canvasW / 2 - 120),
        y: Math.round(canvasH / 2 - 120),
      },
      `Add ${shapeType}`
    );
    if (resp?.layers?.length) selectedLayerIds = [resp.layers[resp.layers.length - 1].id];
  }

  async function addNewTextLayer() {
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
      'Add Text'
    );
    if (resp?.layers?.length) selectedLayerIds = [resp.layers[resp.layers.length - 1].id];
  }

  async function updateSelectedTextLayer() {
    if (!selectedLayer || !isEditingText) return;
    await runCmd(
      'update_text_layer',
      {
        layerId: selectedLayer.id,
        content: textContent,
        fontStyle,
        size: Number(fontSize),
        colorHex: textColor,
        depth3d: Number(textDepth),
        strokeHex: strokeColor,
        strokeWidth: Number(strokeWidth),
      },
      'Update Text'
    );
  }

  function handleDoubleClickText(layer) {
    selectedLayerIds = [layer.id];
    activeTool = 'text';
    drawerOpen = true;
    setTimeout(() => textInputEl?.focus(), 60);
  }

  function commitGizmoTransform(layerId, nx, ny, nscale, nrot) {
    const l = layers.find((item) => item.id === layerId);
    if (!l) return;
    runCmd(
      'update_layer_transform',
      {
        layerId,
        x: nx,
        y: ny,
        scale: nscale,
        rotation: nrot,
        opacity: l.opacity,
        blendMode: l.blend_mode,
      },
      'Transform Layer'
    );
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

  async function applyCrop() {
    await runCmd(
      'crop_canvas',
      {
        x: Number(cropRect.x),
        y: Number(cropRect.y),
        width: Math.max(1, Number(cropRect.width)),
        height: Math.max(1, Number(cropRect.height)),
      },
      'Crop Canvas'
    );
    activeTool = 'layers';
  }
</script>

<svelte:window onkeydown={handleKeydown} onpaste={handlePaste} />

<div class="editor-shell">
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
              {#if selectedLayerIds.length > 0}
                <div class="btn-grid-2">
                  <button class="btn-ghost" onclick={groupSelected} title="Group selected (Ctrl+G)">
                    <FolderPlus size={12} /> Group ({selectedLayerIds.length})
                  </button>
                  <button class="btn-ghost" onclick={ungroupSelected} title="Ungroup (Ctrl+Shift+G)">
                    <Ungroup size={12} /> Ungroup
                  </button>
                </div>
              {/if}

              {#if groups.length > 0}
                <div class="group-list">
                  {#each groups as grp}
                    <div class="group-row" onclick={() => selectGroupLayers(grp.id)} role="button" tabindex="0">
                      <span class="group-name">{grp.name}</span>
                      <div class="layer-actions" onclick={(e) => e.stopPropagation()} role="presentation">
                        <button class="mini-btn" onclick={() => runCmd('toggle_group_visibility', { groupId: grp.id }, 'Group Visibility')} title="Toggle Group Visibility">
                          {#if grp.visible}<Eye size={12} />{:else}<EyeOff size={12} />{/if}
                        </button>
                        <button class="mini-btn" onclick={() => runCmd('toggle_group_lock', { groupId: grp.id }, 'Group Lock')} title="Toggle Group Lock">
                          {#if grp.locked}<Lock size={12} />{:else}<Unlock size={12} />{/if}
                        </button>
                        <button class="mini-btn danger" onclick={() => runCmd('ungroup_layers', { groupId: grp.id }, 'Ungroup')} title="Ungroup">
                          <Ungroup size={12} />
                        </button>
                      </div>
                    </div>
                  {/each}
                </div>
              {/if}

              {#if layers.length === 0}
                <p class="panel-note">No layers yet. Add an image overlay, text, or shape from the left rail.</p>
              {:else}
                <div class="layer-list">
                  {#each [...layers].reverse() as l, revIdx}
                    {@const realIdx = layers.length - 1 - revIdx}
                    <div
                      class="layer-item"
                      class:selected={selectedLayerIds.includes(l.id)}
                      onclick={(e) => toggleLayerSelect(e, l.id)}
                      role="button"
                      tabindex="0"
                    >
                      <span class="layer-name">
                        {#if l.group_id}<span class="grp-tag">G</span>{/if}{l.name}
                      </span>
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
                <div class="btn-grid-2">
                  <button class="btn-ghost" onclick={() => runCmd('flip_layer', { layerId: selectedLayer.id, horizontal: true }, 'Flip H')} disabled={isProcessing || selectedLayer.locked}>
                    <FlipHorizontal size={12} strokeWidth={1.75} /> Flip H
                  </button>
                  <button class="btn-ghost" onclick={() => runCmd('flip_layer', { layerId: selectedLayer.id, horizontal: false }, 'Flip V')} disabled={isProcessing || selectedLayer.locked}>
                    <FlipVertical size={12} strokeWidth={1.75} /> Flip V
                  </button>
                </div>
                <button class="btn-primary fw" onclick={commitTransform} disabled={isProcessing || selectedLayer.locked}>
                  Apply Transform
                </button>
              {/if}
            </div>

          <!-- ══ ELEMENTS & SAVED LIBRARY ══ -->
          {:else if activeTool === 'elements'}
            <div class="panel">
              <div class="search-box">
                <Search size={13} strokeWidth={1.75} />
                <input type="text" bind:value={librarySearch} placeholder="Search saved elements..." />
              </div>

              <button class="btn-primary fw" onclick={uploadAndSaveElement} disabled={isProcessing}>
                <Upload size={14} strokeWidth={2} /> Upload &amp; Save Element
              </button>
              <button class="btn-ghost fw" onclick={addOverlayImage} disabled={!hasProject || isProcessing}>
                <ImagePlus size={13} strokeWidth={1.75} /> One-Time Image Overlay
              </button>

              {#if filteredLibrary.length > 0}
                <div class="section-title">Saved Library ({filteredLibrary.length})</div>
                <div class="lib-grid">
                  {#each filteredLibrary as item}
                    <div
                      class="lib-card"
                      onclick={() => addLibraryItemToCanvas(item)}
                      title={`Click to add ${item.name} (${item.width}×${item.height})`}
                      role="button"
                      tabindex="0"
                    >
                      <img src={`${convertFileSrc(item.path)}?v=1`} alt={item.name} loading="lazy" />
                      <button
                        class="lib-del"
                        onclick={(e) => deleteLibraryItem(e, item.id)}
                        title="Remove from library"
                      >
                        <X size={10} strokeWidth={2.5} />
                      </button>
                    </div>
                  {/each}
                </div>
              {:else}
                <p class="panel-note">Uploaded PNGs &amp; icons are saved here permanently across updates.</p>
              {/if}

              <div class="divider"></div>
              <div class="section-title">Quick Shapes</div>
              <div class="color-row">
                <div class="color-preview" style="background:{shapeColor};"></div>
                <input type="text" bind:value={shapeColor} maxlength="7" />
              </div>
              <label class="checkbox-row">
                <input type="checkbox" bind:checked={shapeWithBorder} />
                <span>Add Shape Border</span>
              </label>
              {#if shapeWithBorder}
                <div class="input-grid-2">
                  <label class="field"><span>Border Hex</span><input type="text" bind:value={shapeBorderColor} maxlength="7" /></label>
                  <label class="field"><span>Border W</span><input type="number" min="1" max="32" bind:value={shapeBorderWidth} /></label>
                </div>
              {/if}
              <div class="btn-grid-3">
                <button class="btn-ghost" onclick={() => addShape('rectangle')} disabled={!hasProject || isProcessing}>Rect</button>
                <button class="btn-ghost" onclick={() => addShape('circle')} disabled={!hasProject || isProcessing}>Circle</button>
                <button class="btn-ghost" onclick={() => addShape('badge')} disabled={!hasProject || isProcessing}>Badge</button>
              </div>

              <div class="divider"></div>
              <div class="section-title">Watermark Stamp</div>
              <div class="field-block">
                <input type="text" bind:value={wmText} placeholder="© Your Brand" />
              </div>
              <div class="input-grid-2">
                <label class="field">
                  <span>Position</span>
                  <select class="select-input" bind:value={wmPosition}>
                    <option value="bottom-right">Bottom Right</option>
                    <option value="bottom-left">Bottom Left</option>
                    <option value="top-right">Top Right</option>
                    <option value="top-left">Top Left</option>
                    <option value="center">Center</option>
                  </select>
                </label>
                <label class="field"><span>Size (px)</span><input type="number" min="10" max="120" bind:value={wmSize} /></label>
              </div>
              <div class="slider-row">
                <div class="slider-label"><span>Opacity</span><span class="mono">{Math.round(wmOpacity * 100)}%</span></div>
                <input type="range" min="0.1" max="1" step="0.05" bind:value={wmOpacity} />
              </div>
              <button class="btn-ghost fw" onclick={addWatermark} disabled={!hasProject || isProcessing || !wmText.trim()}>
                <Stamp size={13} strokeWidth={2} /> Add Watermark
              </button>

              <div class="divider"></div>
              <div class="section-title">Remove Background Color</div>
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

          <!-- ══ TEXT (All Styles + Live Edit) ══ -->
          {:else if activeTool === 'text'}
            <div class="panel">
              <div class="search-box">
                <Search size={13} strokeWidth={1.75} />
                <input type="text" bind:value={textSearch} placeholder="Search text styles..." />
              </div>

              <button class="btn-primary fw" onclick={addNewTextLayer} disabled={!hasProject || isProcessing || !textContent.trim()}>
                <Type size={14} strokeWidth={2} /> Add a text box
              </button>

              <div class="section-title">Default Text Styles</div>
              <div class="tier-stack">
                <button class="tier-btn tier-h1" onclick={() => quickAddTextTier('Add a heading', 112, 'bold_display', 0, 0)} disabled={!hasProject || isProcessing}>
                  Add a heading
                </button>
                <button class="tier-btn tier-h2" onclick={() => quickAddTextTier('Add a subheading', 64, 'modern_sans', 0, 0)} disabled={!hasProject || isProcessing}>
                  Add a subheading
                </button>
                <button class="tier-btn tier-body" onclick={() => quickAddTextTier('Add a little bit of body text', 34, 'modern_sans', 0, 0)} disabled={!hasProject || isProcessing}>
                  Add a little bit of body text
                </button>
              </div>

              <div class="field-block">
                <label class="field-lbl">Text Effects &amp; 3D Presets</label>
                <div class="btn-grid-2">
                  {#each filteredTextPresets as tp}
                    <button
                      class="btn-ghost"
                      onclick={() => {
                        textDepth = tp.depth;
                        strokeWidth = tp.strokeW;
                        if (tp.color) textColor = tp.color;
                        if (tp.font) fontStyle = tp.font;
                      }}
                    >
                      {tp.label}
                    </button>
                  {/each}
                </div>
              </div>

              <div class="field-block">
                <label class="field-lbl">Text Content</label>
                <textarea class="text-area" bind:this={textInputEl} bind:value={textContent} rows="2" disabled={!hasProject}></textarea>
              </div>

              <div class="field-block">
                <label class="field-lbl">Font Family</label>
                <div class="font-grid">
                  {#each fontStyles as fs}
                    <button class="font-btn" class:selected={fontStyle === fs.id} onclick={() => (fontStyle = fs.id)}>{fs.label}</button>
                  {/each}
                </div>
              </div>

              <div class="input-grid-2">
                <label class="field"><span>Size (px)</span><input type="number" min="12" max="400" bind:value={fontSize} /></label>
                <label class="field"><span>3D Depth</span><input type="number" min="0" max="30" bind:value={textDepth} /></label>
                <label class="field"><span>Stroke W</span><input type="number" min="0" max="10" bind:value={strokeWidth} /></label>
                <label class="field"><span>Fill Hex</span><input type="text" bind:value={textColor} maxlength="7" /></label>
              </div>

              <div class="field-block">
                <label class="field-lbl">Stroke Color</label>
                <input type="text" bind:value={strokeColor} maxlength="7" />
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
              <p class="panel-note">Select any text on the canvas to edit it live in the right Inspector or top bar.</p>
            </div>

          <!-- ══ BACKGROUND ══ -->
          {:else if activeTool === 'background'}
            <div class="panel">
              <div class="btn-grid-3">
                <button class="btn-ghost" class:selected-ratio={bgMode === 'solid'} onclick={() => (bgMode = 'solid')}>Solid</button>
                <button class="btn-ghost" class:selected-ratio={bgMode === 'gradient'} onclick={() => (bgMode = 'gradient')}>Gradient</button>
                <button class="btn-ghost" class:selected-ratio={bgMode === 'transparent'} onclick={() => (bgMode = 'transparent')}>PNG Alpha</button>
              </div>

              {#if bgMode === 'solid'}
                <div class="color-row">
                  <div class="color-preview" style="background:{bgSolidHex};"></div>
                  <input type="text" bind:value={bgSolidHex} maxlength="7" />
                </div>
                <div class="quick-colors">
                  {#each quickColors as c}
                    <button class="color-swatch" style="background:{c};" onclick={() => (bgSolidHex = c)} title={c}></button>
                  {/each}
                </div>
              {:else if bgMode === 'gradient'}
                <div class="input-grid-2">
                  <label class="field"><span>From</span><input type="text" bind:value={bgGradFromHex} maxlength="7" /></label>
                  <label class="field"><span>To</span><input type="text" bind:value={bgGradToHex} maxlength="7" /></label>
                </div>
                <div class="slider-row">
                  <div class="slider-label"><span>Angle</span><span class="mono">{bgGradAngle}°</span></div>
                  <input type="range" min="0" max="360" step="15" bind:value={bgGradAngle} />
                </div>
                <div class="section-title">Gradient Presets</div>
                <div class="btn-grid-2">
                  {#each gradientPresets as gp}
                    <button
                      class="grad-preset-btn"
                      onclick={() => {
                        bgGradFromHex = gp.from;
                        bgGradToHex = gp.to;
                      }}
                    >
                      <span class="grad-dot" style="background:linear-gradient(135deg,{gp.from},{gp.to});"></span>
                      <span>{gp.label}</span>
                    </button>
                  {/each}
                </div>
              {:else}
                <p class="panel-note">Transparent checkerboard canvas for PNG logos, stickers, and overlays.</p>
              {/if}

              <button class="btn-primary fw" onclick={applyCanvasBg} disabled={!hasProject || isProcessing}>
                Apply Canvas Background
              </button>

              {#if bgMode === 'gradient'}
                <button class="btn-ghost fw" onclick={addGradientLayer} disabled={!hasProject || isProcessing}>
                  + Add as Gradient Layer
                </button>
              {/if}
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

          <!-- ══ OUTLINE ══ -->
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

          <!-- ══ CROP ══ -->
          {:else if activeTool === 'crop'}
            <div class="panel">
              <p class="panel-note">Drag or resize the blue crop frame on the canvas, or pick a quick aspect ratio:</p>
              <div class="btn-grid-2">
                <button class="btn-ghost" class:selected-ratio={cropAspect === null} onclick={() => setCropAspectRatio(null)}>Free</button>
                <button class="btn-ghost" class:selected-ratio={cropAspect === 16/9} onclick={() => setCropAspectRatio(16 / 9)}>16:9 YouTube</button>
                <button class="btn-ghost" class:selected-ratio={cropAspect === 1} onclick={() => setCropAspectRatio(1)}>1:1 Logo</button>
                <button class="btn-ghost" class:selected-ratio={cropAspect === 9/16} onclick={() => setCropAspectRatio(9 / 16)}>9:16 Reels</button>
              </div>
              <div class="input-grid-2">
                <label class="field"><span>X</span><input type="number" bind:value={cropRect.x} /></label>
                <label class="field"><span>Y</span><input type="number" bind:value={cropRect.y} /></label>
                <label class="field"><span>W</span><input type="number" min="1" bind:value={cropRect.width} /></label>
                <label class="field"><span>H</span><input type="number" min="1" bind:value={cropRect.height} /></label>
              </div>
              <button class="btn-primary fw" onclick={applyCrop} disabled={!hasProject || isProcessing}>
                <Crop size={14} strokeWidth={2} /> Apply Crop
              </button>

              <div class="divider"></div>
              <div class="section-title">Magic Resize (Scale Layers)</div>
              <div class="btn-grid-2">
                <button class="btn-ghost" onclick={() => runCmd('resize_canvas', { width: 1280, height: 720, scaleLayers: true }, 'Magic Resize 1280×720')} disabled={!hasProject || isProcessing}>YT 1280×720</button>
                <button class="btn-ghost" onclick={() => runCmd('resize_canvas', { width: 1080, height: 1080, scaleLayers: true }, 'Magic Resize 1080×1080')} disabled={!hasProject || isProcessing}>IG 1080×1080</button>
                <button class="btn-ghost" onclick={() => runCmd('resize_canvas', { width: 1080, height: 1920, scaleLayers: true }, 'Magic Resize 1080×1920')} disabled={!hasProject || isProcessing}>Story 9:16</button>
                <button class="btn-ghost" onclick={() => runCmd('resize_canvas', { width: 1024, height: 1024, scaleLayers: true }, 'Magic Resize 1024×1024')} disabled={!hasProject || isProcessing}>Logo 1024</button>
              </div>
            </div>

          <!-- ══ MCP AI (Universal Auto-Connector) ══ -->
          {:else if activeTool === 'mcp'}
            <div class="panel">
              <McpPanel compact={true} {mcpIsActive} />
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

    <!-- ── Center Workspace (Contextual Bar + Canvas) ─────────────────────── -->
    <div class="center-col">
      <ContextualBar
        {hasProject}
        {selectedLayer}
        {selectedLayerIds}
        {canvasW}
        {canvasH}
        {brandColors}
        {recentColors}
        {fontStyles}
        onSaveBrandColor={saveBrandColor}
        onRemoveBrandColor={removeBrandColor}
        onCheckpoint={pushCheckpoint}
        onLiveTransform={liveUpdateTransform}
        onLiveText={liveUpdateText}
        onLiveShape={liveUpdateShape}
        onFlip={(id, horizontal) => runCmd('flip_layer', { layerId: id, horizontal }, horizontal ? 'Flip H' : 'Flip V')}
        onReplaceImage={replaceImageLayer}
        onQuickRemoveBg={quickRemoveBg}
        onDelete={(id) => runCmd('delete_layer', { layerId: id }, 'Delete Layer')}
        onDuplicate={(id) => runCmd('duplicate_layer', { layerId: id }, 'Duplicate Layer')}
        onDeleteMulti={(ids) => runCmd('delete_layers', { layerIds: ids }, 'Delete Selected')}
        onDuplicateMulti={(ids) => runCmd('duplicate_layers', { layerIds: ids }, 'Duplicate Selected')}
        onGroup={groupSelected}
        onAlign={(alignment) => runCmd('align_layers', { layerIds: selectedLayerIds, alignment }, `Align ${alignment}`)}
        onDistribute={(axis) => runCmd('distribute_layers', { layerIds: selectedLayerIds, axis }, `Distribute ${axis}`)}
        onFitScreen={fitToScreen}
        onQuickCanvasBg={(hex) => { trackRecentColor(hex); runCmd('set_canvas_background', { colorHex: hex }, 'Set BG'); }}
      />

      <div
        class="canvas-area"
        bind:this={canvasArea}
        use:wheelZoom
        ondragover={(e) => e.preventDefault()}
        ondrop={handleDrop}
        onpointerdown={(e) => {
          if (!cropMode && e.target === canvasArea) selectedLayerIds = [];
        }}
        role="presentation"
      >
        {#if showRulers && hasProject}
          <div
            class="ruler top"
            onclick={(e) => {
              const r = e.currentTarget.getBoundingClientRect();
              addGuide('v', (e.clientX - r.left) / zoom);
            }}
            title="Click to add vertical guide"
            role="presentation"
          >
            <span>Ruler · Click to add vertical guide</span>
          </div>
          <div
            class="ruler left"
            onclick={(e) => {
              const r = e.currentTarget.getBoundingClientRect();
              addGuide('h', (e.clientY - r.top) / zoom);
            }}
            title="Click to add horizontal guide"
            role="presentation"
          ></div>
        {/if}

        {#if hasProject}
          <div
            class="canvas-frame"
            class:transparent-checker={transparentBg}
            style="width:{canvasW * zoom}px; height:{canvasH * zoom}px;"
          >
            <img
              src={previewUrl}
              alt="Veltrax Studios Canvas"
              width={canvasW * zoom}
              height={canvasH * zoom}
              style="display:block; width:100%; height:100%; object-fit:fill; pointer-events:none;"
              draggable="false"
            />

          <CanvasGizmo
            {layers}
            {selectedLayerIds}
            {canvasW}
            {canvasH}
            {zoom}
            {cropMode}
            {cropRect}
            {cropAspect}
            {showGrid}
            {guides}
            onSelectLayers={(ids) => (selectedLayerIds = ids)}
            onDoubleClickText={handleDoubleClickText}
            onCommitTransform={commitGizmoTransform}
            onCommitMultiTransform={(updates) => runCmd('transform_layers', { updates }, 'Transform Layers')}
            onDeleteLayers={(ids) => runCmd('delete_layers', { layerIds: ids }, 'Delete Layers')}
            onDuplicateLayers={(ids) => runCmd('duplicate_layers', { layerIds: ids }, 'Duplicate Layers')}
            onGroupSelected={groupSelected}
            onUngroupSelected={ungroupSelected}
            onRemoveGuide={(id) => (guides = guides.filter((g) => g.id !== id))}
            onUpdateGuide={(id, pos) => (guides = guides.map((g) => (g.id === id ? { ...g, pos } : g)))}
            onCropRectChange={(r) => (cropRect = r)}
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
            <button class="zoom-btn" class:active-toggle={showGrid} onclick={() => (showGrid = !showGrid)} title="Toggle Grid"><Grid3x3 size={11} strokeWidth={2} /></button>
            <button class="zoom-btn" class:active-toggle={showRulers} onclick={() => (showRulers = !showRulers)} title="Toggle Rulers &amp; Guides"><Ruler size={11} strokeWidth={2} /></button>
          </div>
        </div>
      </div>
    </div>

    <InspectorPanel
      {hasProject}
      {project}
      {selectedLayer}
      {fontStyles}
      {filters}
      {brandColors}
      {recentColors}
      onSaveBrandColor={saveBrandColor}
      onRemoveBrandColor={removeBrandColor}
      onCheckpoint={pushCheckpoint}
      onLiveTransform={liveUpdateTransform}
      onLiveText={liveUpdateText}
      onLiveShape={liveUpdateShape}
      onLiveEffects={liveUpdateEffects}
      onLiveOutline={liveUpdateOutline}
      onFlip={(id, horizontal) => runCmd('flip_layer', { layerId: id, horizontal }, horizontal ? 'Flip H' : 'Flip V')}
      onResizeCanvas={(w, h) => runCmd('resize_canvas', { width: w, height: h, scaleLayers: true }, 'Resize Canvas')}
      onSetCanvasBg={(opts) => runCmd('set_canvas_background', opts, 'Set Canvas BG')}
    />
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
  .selected-ratio { border-color:var(--accent) !important; background:var(--accent-subtle) !important; color:var(--text-primary) !important; }
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

  .grad-preset-btn { display:flex; align-items:center; gap:6px; padding:7px 8px; border-radius:var(--r-sm); background:var(--bg-elevated); border:1.5px solid var(--border); color:var(--text-secondary); font-size:11px; text-align:left; }
  .grad-preset-btn:hover:not(:disabled) { border-color:var(--border-active); color:var(--text-primary); }
  .grad-dot { width:16px; height:16px; border-radius:4px; flex-shrink:0; border:1px solid var(--border); }

  .search-box { display:flex; align-items:center; gap:7px; background:var(--bg-input); border:1px solid var(--border-input); border-radius:var(--r-sm); padding:0 9px; color:var(--text-muted); }
  .search-box input { border:none; background:transparent; padding:7px 0; font-size:12px; }
  .search-box input:focus { border:none; }

  .lib-grid { display:grid; grid-template-columns:repeat(2,1fr); gap:8px; max-height:200px; overflow-y:auto; }
  .lib-card { position:relative; aspect-ratio:1/1; background:var(--bg-elevated); border:1px solid var(--border); border-radius:var(--r-sm); overflow:hidden; cursor:pointer; display:grid; place-items:center; padding:6px; }
  .lib-card:hover { border-color:var(--border-active); background:var(--bg-card-hover); }
  .lib-card img { max-width:100%; max-height:100%; object-fit:contain; }
  .lib-del { position:absolute; top:4px; right:4px; width:18px; height:18px; border-radius:50%; background:rgba(0,0,0,0.7); color:#fff; display:grid; place-items:center; opacity:0; transition:opacity 0.15s; padding:0; }
  .lib-card:hover .lib-del { opacity:1; }

  .tier-stack { display:flex; flex-direction:column; gap:6px; }
  .tier-btn { text-align:left; padding:9px 11px; border-radius:var(--r-sm); background:var(--bg-elevated); border:1px solid var(--border); color:var(--text-primary); }
  .tier-btn:hover:not(:disabled) { border-color:var(--border-active); background:var(--bg-card-hover); }
  .tier-h1 { font-size:17px; font-weight:800; }
  .tier-h2 { font-size:14px; font-weight:600; }
  .tier-body { font-size:12px; font-weight:400; color:var(--text-secondary); }

  .group-list { display:flex; flex-direction:column; gap:4px; }
  .group-row { display:flex; align-items:center; justify-content:space-between; padding:6px 8px; border-radius:var(--r-sm); background:rgba(56,189,248,0.08); border:1px solid rgba(56,189,248,0.25); cursor:pointer; }
  .group-name { font-size:11px; font-weight:700; color:#38bdf8; }
  .grp-tag { display:inline-block; font-size:9px; font-weight:700; background:rgba(56,189,248,0.2); color:#38bdf8; padding:0 4px; border-radius:3px; margin-right:5px; }

  .ruler { position:absolute; background:var(--bg-sidebar); border:1px solid var(--border); z-index:45; display:flex; align-items:center; justify-content:center; font-size:9px; color:var(--text-muted); cursor:crosshair; }
  .ruler.top { top:0; left:18px; right:0; height:18px; }
  .ruler.left { top:18px; left:0; bottom:0; width:18px; }
  .zoom-btn.active-toggle { border-color:#38bdf8; color:#38bdf8; background:rgba(56,189,248,0.16); }

  .checkbox-row { display:flex; align-items:center; gap:8px; font-size:12px; color:var(--text-primary); cursor:pointer; }
  .checkbox-row input { width:auto; accent-color:var(--accent); }

  .diag-box { display:flex; flex-direction:column; gap:8px; background:var(--bg-elevated); border:1px solid var(--border); border-radius:var(--r-sm); padding:10px; }
  .diag-item { display:flex; align-items:center; gap:8px; font-size:12px; color:var(--text-secondary); }
  .status-dot { width:8px; height:8px; border-radius:50%; flex-shrink:0; }
  .status-dot.active { background:#10b981; box-shadow:0 0 8px rgba(16,185,129,0.6); }
  .status-dot.standby { background:#f59e0b; box-shadow:0 0 6px rgba(245,158,11,0.4); }
  .status-dot.off { background:#ef4444; box-shadow:0 0 6px rgba(239,68,68,0.4); }
  .restart-note { font-size:11px; color:#fbbf24; }
  .connect-result { font-size:11px; color:#34d399; line-height:1.5; white-space:pre-wrap; }

  .center-col { flex:1; min-width:0; min-height:0; display:flex; flex-direction:column; overflow:hidden; }
  .canvas-area { flex:1; min-width:0; min-height:0; position:relative; overflow:auto; display:flex; align-items:center; justify-content:center; }
  .canvas-frame { position:relative; flex-shrink:0; border-radius:4px; overflow:visible; box-shadow:0 0 0 1px rgba(255,255,255,0.08),0 24px 60px rgba(0,0,0,0.6); }
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
  .processing-overlay { position:absolute; inset:0; background:rgba(13,15,18,0.7); backdrop-filter:blur(4px); display:grid; place-items:center; font-size:13px; font-weight:600; color:var(--text-secondary); z-index:50; }
  .empty-canvas { display:flex; flex-direction:column; align-items:center; gap:14px; text-align:center; color:var(--text-secondary); max-width:320px; }
  .empty-canvas h3 { font-size:17px; color:var(--text-primary); }
  .empty-canvas p { font-size:13px; }
  .empty-canvas button { display:flex; align-items:center; gap:6px; }

  .bottom-bar { position:absolute; bottom:14px; left:50%; transform:translateX(-50%); display:flex; align-items:center; gap:14px; background:var(--bg-elevated); border:1px solid var(--border); border-radius:999px; padding:6px 14px; z-index:60; }
  .step-pill { font-size:12px; color:var(--text-secondary); display:flex; align-items:center; gap:6px; white-space:nowrap; }
  .err-dot { color:#f87171; font-weight:700; }
  .zoom-controls { display:flex; align-items:center; gap:5px; }
  .zoom-btn { width:22px; height:22px; display:grid; place-items:center; border-radius:50%; background:var(--bg-card-hover); border:1px solid var(--border); color:var(--text-secondary); font-size:14px; line-height:1; padding:0; }
  .zoom-pct { font-size:12px; font-variant-numeric:tabular-nums; color:var(--text-primary); min-width:34px; text-align:center; }
</style>
