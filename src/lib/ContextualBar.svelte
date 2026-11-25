<script>
  import {
    Bold, Italic, AlignLeft, AlignCenter, AlignRight,
    FlipHorizontal, FlipVertical, ImagePlus, Eraser,
    Trash2, Copy, Maximize2, FolderPlus,
    AlignStartVertical, AlignCenterVertical, AlignEndVertical,
    AlignStartHorizontal, AlignCenterHorizontal, AlignEndHorizontal,
    Columns3, Rows3
  } from 'lucide-svelte';
  import ScrubNumber from './ScrubNumber.svelte';
  import ColorPickerPopover from './ColorPickerPopover.svelte';

  let {
    hasProject = false,
    selectedLayer = null,
    selectedLayerIds = [],
    canvasW = 1280,
    canvasH = 720,
    brandColors = [],
    recentColors = [],
    fontStyles = [],
    onSaveBrandColor,
    onRemoveBrandColor,
    onCheckpoint,
    onLiveTransform,
    onLiveText,
    onLiveShape,
    onFlip,
    onReplaceImage,
    onQuickRemoveBg,
    onDelete,
    onDuplicate,
    onDeleteMulti,
    onDuplicateMulti,
    onGroup,
    onAlign,
    onDistribute,
    onFitScreen,
    onQuickCanvasBg,
  } = $props();

  function rgbaToHex([r, g, b]) {
    return `#${[r, g, b].map((v) => v.toString(16).padStart(2, '0')).join('').toUpperCase()}`;
  }

  let textSize = $state(96);
  let textColor = $state('#FFFFFF');
  let shapeFill = $state('#6366F1');
  let shapeBorder = $state('#FFFFFF');
  let shapeRadius = $state(16);
  let layerOpacityPct = $state(100);
  let canvasBgHex = $state('#1A1D26');

  $effect(() => {
    if (selectedLayer) {
      layerOpacityPct = Math.round((selectedLayer.opacity ?? 1) * 100);
      if (selectedLayer.kind?.type === 'Text3D') {
        textSize = Math.round(selectedLayer.kind.size);
        textColor = rgbaToHex(selectedLayer.kind.color);
      } else if (selectedLayer.kind?.type === 'Shape') {
        shapeFill = rgbaToHex(selectedLayer.kind.fill_color);
        shapeBorder = rgbaToHex(selectedLayer.kind.border_color ?? [255, 255, 255, 255]);
        shapeRadius = selectedLayer.kind.border_radius ?? 0;
      }
    }
  });
</script>

