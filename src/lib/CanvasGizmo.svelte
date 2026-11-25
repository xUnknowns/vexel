<script>
  let {
    layers = [],
    selectedLayerId = null,
    canvasW = 1280,
    canvasH = 720,
    zoom = 1.0,
    cropMode = false,
    cropRect = { x: 0, y: 0, width: 640, height: 360 },
    cropAspect = null, // null = free, or number e.g. 16/9
    onSelectLayer,
    onDoubleClickText,
    onCommitTransform,
    onDeleteLayer,
    onDuplicateLayer,
    onCropRectChange,
  } = $props();

  let selectedLayer = $derived(layers.find((l) => l.id === selectedLayerId) ?? null);

  // Live 60 FPS transform state while dragging/resizing/rotating
  let mode = $state(null); // 'move' | 'resize' | 'rotate' | 'crop-move' | 'crop-resize' | null
  let liveX = $state(0);
  let liveY = $state(0);
  let liveScale = $state(1.0);
  let liveRot = $state(0);
  let snapX = $state(false);
  let snapY = $state(false);

  let startPtr = { x: 0, y: 0 };
  let startVal = { x: 0, y: 0, scale: 1, rot: 0, w: 100, h: 100 };
  let gizmoEl;

  $effect(() => {
    if (selectedLayer && !mode) {
      liveX = selectedLayer.x;
      liveY = selectedLayer.y;
      liveScale = selectedLayer.scale;
      liveRot = selectedLayer.rotation;
    }
  });

  function layerBox(l, useLive = false) {
    const sc = useLive ? liveScale : l.scale;
    const bw = Math.max(24, (l.bounds_width || 120) * sc);
    const bh = Math.max(24, (l.bounds_height || 120) * sc);
    return {
      x: useLive ? liveX : l.x,
      y: useLive ? liveY : l.y,
      w: bw,
      h: bh,
      rot: useLive ? liveRot : l.rotation,
    };
  }

  function startMove(e, layer) {
    if (cropMode) return;
    e.stopPropagation();
    onSelectLayer(layer.id);
    if (layer.locked) return;
    mode = 'move';
    liveX = layer.x;
    liveY = layer.y;
    liveScale = layer.scale;
    liveRot = layer.rotation;
    startPtr = { x: e.clientX, y: e.clientY };
    startVal = { x: layer.x, y: layer.y, scale: layer.scale, rot: layer.rotation, w: layer.bounds_width || 120, h: layer.bounds_height || 120 };
    e.currentTarget.setPointerCapture?.(e.pointerId);
  }

  function startResize(e) {
    if (!selectedLayer || selectedLayer.locked || cropMode) return;
    e.stopPropagation();
    mode = 'resize';
    startPtr = { x: e.clientX, y: e.clientY };
    startVal = {
      x: liveX,
      y: liveY,
      scale: liveScale,
      rot: liveRot,
      w: selectedLayer.bounds_width || 120,
      h: selectedLayer.bounds_height || 120,
    };
    e.currentTarget.setPointerCapture?.(e.pointerId);
  }

  function startRotate(e) {
    if (!selectedLayer || selectedLayer.locked || cropMode) return;
    e.stopPropagation();
    mode = 'rotate';
    startPtr = { x: e.clientX, y: e.clientY };
    startVal = { x: liveX, y: liveY, scale: liveScale, rot: liveRot, w: selectedLayer.bounds_width || 120, h: selectedLayer.bounds_height || 120 };
    e.currentTarget.setPointerCapture?.(e.pointerId);
  }

  function startCropMove(e) {
    e.stopPropagation();
    mode = 'crop-move';
    startPtr = { x: e.clientX, y: e.clientY };
    startVal = { x: cropRect.x, y: cropRect.y, w: cropRect.width, h: cropRect.height, scale: 1, rot: 0 };
    e.currentTarget.setPointerCapture?.(e.pointerId);
  }

  function startCropResize(e) {
    e.stopPropagation();
    mode = 'crop-resize';
    startPtr = { x: e.clientX, y: e.clientY };
    startVal = { x: cropRect.x, y: cropRect.y, w: cropRect.width, h: cropRect.height, scale: 1, rot: 0 };
    e.currentTarget.setPointerCapture?.(e.pointerId);
  }

  function onPointerMove(e) {
    if (!mode) return;
    const dx = (e.clientX - startPtr.x) / zoom;
    const dy = (e.clientY - startPtr.y) / zoom;

    if (mode === 'move' && selectedLayer) {
      let nx = Math.round(startVal.x + dx);
      let ny = Math.round(startVal.y + dy);
      const bw = startVal.w * liveScale;
      const bh = startVal.h * liveScale;
      const cx = nx + bw / 2;
      const cy = ny + bh / 2;

      if (Math.abs(cx - canvasW / 2) <= 5) {
        nx = Math.round(canvasW / 2 - bw / 2);
        snapX = true;
      } else {
        snapX = false;
      }
      if (Math.abs(cy - canvasH / 2) <= 5) {
        ny = Math.round(canvasH / 2 - bh / 2);
        snapY = true;
      } else {
        snapY = false;
      }
      liveX = nx;
      liveY = ny;
    } else if (mode === 'resize' && selectedLayer) {
      const delta = Math.max(dx, dy);
      const baseDiag = Math.max(40, startVal.w);
      liveScale = Number(Math.max(0.1, Math.min(6.0, startVal.scale + delta / baseDiag)).toFixed(2));
    } else if (mode === 'rotate' && selectedLayer && gizmoEl) {
      const rect = gizmoEl.getBoundingClientRect();
      const boxCx = rect.left + (liveX + (startVal.w * liveScale) / 2) * zoom;
      const boxCy = rect.top + (liveY + (startVal.h * liveScale) / 2) * zoom;
      const angle = Math.atan2(e.clientY - boxCy, e.clientX - boxCx) * (180 / Math.PI) + 90;
      liveRot = Math.round(angle);
    } else if (mode === 'crop-move') {
      const nx = Math.max(0, Math.min(canvasW - startVal.w, Math.round(startVal.x + dx)));
      const ny = Math.max(0, Math.min(canvasH - startVal.h, Math.round(startVal.y + dy)));
      onCropRectChange({ ...cropRect, x: nx, y: ny });
    } else if (mode === 'crop-resize') {
      let nw = Math.max(20, Math.min(canvasW - cropRect.x, Math.round(startVal.w + dx)));
      let nh = cropAspect
        ? Math.max(20, Math.round(nw / cropAspect))
        : Math.max(20, Math.min(canvasH - cropRect.y, Math.round(startVal.h + dy)));
      if (cropRect.y + nh > canvasH) {
        nh = canvasH - cropRect.y;
        if (cropAspect) nw = Math.round(nh * cropAspect);
      }
      onCropRectChange({ ...cropRect, width: nw, height: nh });
    }
  }

  function onPointerUp() {
    if (!mode) return;
    const prevMode = mode;
    mode = null;
    snapX = false;
    snapY = false;
    if ((prevMode === 'move' || prevMode === 'resize' || prevMode === 'rotate') && selectedLayer) {
      onCommitTransform(selectedLayer.id, liveX, liveY, liveScale, liveRot);
    }
  }

  function onWindowKeydown(e) {
    const tag = e.target?.tagName;
    if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return;
    if (!selectedLayer || cropMode) return;

    if (e.key === 'Delete' || e.key === 'Backspace') {
      e.preventDefault();
      onDeleteLayer(selectedLayer.id);
    } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'd') {
      e.preventDefault();
      onDuplicateLayer(selectedLayer.id);
    } else if (['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'].includes(e.key) && !selectedLayer.locked) {
      e.preventDefault();
      const step = e.shiftKey ? 10 : 1;
      const nx = selectedLayer.x + (e.key === 'ArrowRight' ? step : e.key === 'ArrowLeft' ? -step : 0);
      const ny = selectedLayer.y + (e.key === 'ArrowDown' ? step : e.key === 'ArrowUp' ? -step : 0);
      onCommitTransform(selectedLayer.id, nx, ny, selectedLayer.scale, selectedLayer.rotation);
    }
  }
