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
    brandFonts = [],
    filters = [],
    brandColors = [],
    recentColors = [],
    onSaveBrandColor,
    onRemoveBrandColor,
    onCheckpoint,
    onLiveTransform,
    onLiveText,
    onLiveTextAdvanced,
    onLiveShape,
    onLiveEffects,
    onLiveOutline,
    onLiveShadow,
    onLiveGlow,
    onLiveMask,
    onLiveClipToBelow,
    onAiRemoveBg,
    onResetAlphaMask,
    onFlip,
    onResizeCanvas,
    onSetCanvasBg,
  } = $props();

  let secPos = $state(true);
  let secApp = $state(true);
  let secMask = $state(false);
  let secText = $state(true);
  let secShape = $state(true);
  let secFx = $state(true);
  let secShadow = $state(false);
  let secGlow = $state(false);
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
  let tUnderline = $state(false);
  let tGradEnabled = $state(false);
  let tGradFrom = $state('#FFFFFF');
  let tGradTo = $state('#38BDF8');
  let tGradAngle = $state(90);
  let tCurve = $state(0);

  // Shape
  let sType = $state('rectangle');
  let sWidth = $state(240);
  let sHeight = $state(240);
  let sFill = $state('#6366F1');
  let sRadius = $state(16);
  let sBorderColor = $state('#FFFFFF');
  let sBorderW = $state(0);
  let sSides = $state(5);
  let sRadTL = $state(0);
  let sRadTR = $state(0);
  let sRadBR = $state(0);
  let sRadBL = $state(0);
  let sGradEnabled = $state(false);
  let sGradFrom = $state('#6366F1');
  let sGradTo = $state('#38BDF8');
  let sGradAngle = $state(45);

  // Mask & Clip
  let mShape = $state('none');
  let mRadius = $state(24);
  let mClip = $state(false);

  // Drop Shadow & Glow
  let shEnabled = $state(false);
  let shColor = $state('#000000');
  let shBlur = $state(12);
  let shDist = $state(8);
  let shAngle = $state(135);
  let shOpacity = $state(60);

  let ogEnabled = $state(false);
  let ogColor = $state('#38BDF8');
  let ogIntensity = $state(10);
  let igEnabled = $state(false);
  let igColor = $state('#FFFFFF');
  let igIntensity = $state(8);

  // Effects & Outline
  let outEnabled = $state(false);
  let outColor = $state('#FFFFFF');
  let outThickness = $state(6);
  let fxBrightness = $state(0);
  let fxContrast = $state(0);
  let fxSaturation = $state(0);
  let fxBlur = $state(0);
  let fxFilter = $state('original');
  let fxHue = $state(0);
  let fxTemp = $state(0);
  let fxSharp = $state(0);
  let fxVignette = $state(0);
  let fxLvlBlack = $state(0);
  let fxLvlWhite = $state(255);
  let fxLvlGamma = $state(1.0);
  let fxLut = $state('none');

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
      fxHue = selectedLayer.effects?.hue_shift ?? 0;
      fxTemp = selectedLayer.effects?.temperature ?? 0;
      fxSharp = selectedLayer.effects?.sharpness ?? 0;
      fxVignette = selectedLayer.effects?.vignette ?? 0;
      fxLvlBlack = selectedLayer.effects?.levels_black ?? 0;
      fxLvlWhite = selectedLayer.effects?.levels_white ?? 255;
      fxLvlGamma = selectedLayer.effects?.levels_gamma ?? 1.0;
      fxLut = selectedLayer.effects?.lut_preset ?? 'none';

      mShape = selectedLayer.mask_shape ?? 'none';
      mRadius = selectedLayer.mask_radius ?? 24;
      mClip = Boolean(selectedLayer.clip_to_below);

      shEnabled = Boolean(selectedLayer.shadow?.enabled);
      shColor = rgbaToHex(selectedLayer.shadow?.color ?? [0, 0, 0, 255]);
      shBlur = selectedLayer.shadow?.blur ?? 12;
      shDist = selectedLayer.shadow?.distance ?? 8;
      shAngle = selectedLayer.shadow?.angle ?? 135;
      shOpacity = selectedLayer.shadow?.opacity ?? 60;

      ogEnabled = Boolean(selectedLayer.outer_glow?.enabled);
      ogColor = rgbaToHex(selectedLayer.outer_glow?.color ?? [56, 189, 248, 255]);
      ogIntensity = selectedLayer.outer_glow?.intensity ?? 10;

      igEnabled = Boolean(selectedLayer.inner_glow?.enabled);
      igColor = rgbaToHex(selectedLayer.inner_glow?.color ?? [255, 255, 255, 255]);
      igIntensity = selectedLayer.inner_glow?.intensity ?? 8;

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
        tUnderline = Boolean(selectedLayer.kind.underline);
        tGradEnabled = Boolean(selectedLayer.kind.gradient_colors);
        if (selectedLayer.kind.gradient_colors) {
          tGradFrom = rgbaToHex(selectedLayer.kind.gradient_colors[0]);
          tGradTo = rgbaToHex(selectedLayer.kind.gradient_colors[1]);
        }
        tGradAngle = selectedLayer.kind.gradient_angle ?? 90;
        tCurve = selectedLayer.kind.curve_amount ?? 0;
      } else if (selectedLayer.kind?.type === 'Shape') {
        sType = selectedLayer.kind.shape_type ?? 'rectangle';
        sWidth = selectedLayer.kind.width;
        sHeight = selectedLayer.kind.height;
        sFill = rgbaToHex(selectedLayer.kind.fill_color);
        sRadius = selectedLayer.kind.border_radius;
        sBorderColor = rgbaToHex(selectedLayer.kind.border_color ?? [255, 255, 255, 255]);
        sBorderW = selectedLayer.kind.border_width ?? 0;
        sSides = selectedLayer.kind.sides ?? 5;
        const cr = selectedLayer.kind.corner_radii;
        sRadTL = cr ? cr[0] : sRadius;
        sRadTR = cr ? cr[1] : sRadius;
        sRadBR = cr ? cr[2] : sRadius;
        sRadBL = cr ? cr[3] : sRadius;
        sGradEnabled = Boolean(selectedLayer.kind.fill_gradient);
        if (selectedLayer.kind.fill_gradient) {
          sGradFrom = rgbaToHex(selectedLayer.kind.fill_gradient.from);
          sGradTo = rgbaToHex(selectedLayer.kind.fill_gradient.to);
          sGradAngle = selectedLayer.kind.fill_gradient.angle ?? 45;
        }
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
              <option value="difference">Difference</option>
              <option value="exclusion">Exclusion</option>
              <option value="color-dodge">Color Dodge</option>
              <option value="color-burn">Color Burn</option>
            </select>
          </div>
        {/if}
      </div>

      <!-- ══ MASK & CLIPPING ══ -->
      <div class="sec">
        <button class="sec-head" onclick={() => (secMask = !secMask)}>
          {#if secMask}<ChevronDown size={13} />{:else}<ChevronRight size={13} />{/if}
          <span>Mask &amp; Clipping</span>
        </button>
        {#if secMask}
          <div class="field-col">
            <span class="mini-lbl">Shape Mask</span>
            <select
              class="select-input"
              value={mShape}
              onchange={(e) => {
                const val = e.currentTarget.value;
                mShape = val;
                onLiveMask?.({ mask_shape: val === 'none' ? null : val, mask_radius: mRadius }, true);
              }}
            >
              <option value="none">None</option>
              <option value="circle">Circle / Ellipse</option>
              <option value="rounded">Rounded Rect</option>
              <option value="star">Star (5-pt)</option>
            </select>
          </div>
          {#if mShape === 'rounded'}
            <ScrubNumber
              label="Mask Radius"
              bind:value={mRadius}
              min={0}
              max={400}
              onScrubStart={onCheckpoint}
              onLiveChange={(v) => onLiveMask?.({ mask_shape: 'rounded', mask_radius: v }, false)}
              onCommit={(v) => onLiveMask?.({ mask_shape: 'rounded', mask_radius: v }, false)}
            />
          {/if}
          <label class="checkbox-row">
            <input
              type="checkbox"
              checked={mClip}
              onchange={(e) => {
                mClip = e.currentTarget.checked;
                onLiveClipToBelow?.(mClip);
              }}
            />
            <span>Clip to Layer Below</span>
          </label>
          <div class="grid-2" style="margin-top:4px;">
            <button class="btn-primary sm" onclick={() => onAiRemoveBg?.(selectedLayer.id)}>
              AI Remove BG
            </button>
            <button class="btn-ghost sm" onclick={() => onResetAlphaMask?.(selectedLayer.id)} disabled={!selectedLayer.alpha_mask_path}>
              Reset Mask
            </button>
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
              {#if brandFonts.length > 0}
                <optgroup label="Brand">
                  {#each fontStyles.filter((fs) => brandFonts.includes(fs.id)) as fs}
                    <option value={fs.id}>{fs.label}</option>
                  {/each}
                </optgroup>
              {/if}
              <optgroup label="All Fonts">
                {#each fontStyles as fs}
                  <option value={fs.id}>{fs.label}</option>
                {/each}
              </optgroup>
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

            <label class="checkbox-row">
              <input
                type="checkbox"
                checked={tUnderline}
                onchange={(e) => {
                  tUnderline = e.currentTarget.checked;
                  onLiveTextAdvanced?.({ underline: tUnderline }, true);
                }}
              />
              <span>Underline</span>
            </label>

            <ScrubNumber
              label="Curve Arc °"
              bind:value={tCurve}
              min={-180}
              max={180}
              step={5}
              onScrubStart={onCheckpoint}
              onLiveChange={(v) => onLiveTextAdvanced?.({ curve_amount: v }, false)}
              onCommit={(v) => onLiveTextAdvanced?.({ curve_amount: v }, false)}
            />

            <label class="checkbox-row">
              <input
                type="checkbox"
                checked={tGradEnabled}
                onchange={(e) => {
                  tGradEnabled = e.currentTarget.checked;
                  if (tGradEnabled) {
                    onLiveTextAdvanced?.({ gradient_from_hex: tGradFrom, gradient_to_hex: tGradTo, gradient_angle: tGradAngle }, true);
                  } else {
                    onLiveTextAdvanced?.({ clear: true }, true);
                  }
                }}
              />
              <span>Gradient Fill</span>
            </label>
            {#if tGradEnabled}
              <div class="grid-2">
                <ColorPickerPopover
                  label="From"
                  bind:color={tGradFrom}
                  {brandColors}
                  {recentColors}
                  {onSaveBrandColor}
                  {onRemoveBrandColor}
                  onChange={(hex) => onLiveTextAdvanced?.({ gradient_from_hex: hex, gradient_to_hex: tGradTo, gradient_angle: tGradAngle }, true)}
                />
                <ColorPickerPopover
                  label="To"
                  bind:color={tGradTo}
                  {brandColors}
                  {recentColors}
                  {onSaveBrandColor}
                  {onRemoveBrandColor}
                  onChange={(hex) => onLiveTextAdvanced?.({ gradient_from_hex: tGradFrom, gradient_to_hex: hex, gradient_angle: tGradAngle }, true)}
                />
              </div>
              <ScrubNumber
                label="Grad Angle °"
                bind:value={tGradAngle}
                min={0}
                max={360}
                onScrubStart={onCheckpoint}
                onLiveChange={(v) => onLiveTextAdvanced?.({ gradient_from_hex: tGradFrom, gradient_to_hex: tGradTo, gradient_angle: v }, false)}
                onCommit={(v) => onLiveTextAdvanced?.({ gradient_from_hex: tGradFrom, gradient_to_hex: tGradTo, gradient_angle: v }, false)}
              />
            {/if}
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

            {#if sType === 'star' || sType === 'polygon'}
              <ScrubNumber
                label={sType === 'star' ? 'Points' : 'Sides'}
                bind:value={sSides}
                min={3}
                max={12}
                onScrubStart={onCheckpoint}
                onLiveChange={(v) => onLiveShape({ sides: v }, false)}
                onCommit={(v) => onLiveShape({ sides: v }, false)}
              />
            {/if}

            {#if sType === 'rectangle'}
              <div class="mini-lbl">Per-Corner Radii (TL / TR / BR / BL)</div>
              <div class="grid-2">
                <ScrubNumber label="TL" bind:value={sRadTL} min={0} max={250} onScrubStart={onCheckpoint} onLiveChange={(v) => onLiveShape({ corner_radii: [v, sRadTR, sRadBR, sRadBL] }, false)} onCommit={(v) => onLiveShape({ corner_radii: [v, sRadTR, sRadBR, sRadBL] }, false)} />
                <ScrubNumber label="TR" bind:value={sRadTR} min={0} max={250} onScrubStart={onCheckpoint} onLiveChange={(v) => onLiveShape({ corner_radii: [sRadTL, v, sRadBR, sRadBL] }, false)} onCommit={(v) => onLiveShape({ corner_radii: [sRadTL, v, sRadBR, sRadBL] }, false)} />
                <ScrubNumber label="BR" bind:value={sRadBR} min={0} max={250} onScrubStart={onCheckpoint} onLiveChange={(v) => onLiveShape({ corner_radii: [sRadTL, sRadTR, v, sRadBL] }, false)} onCommit={(v) => onLiveShape({ corner_radii: [sRadTL, sRadTR, v, sRadBL] }, false)} />
                <ScrubNumber label="BL" bind:value={sRadBL} min={0} max={250} onScrubStart={onCheckpoint} onLiveChange={(v) => onLiveShape({ corner_radii: [sRadTL, sRadTR, sRadBR, v] }, false)} onCommit={(v) => onLiveShape({ corner_radii: [sRadTL, sRadTR, sRadBR, v] }, false)} />
              </div>
            {/if}

            <label class="checkbox-row">
              <input
                type="checkbox"
                checked={sGradEnabled}
                onchange={(e) => {
                  sGradEnabled = e.currentTarget.checked;
                  if (sGradEnabled) {
                    onLiveShape({ fill_gradient_from: sGradFrom, fill_gradient_to: sGradTo, fill_gradient_angle: sGradAngle }, true);
                  } else {
                    onLiveShape({ clear_fill_gradient: true }, true);
                  }
                }}
              />
              <span>Gradient Fill</span>
            </label>
            {#if sGradEnabled}
              <div class="grid-2">
                <ColorPickerPopover label="From" bind:color={sGradFrom} {brandColors} {recentColors} {onSaveBrandColor} {onRemoveBrandColor} onChange={(hex) => onLiveShape({ fill_gradient_from: hex, fill_gradient_to: sGradTo, fill_gradient_angle: sGradAngle }, true)} />
                <ColorPickerPopover label="To" bind:color={sGradTo} {brandColors} {recentColors} {onSaveBrandColor} {onRemoveBrandColor} onChange={(hex) => onLiveShape({ fill_gradient_from: sGradFrom, fill_gradient_to: hex, fill_gradient_angle: sGradAngle }, true)} />
              </div>
              <ScrubNumber label="Grad Angle °" bind:value={sGradAngle} min={0} max={360} onScrubStart={onCheckpoint} onLiveChange={(v) => onLiveShape({ fill_gradient_from: sGradFrom, fill_gradient_to: sGradTo, fill_gradient_angle: v }, false)} onCommit={(v) => onLiveShape({ fill_gradient_from: sGradFrom, fill_gradient_to: sGradTo, fill_gradient_angle: v }, false)} />
            {/if}
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

      <!-- ══ DROP SHADOW ══ -->
      <div class="sec">
        <button class="sec-head" onclick={() => (secShadow = !secShadow)}>
          {#if secShadow}<ChevronDown size={13} />{:else}<ChevronRight size={13} />{/if}
          <span>Drop Shadow</span>
        </button>
        {#if secShadow}
          <label class="checkbox-row">
            <input
              type="checkbox"
              checked={shEnabled}
              onchange={(e) => {
                shEnabled = e.currentTarget.checked;
                onLiveShadow?.({ enabled: shEnabled }, true);
              }}
            />
            <span>Enable Drop Shadow</span>
          </label>
          {#if shEnabled}
            <div class="grid-2">
              <ColorPickerPopover label="Shadow" bind:color={shColor} {brandColors} {recentColors} {onSaveBrandColor} {onRemoveBrandColor} onChange={(hex) => onLiveShadow?.({ enabled: true, color_hex: hex }, true)} />
              <ScrubNumber label="Opacity %" bind:value={shOpacity} min={0} max={100} onScrubStart={onCheckpoint} onLiveChange={(v) => onLiveShadow?.({ enabled: true, opacity: v }, false)} onCommit={(v) => onLiveShadow?.({ enabled: true, opacity: v }, false)} />
            </div>
            <div class="grid-2">
              <ScrubNumber label="Distance" bind:value={shDist} min={0} max={100} onScrubStart={onCheckpoint} onLiveChange={(v) => onLiveShadow?.({ enabled: true, distance: v }, false)} onCommit={(v) => onLiveShadow?.({ enabled: true, distance: v }, false)} />
              <ScrubNumber label="Angle °" bind:value={shAngle} min={0} max={360} onScrubStart={onCheckpoint} onLiveChange={(v) => onLiveShadow?.({ enabled: true, angle: v }, false)} onCommit={(v) => onLiveShadow?.({ enabled: true, angle: v }, false)} />
            </div>
            <ScrubNumber label="Blur" bind:value={shBlur} min={0} max={40} onScrubStart={onCheckpoint} onLiveChange={(v) => onLiveShadow?.({ enabled: true, blur: v }, false)} onCommit={(v) => onLiveShadow?.({ enabled: true, blur: v }, false)} />
          {/if}
        {/if}
      </div>

      <!-- ══ OUTER & INNER GLOW ══ -->
      <div class="sec">
        <button class="sec-head" onclick={() => (secGlow = !secGlow)}>
          {#if secGlow}<ChevronDown size={13} />{:else}<ChevronRight size={13} />{/if}
          <span>Glow Effects</span>
        </button>
        {#if secGlow}
          <label class="checkbox-row">
            <input
              type="checkbox"
              checked={ogEnabled}
              onchange={(e) => {
                ogEnabled = e.currentTarget.checked;
                onLiveGlow?.({ kind: 'outer', enabled: ogEnabled }, true);
              }}
            />
            <span>Outer Glow</span>
          </label>
          {#if ogEnabled}
            <div class="grid-2">
              <ColorPickerPopover label="Color" bind:color={ogColor} {brandColors} {recentColors} {onSaveBrandColor} {onRemoveBrandColor} onChange={(hex) => onLiveGlow?.({ kind: 'outer', enabled: true, color_hex: hex }, true)} />
              <ScrubNumber label="Intensity" bind:value={ogIntensity} min={1} max={40} onScrubStart={onCheckpoint} onLiveChange={(v) => onLiveGlow?.({ kind: 'outer', enabled: true, intensity: v }, false)} onCommit={(v) => onLiveGlow?.({ kind: 'outer', enabled: true, intensity: v }, false)} />
            </div>
          {/if}

          <label class="checkbox-row">
            <input
              type="checkbox"
              checked={igEnabled}
              onchange={(e) => {
                igEnabled = e.currentTarget.checked;
                onLiveGlow?.({ kind: 'inner', enabled: igEnabled }, true);
              }}
            />
            <span>Inner Glow</span>
          </label>
          {#if igEnabled}
            <div class="grid-2">
              <ColorPickerPopover label="Color" bind:color={igColor} {brandColors} {recentColors} {onSaveBrandColor} {onRemoveBrandColor} onChange={(hex) => onLiveGlow?.({ kind: 'inner', enabled: true, color_hex: hex }, true)} />
              <ScrubNumber label="Intensity" bind:value={igIntensity} min={1} max={40} onScrubStart={onCheckpoint} onLiveChange={(v) => onLiveGlow?.({ kind: 'inner', enabled: true, intensity: v }, false)} onCommit={(v) => onLiveGlow?.({ kind: 'inner', enabled: true, intensity: v }, false)} />
            </div>
          {/if}
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
          <div class="slider-row">
            <div class="slider-label"><span>Temperature</span><span class="mono">{fxTemp}</span></div>
            <input type="range" min="-100" max="100" step="1" bind:value={fxTemp} onpointerdown={onCheckpoint} oninput={() => onLiveEffects({ temperature: fxTemp }, false)} />
          </div>
          <div class="slider-row">
            <div class="slider-label"><span>Hue Shift °</span><span class="mono">{fxHue}°</span></div>
            <input type="range" min="-180" max="180" step="1" bind:value={fxHue} onpointerdown={onCheckpoint} oninput={() => onLiveEffects({ hue_shift: fxHue }, false)} />
          </div>
          <div class="slider-row">
            <div class="slider-label"><span>Sharpness</span><span class="mono">{fxSharp}</span></div>
            <input type="range" min="0" max="100" step="1" bind:value={fxSharp} onpointerdown={onCheckpoint} oninput={() => onLiveEffects({ sharpness: fxSharp }, false)} />
          </div>
          <div class="slider-row">
            <div class="slider-label"><span>Vignette</span><span class="mono">{fxVignette}%</span></div>
            <input type="range" min="0" max="100" step="1" bind:value={fxVignette} onpointerdown={onCheckpoint} oninput={() => onLiveEffects({ vignette: fxVignette }, false)} />
          </div>
          <div class="mini-lbl">Levels (Black / Gamma / White)</div>
          <div class="grid-3" style="display:grid; grid-template-columns:repeat(3,1fr); gap:6px;">
            <ScrubNumber label="Blk" bind:value={fxLvlBlack} min={0} max={240} onScrubStart={onCheckpoint} onLiveChange={(v) => onLiveEffects({ levels_black: v }, false)} onCommit={(v) => onLiveEffects({ levels_black: v }, false)} />
            <ScrubNumber label="Gam" bind:value={fxLvlGamma} min={0.2} max={3.0} step={0.05} onScrubStart={onCheckpoint} onLiveChange={(v) => onLiveEffects({ levels_gamma: v }, false)} onCommit={(v) => onLiveEffects({ levels_gamma: v }, false)} />
            <ScrubNumber label="Wht" bind:value={fxLvlWhite} min={15} max={255} onScrubStart={onCheckpoint} onLiveChange={(v) => onLiveEffects({ levels_white: v }, false)} onCommit={(v) => onLiveEffects({ levels_white: v }, false)} />
          </div>
          <div class="field-col">
            <span class="mini-lbl">Color Grading LUT</span>
            <select
              class="select-input"
              value={fxLut}
              onchange={(e) => {
                fxLut = e.currentTarget.value;
                onLiveEffects({ lut_preset: fxLut }, true);
              }}
            >
              <option value="none">None</option>
              <option value="cinematic">Cinematic (Teal &amp; Orange)</option>
              <option value="cyberpunk">Cyberpunk (Neon)</option>
              <option value="vintage">Vintage Film</option>
              <option value="moody">Moody Matte</option>
              <option value="pastel">Pastel Lift</option>
            </select>
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