<div class="ctx-bar">
  {#if !hasProject}
    <span class="ctx-muted">Open or create a design to start editing</span>
  {:else if !selectedLayer}
    <div class="ctx-group">
      <span class="ctx-tag">Canvas</span>
      <span class="ctx-dim">{canvasW} × {canvasH} px</span>
    </div>
    <div class="ctx-sep"></div>
    <div class="ctx-group">
      <span class="ctx-lbl">BG</span>
      <ColorPickerPopover
        compact={true}
        bind:color={canvasBgHex}
        {brandColors}
        {recentColors}
        {onSaveBrandColor}
        {onRemoveBrandColor}
        onChange={(hex) => onQuickCanvasBg(hex)}
      />
    </div>
    <div class="ctx-spacer"></div>
    <button class="ctx-btn" onclick={onFitScreen} title="Fit canvas to screen">
      <Maximize2 size={13} strokeWidth={1.75} /> Fit
    </button>
  {:else if selectedLayerIds.length > 1}
    <div class="ctx-group">
      <span class="ctx-tag">{selectedLayerIds.length} Selected</span>
      <button class="ctx-btn" onclick={onGroup} title="Group layers (Ctrl+G)">
        <FolderPlus size={13} strokeWidth={1.75} /> Group
      </button>
    </div>
    <div class="ctx-sep"></div>
    <div class="ctx-group">
      <button class="ctx-icon" onclick={() => onAlign('left')} title="Align Left"><AlignStartVertical size={13} /></button>
      <button class="ctx-icon" onclick={() => onAlign('center')} title="Align Center"><AlignCenterVertical size={13} /></button>
      <button class="ctx-icon" onclick={() => onAlign('right')} title="Align Right"><AlignEndVertical size={13} /></button>
      <button class="ctx-icon" onclick={() => onAlign('top')} title="Align Top"><AlignStartHorizontal size={13} /></button>
      <button class="ctx-icon" onclick={() => onAlign('middle')} title="Align Middle"><AlignCenterHorizontal size={13} /></button>
      <button class="ctx-icon" onclick={() => onAlign('bottom')} title="Align Bottom"><AlignEndHorizontal size={13} /></button>
    </div>
    {#if selectedLayerIds.length >= 3}
      <div class="ctx-sep"></div>
      <div class="ctx-group">
        <button class="ctx-icon" onclick={() => onDistribute('horizontal')} title="Distribute Horizontally"><Columns3 size={13} /></button>
        <button class="ctx-icon" onclick={() => onDistribute('vertical')} title="Distribute Vertically"><Rows3 size={13} /></button>
      </div>
    {/if}
    <div class="ctx-spacer"></div>
    <button class="ctx-icon" onclick={() => onDuplicateMulti(selectedLayerIds)} title="Duplicate Selected"><Copy size={13} strokeWidth={1.75} /></button>
    <button class="ctx-icon danger" onclick={() => onDeleteMulti(selectedLayerIds)} title="Delete Selected"><Trash2 size={13} strokeWidth={1.75} /></button>
  {:else if selectedLayer.kind?.type === 'Text3D'}
    {@const k = selectedLayer.kind}
    <div class="ctx-group">
      <select
        class="ctx-select"
        value={k.font_style}
        onchange={(e) => onLiveText({ font_style: e.currentTarget.value }, true)}
      >
        {#each fontStyles as fs}
          <option value={fs.id}>{fs.label}</option>
        {/each}
      </select>
      <div class="ctx-num-sm">
        <ScrubNumber
          bind:value={textSize}
          min={8}
          max={400}
          onScrubStart={onCheckpoint}
          onLiveChange={(v) => onLiveText({ size: v }, false)}
          onCommit={(v) => onLiveText({ size: v }, false)}
        />
      </div>
      <ColorPickerPopover
        compact={true}
        bind:color={textColor}
        {brandColors}
        {recentColors}
        {onSaveBrandColor}
        {onRemoveBrandColor}
        onChange={(hex) => onLiveText({ color_hex: hex }, true)}
      />
    </div>
    <div class="ctx-sep"></div>
    <div class="ctx-group">
      <button
        class="ctx-icon"
        class:active={k.bold}
        onclick={() => onLiveText({ bold: !k.bold }, true)}
        title="Bold"
      >
        <Bold size={13} strokeWidth={2} />
      </button>
      <button
        class="ctx-icon"
        class:active={k.italic}
        onclick={() => onLiveText({ italic: !k.italic }, true)}
        title="Italic"
      >
        <Italic size={13} strokeWidth={2} />
      </button>
    </div>
    <div class="ctx-sep"></div>
    <div class="ctx-group">
      <button class="ctx-icon" class:active={(k.align ?? 'left') === 'left'} onclick={() => onLiveText({ align: 'left' }, true)} title="Align Left">
        <AlignLeft size={13} strokeWidth={1.75} />
      </button>
      <button class="ctx-icon" class:active={k.align === 'center'} onclick={() => onLiveText({ align: 'center' }, true)} title="Align Center">
        <AlignCenter size={13} strokeWidth={1.75} />
      </button>
      <button class="ctx-icon" class:active={k.align === 'right'} onclick={() => onLiveText({ align: 'right' }, true)} title="Align Right">
        <AlignRight size={13} strokeWidth={1.75} />
      </button>
    </div>
    <div class="ctx-spacer"></div>
    <button class="ctx-icon" onclick={() => onDuplicate(selectedLayer.id)} title="Duplicate"><Copy size={13} strokeWidth={1.75} /></button>
    <button class="ctx-icon danger" onclick={() => onDelete(selectedLayer.id)} title="Delete"><Trash2 size={13} strokeWidth={1.75} /></button>

  {:else if selectedLayer.kind?.type === 'Image'}
    <div class="ctx-group">
      <button class="ctx-btn" onclick={() => onReplaceImage(selectedLayer.id)} title="Replace image source">
        <ImagePlus size={13} strokeWidth={1.75} /> Replace
      </button>
      <button class="ctx-btn" onclick={() => onFlip(selectedLayer.id, true)} title="Flip Horizontal">
        <FlipHorizontal size={13} strokeWidth={1.75} /> Flip H
      </button>
      <button class="ctx-btn" onclick={() => onFlip(selectedLayer.id, false)} title="Flip Vertical">
        <FlipVertical size={13} strokeWidth={1.75} /> Flip V
      </button>
      <button class="ctx-btn" onclick={() => onQuickRemoveBg(selectedLayer.id)} title="Remove background color">
        <Eraser size={13} strokeWidth={1.75} /> Remove BG
      </button>
    </div>
    <div class="ctx-sep"></div>
    <div class="ctx-group">
      <span class="ctx-lbl">Opacity</span>
      <div class="ctx-num-sm">
        <ScrubNumber
          bind:value={layerOpacityPct}
          min={0}
          max={100}
          suffix="%"
          onScrubStart={onCheckpoint}
          onLiveChange={(v) => onLiveTransform({ opacity: v / 100 }, false)}
          onCommit={(v) => onLiveTransform({ opacity: v / 100 }, false)}
        />
      </div>
    </div>
    <div class="ctx-spacer"></div>
    <button class="ctx-icon" onclick={() => onDuplicate(selectedLayer.id)} title="Duplicate"><Copy size={13} strokeWidth={1.75} /></button>
    <button class="ctx-icon danger" onclick={() => onDelete(selectedLayer.id)} title="Delete"><Trash2 size={13} strokeWidth={1.75} /></button>

  {:else if selectedLayer.kind?.type === 'Shape'}
    <div class="ctx-group">
      <span class="ctx-lbl">Fill</span>
      <ColorPickerPopover
        compact={true}
        bind:color={shapeFill}
        {brandColors}
        {recentColors}
        {onSaveBrandColor}
        {onRemoveBrandColor}
        onChange={(hex) => onLiveShape({ fill_hex: hex }, true)}
      />
      <span class="ctx-lbl">Border</span>
      <ColorPickerPopover
        compact={true}
        bind:color={shapeBorder}
        {brandColors}
        {recentColors}
        {onSaveBrandColor}
        {onRemoveBrandColor}
        onChange={(hex) => onLiveShape({ border_hex: hex, border_width: Math.max(2, selectedLayer.kind.border_width || 0) }, true)}
      />
      <span class="ctx-lbl">Radius</span>
      <div class="ctx-num-sm">
        <ScrubNumber
          bind:value={shapeRadius}
          min={0}
          max={250}
          onScrubStart={onCheckpoint}
          onLiveChange={(v) => onLiveShape({ border_radius: v }, false)}
          onCommit={(v) => onLiveShape({ border_radius: v }, false)}
        />
      </div>
    </div>
    <div class="ctx-sep"></div>
    <div class="ctx-group">
      <span class="ctx-lbl">Opacity</span>
      <div class="ctx-num-sm">
        <ScrubNumber
          bind:value={layerOpacityPct}
          min={0}
          max={100}
          suffix="%"
          onScrubStart={onCheckpoint}
          onLiveChange={(v) => onLiveTransform({ opacity: v / 100 }, false)}
          onCommit={(v) => onLiveTransform({ opacity: v / 100 }, false)}
        />
      </div>
    </div>
    <div class="ctx-spacer"></div>
    <button class="ctx-icon" onclick={() => onDuplicate(selectedLayer.id)} title="Duplicate"><Copy size={13} strokeWidth={1.75} /></button>
    <button class="ctx-icon danger" onclick={() => onDelete(selectedLayer.id)} title="Delete"><Trash2 size={13} strokeWidth={1.75} /></button>

  {:else}
    <div class="ctx-group">
      <span class="ctx-tag">{selectedLayer.kind?.type ?? 'Layer'}</span>
      <span class="ctx-lbl">Opacity</span>
      <div class="ctx-num-sm">
        <ScrubNumber
          bind:value={layerOpacityPct}
          min={0}
          max={100}
          suffix="%"
          onScrubStart={onCheckpoint}
          onLiveChange={(v) => onLiveTransform({ opacity: v / 100 }, false)}
          onCommit={(v) => onLiveTransform({ opacity: v / 100 }, false)}
        />
      </div>
    </div>
    <div class="ctx-spacer"></div>
    <button class="ctx-icon" onclick={() => onDuplicate(selectedLayer.id)} title="Duplicate"><Copy size={13} strokeWidth={1.75} /></button>
    <button class="ctx-icon danger" onclick={() => onDelete(selectedLayer.id)} title="Delete"><Trash2 size={13} strokeWidth={1.75} /></button>
  {/if}
</div>

<style>
  .ctx-bar { height:40px; flex-shrink:0; background:var(--bg-card); border-bottom:1px solid var(--border); display:flex; align-items:center; padding:0 12px; gap:8px; overflow-x:auto; }
  .ctx-group { display:flex; align-items:center; gap:6px; flex-shrink:0; }
  .ctx-sep { width:1px; height:18px; background:var(--border); flex-shrink:0; }
  .ctx-spacer { flex:1; min-width:8px; }
  .ctx-muted { font-size:11px; color:var(--text-muted); }
  .ctx-tag { font-size:10px; font-weight:700; text-transform:uppercase; letter-spacing:0.05em; color:#38bdf8; background:rgba(56,189,248,0.12); padding:2px 7px; border-radius:4px; }
  .ctx-dim { font-size:11px; color:var(--text-secondary); font-family:var(--mono); }
  .ctx-lbl { font-size:10px; color:var(--text-secondary); font-weight:600; }
  .ctx-select { background:var(--bg-input); color:var(--text-primary); border:1px solid var(--border-input); border-radius:var(--r-sm); padding:4px 8px; font-size:11px; height:28px; outline:none; }
  .ctx-num-sm { width:62px; }
  .ctx-btn { display:flex; align-items:center; gap:5px; padding:4px 9px; height:28px; border-radius:var(--r-sm); background:var(--bg-elevated); border:1px solid var(--border); color:var(--text-secondary); font-size:11px; font-weight:500; }
  .ctx-btn:hover { color:var(--text-primary); border-color:#38bdf8; }
  .ctx-icon { width:28px; height:28px; display:grid; place-items:center; border-radius:var(--r-sm); background:var(--bg-elevated); border:1px solid var(--border); color:var(--text-secondary); padding:0; }
  .ctx-icon:hover { color:var(--text-primary); border-color:#38bdf8; }
  .ctx-icon.active { background:rgba(56,189,248,0.16); color:#38bdf8; border-color:#38bdf8; }
  .ctx-icon.danger:hover { color:#f87171; border-color:rgba(248,113,113,0.5); }
</style>