</script>

<svelte:window onkeydown={onWindowKeydown} onpointermove={onPointerMove} onpointerup={onPointerUp} />

<div
  class="gizmo-overlay"
  bind:this={gizmoEl}
  onpointerdown={(e) => {
    if (!cropMode && e.target === e.currentTarget) onSelectLayer(null);
  }}
  role="presentation"
>
  {#if !cropMode}
    <!-- Magnetic center snap guides -->
    {#if snapX}
      <div class="snap-line vertical"></div>
    {/if}
    {#if snapY}
      <div class="snap-line horizontal"></div>
    {/if}

    <!-- Layer hitboxes (non-selected) -->
    {#each layers as layer}
      {#if layer.visible && layer.id !== selectedLayerId}
        {@const box = layerBox(layer, false)}
        <div
          class="layer-hitbox"
          style="
            left:{box.x * zoom}px;
            top:{box.y * zoom}px;
            width:{box.w * zoom}px;
            height:{box.h * zoom}px;
            transform:rotate({box.rot}deg);
          "
          onpointerdown={(e) => startMove(e, layer)}
          ondblclick={(e) => {
            e.stopPropagation();
            if (layer.kind?.type === 'Text3D') onDoubleClickText(layer);
          }}
          role="presentation"
        ></div>
      {/if}
    {/each}

    <!-- Selected Layer Bounding Box -->
    {#if selectedLayer && selectedLayer.visible}
      {@const box = layerBox(selectedLayer, true)}
      <div
        class="selection-box"
        class:locked={selectedLayer.locked}
        style="
          left:{box.x * zoom}px;
          top:{box.y * zoom}px;
          width:{box.w * zoom}px;
          height:{box.h * zoom}px;
          transform:rotate({box.rot}deg);
        "
        onpointerdown={(e) => startMove(e, selectedLayer)}
        ondblclick={(e) => {
          e.stopPropagation();
          if (selectedLayer.kind?.type === 'Text3D') onDoubleClickText(selectedLayer);
        }}
        role="presentation"
      >
        {#if !selectedLayer.locked}
          <div class="rot-stem"></div>
          <div class="rot-handle" onpointerdown={startRotate} title="Drag to rotate" role="presentation"></div>
          <div class="corner nw" onpointerdown={startResize} role="presentation"></div>
          <div class="corner ne" onpointerdown={startResize} role="presentation"></div>
          <div class="corner sw" onpointerdown={startResize} role="presentation"></div>
          <div class="corner se" onpointerdown={startResize} role="presentation"></div>
        {/if}
      </div>
    {/if}
  {:else}
    <!-- Visual Crop Overlay -->
    <div
      class="crop-box"
      style="
        left:{cropRect.x * zoom}px;
        top:{cropRect.y * zoom}px;
        width:{cropRect.width * zoom}px;
        height:{cropRect.height * zoom}px;
      "
      onpointerdown={startCropMove}
      role="presentation"
    >
      <div class="crop-grid"></div>
      <div class="crop-dims">{cropRect.width} × {cropRect.height}</div>
      <div class="corner se" onpointerdown={startCropResize} role="presentation"></div>
    </div>
  {/if}
</div>

<style>
  .gizmo-overlay {
    position: absolute;
    inset: 0;
    pointer-events: auto;
    overflow: hidden;
  }

  .snap-line {
    position: absolute;
    background: #38bdf8;
    box-shadow: 0 0 8px rgba(56, 189, 248, 0.9);
    z-index: 30;
  }
  .snap-line.vertical {
    left: 50%;
    top: 0;
    bottom: 0;
    width: 1px;
  }
  .snap-line.horizontal {
    top: 50%;
    left: 0;
    right: 0;
    height: 1px;
  }

  .layer-hitbox {
    position: absolute;
    pointer-events: auto;
    cursor: pointer;
    transform-origin: center center;
  }
  .layer-hitbox:hover {
    outline: 1px dashed rgba(56, 189, 248, 0.45);
  }

  .selection-box {
    position: absolute;
    pointer-events: auto;
    border: 1.5px solid #38bdf8;
    box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.45);
    cursor: grab;
    transform-origin: center center;
    z-index: 20;
  }
  .selection-box:active { cursor: grabbing; }
  .selection-box.locked { border-style: dashed; border-color: #f59e0b; cursor: not-allowed; }

  .corner {
    position: absolute;
    width: 10px;
    height: 10px;
    background: #ffffff;
    border: 2px solid #38bdf8;
    border-radius: 2px;
    z-index: 25;
  }
  .corner.nw { top: -5px; left: -5px; cursor: nwse-resize; }
  .corner.ne { top: -5px; right: -5px; cursor: nesw-resize; }
  .corner.sw { bottom: -5px; left: -5px; cursor: nesw-resize; }
  .corner.se { bottom: -5px; right: -5px; cursor: nwse-resize; }

  .rot-stem {
    position: absolute;
    top: -22px;
    left: 50%;
    width: 1.5px;
    height: 22px;
    background: #38bdf8;
    transform: translateX(-50%);
  }
  .rot-handle {
    position: absolute;
    top: -30px;
    left: 50%;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: #ffffff;
    border: 2px solid #38bdf8;
    transform: translateX(-50%);
    cursor: grab;
  }

  .crop-box {
    position: absolute;
    pointer-events: auto;
    border: 2px solid #38bdf8;
    box-shadow: 0 0 0 9999px rgba(0, 0, 0, 0.58);
    cursor: move;
    z-index: 40;
  }
  .crop-grid {
    position: absolute;
    inset: 0;
    background-image:
      linear-gradient(to right, rgba(255, 255, 255, 0.2) 1px, transparent 1px),
      linear-gradient(to bottom, rgba(255, 255, 255, 0.2) 1px, transparent 1px);
    background-size: 33.33% 33.33%;
    pointer-events: none;
  }
  .crop-dims {
    position: absolute;
    top: 6px;
    left: 6px;
    background: rgba(13, 15, 18, 0.85);
    color: #38bdf8;
    font-size: 11px;
    font-weight: 600;
    padding: 2px 7px;
    border-radius: 4px;
    pointer-events: none;
  }
</style>
