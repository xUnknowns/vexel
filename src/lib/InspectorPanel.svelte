<script>
  import {
    ChevronDown, ChevronRight, Lock, Unlock,
    Bold, Italic, AlignLeft, AlignCenter, AlignRight,
    FlipHorizontal, FlipVertical
  } from 'lucide-svelte';
  import ScrubNumber from './ScrubNumber.svelte';
  import ColorPickerPopover from './ColorPickerPopover.svelte';

  let {
    hasProject = false,
    project = null,
    selectedLayer = null,
    fontStyles = [],
    filters = [],
    brandColors = [],
    recentColors = [],
    onSaveBrandColor,
    onRemoveBrandColor,
    onCheckpoint,
    onLiveTransform,
    onLiveText,
    onLiveShape,
    onLiveEffects,
    onLiveOutline,
    onFlip,
    onResizeCanvas,
    onSetCanvasBg,
  } = $props();

  let secPos = $state(true);
  let secApp = $state(true);
  let secText = $state(true);
  let secShape = $state(true);
  let secFx = $state(true);
  let secColor = $state(false);
  let aspectLocked = $state(true);

  function rgbaToHex([r, g, b]) {
    return `#${[r, g, b].map((v) => v.toString(16).padStart(2, '0')).join('').toUpperCase()}`;
  }

  // Local bound fields synced with selectedLayer
  let lx = $state(0);
  let ly = $state(0);
  let lw = $state(100);
  let lh = $state(100);
  let lrot = $state(0);
  let lopacity = $state(100);
  let lblend = $state('normal');

  // Text
  let tContent = $state('');
  let tFont = $state('bold_display');
  let tSize = $state(96);
  let tColor = $state('#FFFFFF');
  let tBold = $state(false);
  let tItalic = $state(false);
  let tAlign = $state('left');
  let tLineH = $state(1.2);
  let tLetterS = $state(0);
  let tStrokeColor = $state('#000000');
  let tStrokeW = $state(0);
  let tDepth = $state(0);

  // Shape
  let sWidth = $state(240);
  let sHeight = $state(240);
  let sFill = $state('#6366F1');
  let sRadius = $state(16);
  let sBorderColor = $state('#FFFFFF');
  let sBorderW = $state(0);

  // Effects & Outline
  let outEnabled = $state(false);
  let outColor = $state('#FFFFFF');
  let outThickness = $state(6);
  let fxBrightness = $state(0);
  let fxContrast = $state(0);
  let fxSaturation = $state(0);
  let fxBlur = $state(0);
  let fxFilter = $state('original');

  // Canvas (no selection)
  let cWidth = $state(1280);
  let cHeight = $state(720);
  let cBgHex = $state('#FFFFFF');

  $effect(() => {
    if (project) {
      cWidth = project.canvas_width;
      cHeight = project.canvas_height;
      if (project.bg_color) cBgHex = rgbaToHex(project.bg_color);
    }
  });

  $effect(() => {
    if (selectedLayer) {
      lx = selectedLayer.x;
      ly = selectedLayer.y;
      const bw = Math.max(1, selectedLayer.bounds_width || 100);
      const bh = Math.max(1, selectedLayer.bounds_height || 100);
      lw = Math.round(bw * selectedLayer.scale);
      lh = Math.round(bh * selectedLayer.scale);
      lrot = Math.round(selectedLayer.rotation);
      lopacity = Math.round((selectedLayer.opacity ?? 1) * 100);
      lblend = selectedLayer.blend_mode ?? 'normal';

      outEnabled = selectedLayer.outline?.enabled ?? false;
      outThickness = selectedLayer.outline?.thickness ?? 6;
      outColor = rgbaToHex(selectedLayer.outline?.color ?? [255, 255, 255, 255]);

      fxBrightness = selectedLayer.effects?.brightness ?? 0;
      fxContrast = selectedLayer.effects?.contrast ?? 0;
      fxSaturation = selectedLayer.effects?.saturation ?? 0;
      fxBlur = selectedLayer.effects?.blur ?? 0;
      fxFilter = selectedLayer.effects?.filter ?? 'original';

      if (selectedLayer.kind?.type === 'Text3D') {
        tContent = selectedLayer.kind.content;
        tFont = selectedLayer.kind.font_style;
        tSize = Math.round(selectedLayer.kind.size);
        tColor = rgbaToHex(selectedLayer.kind.color);
        tBold = Boolean(selectedLayer.kind.bold);
        tItalic = Boolean(selectedLayer.kind.italic);
        tAlign = selectedLayer.kind.align ?? 'left';
        tLineH = selectedLayer.kind.line_height ?? 1.2;
        tLetterS = selectedLayer.kind.letter_spacing ?? 0;
        tStrokeColor = rgbaToHex(selectedLayer.kind.stroke_color);
        tStrokeW = selectedLayer.kind.stroke_width;
        tDepth = selectedLayer.kind.depth;
      } else if (selectedLayer.kind?.type === 'Shape') {
        sWidth = selectedLayer.kind.width;
        sHeight = selectedLayer.kind.height;
        sFill = rgbaToHex(selectedLayer.kind.fill_color);
        sRadius = selectedLayer.kind.border_radius;
        sBorderColor = rgbaToHex(selectedLayer.kind.border_color ?? [255, 255, 255, 255]);
        sBorderW = selectedLayer.kind.border_width ?? 0;
      }
    }
  });

  function handleWidthChange(newW) {
    if (!selectedLayer) return;
    const bw = Math.max(1, selectedLayer.bounds_width || 100);
    const bh = Math.max(1, selectedLayer.bounds_height || 100);
    const nextScale = Math.max(0.05, Math.min(8.0, newW / bw));
    if (aspectLocked) lh = Math.round(bh * nextScale);
    onLiveTransform({ scale: nextScale }, false);
  }

  function handleHeightChange(newH) {
    if (!selectedLayer) return;
    const bw = Math.max(1, selectedLayer.bounds_width || 100);
    const bh = Math.max(1, selectedLayer.bounds_height || 100);
    const nextScale = Math.max(0.05, Math.min(8.0, newH / bh));
    if (aspectLocked) lw = Math.round(bw * nextScale);
    onLiveTransform({ scale: nextScale }, false);
  }
</script>

<aside class="inspector">
  <div class="insp-header">
    <span>{selectedLayer ? selectedLayer.name : 'Canvas Properties'}</span>
    {#if selectedLayer}
      <span class="insp-type">{selectedLayer.kind?.type}</span>
    {/if}
  </div>

  <div class="insp-scroll">
    {#if !hasProject}
      <p class="insp-empty">No active project.</p>
    {:else if !selectedLayer}
      <!-- ══ CANVAS PROPERTIES ══ -->
      <div class="sec">
        <div class="sec-title-static">Dimensions</div>
        <div class="grid-2">
          <ScrubNumber label="Width" bind:value={cWidth} min={1} max={8192} />
          <ScrubNumber label="Height" bind:value={cHeight} min={1} max={8192} />
        </div>
        <button class="btn-primary fw sm" onclick={() => onResizeCanvas(cWidth, cHeight)}>
          Apply Canvas Size
        </button>
      </div>

      <div class="sec">
        <div class="sec-title-static">Background</div>
        <ColorPickerPopover
          label="Canvas Color"
          bind:color={cBgHex}
          {brandColors}
          {recentColors}
          {onSaveBrandColor}
          {onRemoveBrandColor}
          onChange={(hex) => onSetCanvasBg({ colorHex: hex })}
        />
        <button class="btn-ghost fw sm" onclick={() => onSetCanvasBg({ transparent: true })}>
          Set Transparent PNG
        </button>
      </div>
    {:else}
      <!-- ══ POSITION & SIZE ══ -->
      <div class="sec">
        <button class="sec-head" onclick={() => (secPos = !secPos)}>
          {#if secPos}<ChevronDown size={13} />{:else}<ChevronRight size={13} />{/if}
          <span>Position &amp; Size</span>
        </button>
        {#if secPos}
          <div class="grid-2">
            <ScrubNumber
              label="X"
              bind:value={lx}
              disabled={selectedLayer.locked}
              onScrubStart={onCheckpoint}
              onLiveChange={(v) => onLiveTransform({ x: v }, false)}
              onCommit={(v) => onLiveTransform({ x: v }, false)}
            />
            <ScrubNumber
              label="Y"
              bind:value={ly}
              disabled={selectedLayer.locked}
              onScrubStart={onCheckpoint}
              onLiveChange={(v) => onLiveTransform({ y: v }, false)}
              onCommit={(v) => onLiveTransform({ y: v }, false)}
            />
          </div>
          <div class="size-row">
            <ScrubNumber
              label="W"
              bind:value={lw}
              min={4}
              max={8192}
              disabled={selectedLayer.locked}
              onScrubStart={onCheckpoint}
              onLiveChange={handleWidthChange}
              onCommit={handleWidthChange}
            />
            <button
              class="lock-btn"
              class:active={aspectLocked}
              onclick={() => (aspectLocked = !aspectLocked)}
              title="Lock aspect ratio"
            >
              {#if aspectLocked}<Lock size={12} />{:else}<Unlock size={12} />{/if}
            </button>
            <ScrubNumber
              label="H"
              bind:value={lh}
              min={4}
              max={8192}
              disabled={selectedLayer.locked}
              onScrubStart={onCheckpoint}
              onLiveChange={handleHeightChange}
              onCommit={handleHeightChange}
            />
          </div>
          <div class="grid-2">
            <ScrubNumber
              label="Rotate °"
              bind:value={lrot}
              min={-360}
              max={360}
              disabled={selectedLayer.locked}
              onScrubStart={onCheckpoint}
              onLiveChange={(v) => onLiveTransform({ rotation: v }, false)}
              onCommit={(v) => onLiveTransform({ rotation: v }, false)}
            />
            <div class="flip-col">
              <span class="mini-lbl">Flip</span>
              <div class="btn-row">
                <button class="icon-sq" onclick={() => onFlip(selectedLayer.id, true)} disabled={selectedLayer.locked} title="Flip Horizontal">
                  <FlipHorizontal size={13} />
                </button>
                <button class="icon-sq" onclick={() => onFlip(selectedLayer.id, false)} disabled={selectedLayer.locked} title="Flip Vertical">
                  <FlipVertical size={13} />
                </button>
              </div>
            </div>
          </div>
        {/if}
      </div>

      <!-- ══ APPEARANCE ══ -->
      <div class="sec">
        <button class="sec-head" onclick={() => (secApp = !secApp)}>
          {#if secApp}<ChevronDown size={13} />{:else}<ChevronRight size={13} />{/if}
          <span>Appearance</span>
        </button>
        {#if secApp}
          <div class="slider-row">
            <div class="slider-label"><span>Opacity</span><span class="mono">{lopacity}%</span></div>
            <input
              type="range"
              min="0"
              max="100"
              step="1"
              bind:value={lopacity}
              onpointerdown={onCheckpoint}
              oninput={() => onLiveTransform({ opacity: lopacity / 100 }, false)}
            />
          </div>
          <div class="field-col">
            <span class="mini-lbl">Blend Mode</span>
            <select
              class="select-input"
              value={lblend}
              onchange={(e) => onLiveTransform({ blend_mode: e.currentTarget.value }, true)}
            >
              <option value="normal">Normal</option>
              <option value="multiply">Multiply</option>
              <option value="screen">Screen</option>
              <option value="overlay">Overlay</option>
            </select>
          </div>
        {/if}
      </div>

      <!-- ══ TEXT (Live Edit) ══ -->
      {#if selectedLayer.kind?.type === 'Text3D'}
        <div class="sec">
          <button class="sec-head" onclick={() => (secText = !secText)}>
            {#if secText}<ChevronDown size={13} />{:else}<ChevronRight size={13} />{/if}
            <span>Text (Live)</span>
          </button>
          {#if secText}
            <textarea
              class="text-area"
              rows="2"
              bind:value={tContent}
              onfocus={onCheckpoint}
              oninput={() => onLiveText({ content: tContent }, false)}
            ></textarea>

            <select
              class="select-input"
              value={tFont}
              onchange={(e) => onLiveText({ font_style: e.currentTarget.value }, true)}
            >
              {#each fontStyles as fs}
                <option value={fs.id}>{fs.label}</option>
              {/each}
            </select>

            <div class="grid-2">
              <ScrubNumber
                label="Size"
                bind:value={tSize}
                min={8}
                max={400}
                onScrubStart={onCheckpoint}
                onLiveChange={(v) => onLiveText({ size: v }, false)}
                onCommit={(v) => onLiveText({ size: v }, false)}
              />
              <ColorPickerPopover
                label="Color"
                bind:color={tColor}
                {brandColors}
                {recentColors}
                {onSaveBrandColor}
                {onRemoveBrandColor}
                onChange={(hex) => onLiveText({ color_hex: hex }, true)}
              />
            </div>

            <div class="btn-row">
              <button class="icon-sq" class:active={tBold} onclick={() => onLiveText({ bold: !tBold }, true)} title="Bold"><Bold size={13} /></button>
              <button class="icon-sq" class:active={tItalic} onclick={() => onLiveText({ italic: !tItalic }, true)} title="Italic"><Italic size={13} /></button>
              <div class="v-sep"></div>
              <button class="icon-sq" class:active={tAlign === 'left'} onclick={() => onLiveText({ align: 'left' }, true)} title="Left"><AlignLeft size={13} /></button>
              <button class="icon-sq" class:active={tAlign === 'center'} onclick={() => onLiveText({ align: 'center' }, true)} title="Center"><AlignCenter size={13} /></button>
              <button class="icon-sq" class:active={tAlign === 'right'} onclick={() => onLiveText({ align: 'right' }, true)} title="Right"><AlignRight size={13} /></button>
            </div>

            <div class="grid-2">
              <ScrubNumber
                label="Line Height"
                bind:value={tLineH}
                min={0.7}
                max={3.0}
                step={0.1}
                onScrubStart={onCheckpoint}
                onLiveChange={(v) => onLiveText({ line_height: v }, false)}
                onCommit={(v) => onLiveText({ line_height: v }, false)}
              />
              <ScrubNumber
                label="Letter Spc"
                bind:value={tLetterS}
                min={-10}
                max={60}
                step={1}
                onScrubStart={onCheckpoint}
                onLiveChange={(v) => onLiveText({ letter_spacing: v }, false)}
                onCommit={(v) => onLiveText({ letter_spacing: v }, false)}
              />
            </div>

            <div class="grid-2">
              <ScrubNumber
                label="Stroke W"
                bind:value={tStrokeW}
                min={0}
                max={10}
                onScrubStart={onCheckpoint}
                onLiveChange={(v) => onLiveText({ stroke_width: v }, false)}
                onCommit={(v) => onLiveText({ stroke_width: v }, false)}
              />
              <ColorPickerPopover
                label="Stroke"
                bind:color={tStrokeColor}
                {brandColors}
                {recentColors}
                {onSaveBrandColor}
                {onRemoveBrandColor}
                onChange={(hex) => onLiveText({ stroke_hex: hex }, true)}
              />
            </div>

            <ScrubNumber
              label="3D Extrude Depth"
              bind:value={tDepth}
              min={0}
              max={30}
              onScrubStart={onCheckpoint}
              onLiveChange={(v) => onLiveText({ depth_3d: v }, false)}
              onCommit={(v) => onLiveText({ depth_3d: v }, false)}
            />
          {/if}
        </div>
      {/if}

      <!-- ══ SHAPE FILL & BORDER ══ -->
      {#if selectedLayer.kind?.type === 'Shape'}
        <div class="sec">
          <button class="sec-head" onclick={() => (secShape = !secShape)}>
            {#if secShape}<ChevronDown size={13} />{:else}<ChevronRight size={13} />{/if}
            <span>Fill &amp; Border</span>
          </button>
          {#if secShape}
            <div class="grid-2">
              <ColorPickerPopover
                label="Fill"
                bind:color={sFill}
                {brandColors}
                {recentColors}
                {onSaveBrandColor}
                {onRemoveBrandColor}
                onChange={(hex) => onLiveShape({ fill_hex: hex }, true)}
              />
              <ScrubNumber
                label="Radius"
                bind:value={sRadius}
                min={0}
                max={250}
                onScrubStart={onCheckpoint}
                onLiveChange={(v) => onLiveShape({ border_radius: v }, false)}
                onCommit={(v) => onLiveShape({ border_radius: v }, false)}
              />
            </div>
            <div class="grid-2">
              <ColorPickerPopover
                label="Border"
                bind:color={sBorderColor}
                {brandColors}
                {recentColors}
                {onSaveBrandColor}
                {onRemoveBrandColor}
                onChange={(hex) => onLiveShape({ border_hex: hex }, true)}
              />
              <ScrubNumber
                label="Border W"
                bind:value={sBorderW}
                min={0}
                max={32}
                onScrubStart={onCheckpoint}
                onLiveChange={(v) => onLiveShape({ border_width: v }, false)}
                onCommit={(v) => onLiveShape({ border_width: v }, false)}
              />
            </div>
          {/if}
        </div>
      {/if}

      <!-- ══ EFFECTS & OUTLINE ══ -->
      <div class="sec">
        <button class="sec-head" onclick={() => (secFx = !secFx)}>
          {#if secFx}<ChevronDown size={13} />{:else}<ChevronRight size={13} />{/if}
          <span>Effects &amp; Outline</span>
        </button>
        {#if secFx}
          <label class="checkbox-row">
            <input
              type="checkbox"
              checked={outEnabled}
              onchange={(e) => onLiveOutline({ enabled: e.currentTarget.checked }, true)}
            />
            <span>Outer Stroke / Glow</span>
          </label>
          {#if outEnabled}
            <div class="grid-2">
              <ColorPickerPopover
                label="Color"
                bind:color={outColor}
                {brandColors}
                {recentColors}
                {onSaveBrandColor}
                {onRemoveBrandColor}
                onChange={(hex) => onLiveOutline({ color_hex: hex }, true)}
              />
              <ScrubNumber
                label="Thickness"
                bind:value={outThickness}
                min={1}
                max={24}
                onScrubStart={onCheckpoint}
                onLiveChange={(v) => onLiveOutline({ thickness: v }, false)}
                onCommit={(v) => onLiveOutline({ thickness: v }, false)}
              />
            </div>
          {/if}
          <div class="slider-row">
            <div class="slider-label"><span>Blur</span><span class="mono">{fxBlur}</span></div>
            <input
              type="range"
              min="0"
              max="20"
              step="0.5"
              bind:value={fxBlur}
              onpointerdown={onCheckpoint}
              oninput={() => onLiveEffects({ blur: fxBlur }, false)}
            />
          </div>
        {/if}
      </div>

      <!-- ══ COLOR ADJUSTMENTS & FILTERS ══ -->
      <div class="sec">
        <button class="sec-head" onclick={() => (secColor = !secColor)}>
          {#if secColor}<ChevronDown size={13} />{:else}<ChevronRight size={13} />{/if}
          <span>Color &amp; Filters</span>
        </button>
        {#if secColor}
          <div class="slider-row">
            <div class="slider-label"><span>Brightness</span><span class="mono">{fxBrightness}</span></div>
            <input type="range" min="-100" max="100" step="1" bind:value={fxBrightness} onpointerdown={onCheckpoint} oninput={() => onLiveEffects({ brightness: fxBrightness }, false)} />
          </div>
          <div class="slider-row">
            <div class="slider-label"><span>Contrast</span><span class="mono">{fxContrast}</span></div>
            <input type="range" min="-100" max="100" step="1" bind:value={fxContrast} onpointerdown={onCheckpoint} oninput={() => onLiveEffects({ contrast: fxContrast }, false)} />
          </div>
          <div class="slider-row">
            <div class="slider-label"><span>Saturation</span><span class="mono">{fxSaturation}</span></div>
            <input type="range" min="-100" max="100" step="1" bind:value={fxSaturation} onpointerdown={onCheckpoint} oninput={() => onLiveEffects({ saturation: fxSaturation }, false)} />
          </div>
          <div class="filter-grid">
            {#each filters as f}
              <button
                class="filter-chip"
                class:selected={fxFilter === f.id}
                onclick={() => onLiveEffects({ filter: f.id }, true)}
              >
                <span class="f-dot" style={f.style}></span>
                <span>{f.label}</span>
              </button>
            {/each}
          </div>
        {/if}
      </div>
    {/if}
  </div>
</aside>

<style>
  .inspector { width:282px; flex-shrink:0; background:var(--bg-card); border-left:1px solid var(--border); display:flex; flex-direction:column; overflow:hidden; }
  .insp-header { height:40px; padding:0 14px; border-bottom:1px solid var(--border); display:flex; align-items:center; justify-content:space-between; gap:8px; font-size:12px; font-weight:700; flex-shrink:0; }
  .insp-header span:first-child { overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .insp-type { font-size:10px; color:#38bdf8; background:rgba(56,189,248,0.12); padding:2px 6px; border-radius:4px; flex-shrink:0; }
  .insp-scroll { flex:1; overflow-y:auto; padding:8px 12px 24px; display:flex; flex-direction:column; gap:6px; }
  .insp-empty { font-size:12px; color:var(--text-muted); padding:16px 4px; text-align:center; }

  .sec { border-bottom:1px solid var(--border); padding:8px 0 12px; display:flex; flex-direction:column; gap:9px; }
  .sec:last-child { border-bottom:none; }
  .sec-head { display:flex; align-items:center; gap:6px; background:transparent; color:var(--text-primary); font-size:11px; font-weight:700; text-transform:uppercase; letter-spacing:0.05em; padding:2px 0; text-align:left; }
  .sec-head:hover { color:#38bdf8; transform:none; }
  .sec-title-static { font-size:11px; font-weight:700; text-transform:uppercase; letter-spacing:0.05em; color:var(--text-secondary); }

  .grid-2 { display:grid; grid-template-columns:1fr 1fr; gap:8px; align-items:end; }
  .size-row { display:grid; grid-template-columns:1fr 26px 1fr; gap:5px; align-items:end; }
  .lock-btn { height:28px; display:grid; place-items:center; border-radius:var(--r-sm); background:var(--bg-elevated); border:1px solid var(--border); color:var(--text-muted); padding:0; }
  .lock-btn.active { color:#38bdf8; border-color:rgba(56,189,248,0.4); }

  .flip-col, .field-col { display:flex; flex-direction:column; gap:3px; }
  .mini-lbl { font-size:10px; font-weight:600; color:var(--text-secondary); }
  .btn-row { display:flex; align-items:center; gap:5px; }
  .icon-sq { width:28px; height:28px; display:grid; place-items:center; border-radius:var(--r-sm); background:var(--bg-elevated); border:1px solid var(--border); color:var(--text-secondary); padding:0; }
  .icon-sq:hover { color:var(--text-primary); border-color:#38bdf8; }
  .icon-sq.active { background:rgba(56,189,248,0.16); color:#38bdf8; border-color:#38bdf8; }
  .v-sep { width:1px; height:18px; background:var(--border); margin:0 2px; }

  .slider-row { display:flex; flex-direction:column; gap:4px; }
  .slider-label { display:flex; justify-content:space-between; font-size:11px; color:var(--text-secondary); }
  .mono { font-variant-numeric:tabular-nums; color:var(--text-primary); }
  .select-input { background:var(--bg-input); color:var(--text-primary); border:1px solid var(--border-input); border-radius:var(--r-sm); padding:6px 8px; font-size:12px; outline:none; width:100%; }
  .text-area { font-family:var(--font); font-size:12px; resize:vertical; min-height:48px; background:var(--bg-input); color:var(--text-primary); border:1px solid var(--border-input); border-radius:var(--r-sm); padding:7px 9px; outline:none; width:100%; }
  .checkbox-row { display:flex; align-items:center; gap:7px; font-size:12px; color:var(--text-primary); cursor:pointer; }
  .checkbox-row input { width:auto; accent-color:#38bdf8; }

  .filter-grid { display:grid; grid-template-columns:1fr 1fr; gap:6px; }
  .filter-chip { display:flex; align-items:center; gap:6px; padding:6px 8px; border-radius:var(--r-sm); background:var(--bg-elevated); border:1px solid var(--border); color:var(--text-secondary); font-size:11px; }
  .filter-chip.selected { border-color:#38bdf8; background:rgba(56,189,248,0.14); color:var(--text-primary); }
  .f-dot { width:14px; height:14px; border-radius:3px; flex-shrink:0; }

  .fw { width:100%; }
  .sm { font-size:11px; padding:6px 10px; }
</style>
