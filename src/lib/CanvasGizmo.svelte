<script>
  let {
    layers = [],
    selectedLayerIds = [],
    canvasW = 1280,
    canvasH = 720,
    zoom = 1.0,
    cropMode = false,
    cropRect = { x: 0, y: 0, width: 640, height: 360 },
    cropAspect = null,
    showGrid = false,
    gridSize = 40,
    guides = [],
    onSelectLayers,
    onDoubleClickText,
    onCommitTransform,
    onCommitMultiTransform,
    onDeleteLayers,
    onDuplicateLayers,
    onGroupSelected,
    onUngroupSelected,
    onRemoveGuide,
    onUpdateGuide,
    onCropRectChange,
  } = $props();

  let selectedLayers = $derived(layers.filter((l) => selectedLayerIds.includes(l.id)));
  let primaryLayer = $derived(selectedLayers.length === 1 ? selectedLayers[0] : null);

  let mode = $state(null); // 'move' | 'resize' | 'rotate' | 'marquee' | 'guide' | 'crop-move' | 'crop-resize' | null
  let liveMap = $state({}); // { [id]: { x, y, scale, rotation } }
  let marquee = $state({ x1: 0, y1: 0, x2: 0, y2: 0 });
  let snapGuides = $state({ v: null, h: null, distLabel: null });
  let dragGuideId = null;

  let startPtr = { x: 0, y: 0 };
  let startMulti = {}; // { [id]: { x, y, scale, rotation, w, h } }
  let startGroupBox = { x: 0, y: 0, w: 100, h: 100 };
  let startCrop = { x: 0, y: 0, w: 100, h: 100 };
  let gizmoEl;

  function layerBox(l, useLive = false) {
    const lv = useLive ? liveMap[l.id] : null;
    const sc = lv ? lv.scale : l.scale;
    const bw = Math.max(24, (l.bounds_width || 120) * sc);
    const bh = Math.max(24, (l.bounds_height || 120) * sc);
    return {
      x: lv ? lv.x : l.x,
      y: lv ? lv.y : l.y,
      w: bw,
      h: bh,
      rot: lv ? lv.rotation : l.rotation,
    };
  }

  let selectionBounds = $derived.by(() => {
    const vis = selectedLayers.filter((l) => l.visible);
    if (vis.length === 0) return null;
    let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
    const useLive = mode === 'move' || mode === 'resize' || mode === 'rotate';
    for (const l of vis) {
      const b = layerBox(l, useLive);
      minX = Math.min(minX, b.x);
      minY = Math.min(minY, b.y);
      maxX = Math.max(maxX, b.x + b.w);
      maxY = Math.max(maxY, b.y + b.h);
    }
    return { x: minX, y: minY, w: maxX - minX, h: maxY - minY, rot: vis.length === 1 ? layerBox(vis[0], useLive).rot : 0 };
  });

  function snapshotSelection(targetIds) {
    const m = {};
    const live = {};
    for (const l of layers) {
      if (targetIds.includes(l.id)) {
        m[l.id] = { x: l.x, y: l.y, scale: l.scale, rotation: l.rotation, w: l.bounds_width || 120, h: l.bounds_height || 120 };
        live[l.id] = { x: l.x, y: l.y, scale: l.scale, rotation: l.rotation };
      }
    }
    startMulti = m;
    liveMap = live;
  }

  function startMove(e, layer) {
    if (cropMode) return;
    e.stopPropagation();

    let nextIds = selectedLayerIds;
    if (e.shiftKey) {
      nextIds = selectedLayerIds.includes(layer.id)
        ? selectedLayerIds.filter((id) => id !== layer.id)
        : [...selectedLayerIds, layer.id];
      onSelectLayers(nextIds);
      if (!nextIds.includes(layer.id)) return;
    } else if (!selectedLayerIds.includes(layer.id)) {
      nextIds = [layer.id];
      onSelectLayers(nextIds);
    }

    const movable = layers.filter((l) => nextIds.includes(l.id) && !l.locked);
    if (movable.length === 0) return;

    mode = 'move';
    startPtr = { x: e.clientX, y: e.clientY };
    snapshotSelection(movable.map((l) => l.id));

    let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
    for (const l of movable) {
      const b = layerBox(l, false);
      minX = Math.min(minX, b.x);
      minY = Math.min(minY, b.y);
      maxX = Math.max(maxX, b.x + b.w);
      maxY = Math.max(maxY, b.y + b.h);
    }
    startGroupBox = { x: minX, y: minY, w: maxX - minX, h: maxY - minY };
    e.currentTarget.setPointerCapture?.(e.pointerId);
  }

  function startResize(e) {
    if (cropMode || selectedLayers.every((l) => l.locked)) return;
    e.stopPropagation();
    mode = 'resize';
    startPtr = { x: e.clientX, y: e.clientY };
    const movableIds = selectedLayers.filter((l) => !l.locked).map((l) => l.id);
    snapshotSelection(movableIds);
    if (selectionBounds) startGroupBox = { ...selectionBounds };
    e.currentTarget.setPointerCapture?.(e.pointerId);
  }

  function startRotate(e) {
    if (cropMode || selectedLayers.every((l) => l.locked)) return;
    e.stopPropagation();
    mode = 'rotate';
    startPtr = { x: e.clientX, y: e.clientY };
    const movableIds = selectedLayers.filter((l) => !l.locked).map((l) => l.id);
    snapshotSelection(movableIds);
    if (selectionBounds) startGroupBox = { ...selectionBounds };
    e.currentTarget.setPointerCapture?.(e.pointerId);
  }

  function startMarquee(e) {
    if (cropMode || e.target !== e.currentTarget || !gizmoEl) return;
    const rect = gizmoEl.getBoundingClientRect();
    const cx = (e.clientX - rect.left) / zoom;
    const cy = (e.clientY - rect.top) / zoom;
    mode = 'marquee';
    marquee = { x1: cx, y1: cy, x2: cx, y2: cy };
    if (!e.shiftKey) onSelectLayers([]);
    e.currentTarget.setPointerCapture?.(e.pointerId);
  }

  function startGuideDrag(e, guide) {
    e.stopPropagation();
    mode = 'guide';
    dragGuideId = guide.id;
    e.currentTarget.setPointerCapture?.(e.pointerId);
  }

  function startCropMove(e) {
    e.stopPropagation();
    mode = 'crop-move';
    startPtr = { x: e.clientX, y: e.clientY };
    startCrop = { x: cropRect.x, y: cropRect.y, w: cropRect.width, h: cropRect.height };
    e.currentTarget.setPointerCapture?.(e.pointerId);
  }

  function startCropResize(e) {
    e.stopPropagation();
    mode = 'crop-resize';
    startPtr = { x: e.clientX, y: e.clientY };
    startCrop = { x: cropRect.x, y: cropRect.y, w: cropRect.width, h: cropRect.height };
    e.currentTarget.setPointerCapture?.(e.pointerId);
  }

  function computeSmartSnap(nx, ny, bw, bh) {
    const tol = 5;
    let snapV = null;
    let snapH = null;
    let distLabel = null;

    // Candidate X anchors: canvas center, user v-guides, other layers' left/center/right
    const vTargets = [{ pos: canvasW / 2, ref: null }];
    for (const g of guides) if (g.axis === 'v') vTargets.push({ pos: g.pos, ref: null });
    const hTargets = [{ pos: canvasH / 2, ref: null }];
    for (const g of guides) if (g.axis === 'h') hTargets.push({ pos: g.pos, ref: null });

    for (const other of layers) {
      if (!other.visible || selectedLayerIds.includes(other.id)) continue;
      const ob = layerBox(other, false);
      vTargets.push({ pos: ob.x, ref: ob }, { pos: ob.x + ob.w / 2, ref: ob }, { pos: ob.x + ob.w, ref: ob });
      hTargets.push({ pos: ob.y, ref: ob }, { pos: ob.y + ob.h / 2, ref: ob }, { pos: ob.y + ob.h, ref: ob });
    }

    // Check left, center, right of dragged box
    const xEdges = [
      { val: nx, offset: 0 },
      { val: nx + bw / 2, offset: bw / 2 },
      { val: nx + bw, offset: bw },
    ];
    for (const t of vTargets) {
      for (const edge of xEdges) {
        if (Math.abs(edge.val - t.pos) <= tol) {
          nx = Math.round(t.pos - edge.offset);
          snapV = t.pos;
          if (t.ref) {
            const dy = Math.round(Math.max(0, t.ref.y - (ny + bh), ny - (t.ref.y + t.ref.h)));
            if (dy > 0) distLabel = { x: t.pos + 6, y: Math.min(ny, t.ref.y) + dy / 2, text: `${dy}px` };
          }
          break;
        }
      }
      if (snapV !== null) break;
    }

    const yEdges = [
      { val: ny, offset: 0 },
      { val: ny + bh / 2, offset: bh / 2 },
      { val: ny + bh, offset: bh },
    ];
    for (const t of hTargets) {
      for (const edge of yEdges) {
        if (Math.abs(edge.val - t.pos) <= tol) {
          ny = Math.round(t.pos - edge.offset);
          snapH = t.pos;
          if (t.ref && !distLabel) {
            const dx = Math.round(Math.max(0, t.ref.x - (nx + bw), nx - (t.ref.x + t.ref.w)));
            if (dx > 0) distLabel = { x: Math.min(nx, t.ref.x) + dx / 2, y: t.pos - 16, text: `${dx}px` };
          }
          break;
        }
      }
      if (snapH !== null) break;
    }

    if (showGrid && snapV === null && snapH === null) {
      const gx = Math.round(nx / gridSize) * gridSize;
      const gy = Math.round(ny / gridSize) * gridSize;
      if (Math.abs(nx - gx) <= 4) nx = gx;
      if (Math.abs(ny - gy) <= 4) ny = gy;
    }

    snapGuides = { v: snapV, h: snapH, distLabel };
    return { nx, ny };
  }

  function onPointerMove(e) {
    if (!mode) return;
    const dx = (e.clientX - startPtr.x) / zoom;
    const dy = (e.clientY - startPtr.y) / zoom;

    if (mode === 'move') {
      const rawX = Math.round(startGroupBox.x + dx);
      const rawY = Math.round(startGroupBox.y + dy);
      const { nx, ny } = computeSmartSnap(rawX, rawY, startGroupBox.w, startGroupBox.h);
      const adx = nx - startGroupBox.x;
      const ady = ny - startGroupBox.y;
      const next = {};
      for (const [id, st] of Object.entries(startMulti)) {
        next[id] = { ...st, x: st.x + adx, y: st.y + ady };
      }
      liveMap = next;
    } else if (mode === 'resize') {
      const delta = Math.max(dx, dy);
      const ratio = Math.max(0.1, Math.min(6.0, (startGroupBox.w + delta) / Math.max(24, startGroupBox.w)));
      const next = {};
      for (const [id, st] of Object.entries(startMulti)) {
        const relX = st.x - startGroupBox.x;
        const relY = st.y - startGroupBox.y;
        next[id] = {
          ...st,
          x: Math.round(startGroupBox.x + relX * ratio),
          y: Math.round(startGroupBox.y + relY * ratio),
          scale: Number(Math.max(0.05, Math.min(8.0, st.scale * ratio)).toFixed(2)),
        };
      }
      liveMap = next;
    } else if (mode === 'rotate' && gizmoEl) {
      const rect = gizmoEl.getBoundingClientRect();
      const boxCx = rect.left + (startGroupBox.x + startGroupBox.w / 2) * zoom;
      const boxCy = rect.top + (startGroupBox.y + startGroupBox.h / 2) * zoom;
      const angle = Math.round(Math.atan2(e.clientY - boxCy, e.clientX - boxCx) * (180 / Math.PI) + 90);
      const next = {};
      const ids = Object.keys(startMulti);
      const baseRot = ids.length === 1 ? startMulti[ids[0]].rotation : 0;
      const deltaRot = angle - baseRot;
      for (const [id, st] of Object.entries(startMulti)) {
        next[id] = { ...st, rotation: ids.length === 1 ? angle : Math.round(st.rotation + deltaRot) };
      }
      liveMap = next;
    } else if (mode === 'marquee' && gizmoEl) {
      const rect = gizmoEl.getBoundingClientRect();
      marquee = {
        ...marquee,
        x2: (e.clientX - rect.left) / zoom,
        y2: (e.clientY - rect.top) / zoom,
      };
    } else if (mode === 'guide' && gizmoEl && dragGuideId) {
      const rect = gizmoEl.getBoundingClientRect();
      const guide = guides.find((g) => g.id === dragGuideId);
      if (guide) {
        const pos = guide.axis === 'v' ? Math.round((e.clientX - rect.left) / zoom) : Math.round((e.clientY - rect.top) / zoom);
        onUpdateGuide?.(dragGuideId, pos);
      }
    } else if (mode === 'crop-move') {
      const nx = Math.max(0, Math.min(canvasW - startCrop.w, Math.round(startCrop.x + dx)));
      const ny = Math.max(0, Math.min(canvasH - startCrop.h, Math.round(startCrop.y + dy)));
      onCropRectChange({ ...cropRect, x: nx, y: ny });
    } else if (mode === 'crop-resize') {
      let nw = Math.max(20, Math.min(canvasW - cropRect.x, Math.round(startCrop.w + dx)));
      let nh = cropAspect
        ? Math.max(20, Math.round(nw / cropAspect))
        : Math.max(20, Math.min(canvasH - cropRect.y, Math.round(startCrop.h + dy)));
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
    snapGuides = { v: null, h: null, distLabel: null };
    dragGuideId = null;

    if (prevMode === 'marquee') {
      const mx1 = Math.min(marquee.x1, marquee.x2);
      const my1 = Math.min(marquee.y1, marquee.y2);
      const mx2 = Math.max(marquee.x1, marquee.x2);
      const my2 = Math.max(marquee.y1, marquee.y2);
      if (mx2 - mx1 > 4 || my2 - my1 > 4) {
        const hits = layers
          .filter((l) => {
            if (!l.visible) return false;
            const b = layerBox(l, false);
            return b.x < mx2 && b.x + b.w > mx1 && b.y < my2 && b.y + b.h > my1;
          })
          .map((l) => l.id);
        onSelectLayers(hits);
      }
      return;
    }

    if (prevMode === 'move' || prevMode === 'resize' || prevMode === 'rotate') {
      const entries = Object.entries(liveMap);
      if (entries.length === 1) {
        const [id, v] = entries[0];
        onCommitTransform(id, v.x, v.y, v.scale, v.rotation);
      } else if (entries.length > 1) {
        onCommitMultiTransform(
          entries.map(([layer_id, v]) => ({ layer_id, x: v.x, y: v.y, scale: v.scale, rotation: v.rotation }))
        );
      }
      liveMap = {};
    }
  }

  function onWindowKeydown(e) {
    const tag = e.target?.tagName;
    if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return;
    if (selectedLayerIds.length === 0 || cropMode) return;

    if (e.key === 'Delete' || e.key === 'Backspace') {
      e.preventDefault();
      onDeleteLayers(selectedLayerIds);
    } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'd') {
      e.preventDefault();
      onDuplicateLayers(selectedLayerIds);
    } else if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === 'g') {
      e.preventDefault();
      onUngroupSelected?.();
    } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'g') {
      e.preventDefault();
      onGroupSelected?.();
    } else if (['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'].includes(e.key)) {
      e.preventDefault();
      const step = e.shiftKey ? 10 : 1;
      const dx = e.key === 'ArrowRight' ? step : e.key === 'ArrowLeft' ? -step : 0;
      const dy = e.key === 'ArrowDown' ? step : e.key === 'ArrowUp' ? -step : 0;
      const movable = selectedLayers.filter((l) => !l.locked);
      if (movable.length === 1) {
        const l = movable[0];
        onCommitTransform(l.id, l.x + dx, l.y + dy, l.scale, l.rotation);
      } else if (movable.length > 1) {
        onCommitMultiTransform(
          movable.map((l) => ({ layer_id: l.id, x: l.x + dx, y: l.y + dy, scale: l.scale, rotation: l.rotation }))
        );
      }
    }
  }
</script>

<svelte:window onkeydown={onWindowKeydown} onpointermove={onPointerMove} onpointerup={onPointerUp} />

<div
  class="gizmo-overlay"
  bind:this={gizmoEl}
  onpointerdown={startMarquee}
  role="presentation"
>
  <!-- Optional Grid -->
  {#if showGrid}
    <div
      class="canvas-grid"
      style="background-size:{gridSize * zoom}px {gridSize * zoom}px;"
    ></div>
  {/if}

  <!-- User Draggable Guides -->
  {#each guides as g}
    <div
      class="user-guide"
      class:v={g.axis === 'v'}
      class:h={g.axis === 'h'}
      style={g.axis === 'v' ? `left:${g.pos * zoom}px;` : `top:${g.pos * zoom}px;`}
      onpointerdown={(e) => startGuideDrag(e, g)}
      ondblclick={(e) => { e.stopPropagation(); onRemoveGuide?.(g.id); }}
      title="Drag to move · Double-click to remove"
      role="presentation"
    ></div>
  {/each}

  {#if !cropMode}
    <!-- Smart Snap Guides & Distance -->
    {#if snapGuides.v !== null}
      <div class="snap-line vertical" style="left:{snapGuides.v * zoom}px;"></div>
    {/if}
    {#if snapGuides.h !== null}
      <div class="snap-line horizontal" style="top:{snapGuides.h * zoom}px;"></div>
    {/if}
    {#if snapGuides.distLabel}
      <div
        class="dist-badge"
        style="left:{snapGuides.distLabel.x * zoom}px; top:{snapGuides.distLabel.y * zoom}px;"
      >
        {snapGuides.distLabel.text}
      </div>
    {/if}

    <!-- Layer Hitboxes -->
    {#each layers as layer}
      {#if layer.visible}
        {@const box = layerBox(layer, mode === 'move' || mode === 'resize' || mode === 'rotate')}
        {@const isSel = selectedLayerIds.includes(layer.id)}
        <div
          class="layer-hitbox"
          class:multi-member={isSel && selectedLayerIds.length > 1}
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

    <!-- Selection Bounding Box (Single or Multi) -->
    {#if selectionBounds}
      {@const allLocked = selectedLayers.every((l) => l.locked)}
      <div
        class="selection-box"
        class:locked={allLocked}
        style="
          left:{selectionBounds.x * zoom}px;
          top:{selectionBounds.y * zoom}px;
          width:{selectionBounds.w * zoom}px;
          height:{selectionBounds.h * zoom}px;
          transform:rotate({selectionBounds.rot}deg);
        "
        onpointerdown={(e) => primaryLayer && startMove(e, primaryLayer)}
        ondblclick={(e) => {
          e.stopPropagation();
          if (primaryLayer?.kind?.type === 'Text3D') onDoubleClickText(primaryLayer);
        }}
        role="presentation"
      >
        {#if !allLocked}
          <div class="rot-stem"></div>
          <div class="rot-handle" onpointerdown={startRotate} title="Drag to rotate" role="presentation"></div>
          <div class="corner nw" onpointerdown={startResize} role="presentation"></div>
          <div class="corner ne" onpointerdown={startResize} role="presentation"></div>
          <div class="corner sw" onpointerdown={startResize} role="presentation"></div>
          <div class="corner se" onpointerdown={startResize} role="presentation"></div>
        {/if}
      </div>
    {/if}

    <!-- Marquee Selection Rectangle -->
    {#if mode === 'marquee'}
      {@const mx = Math.min(marquee.x1, marquee.x2) * zoom}
      {@const my = Math.min(marquee.y1, marquee.y2) * zoom}
      {@const mw = Math.abs(marquee.x2 - marquee.x1) * zoom}
      {@const mh = Math.abs(marquee.y2 - marquee.y1) * zoom}
      <div class="marquee-box" style="left:{mx}px; top:{my}px; width:{mw}px; height:{mh}px;"></div>
    {/if}
  {:else}
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
  .gizmo-overlay { position:absolute; inset:0; pointer-events:auto; overflow:hidden; }
  .canvas-grid { position:absolute; inset:0; pointer-events:none; background-image:linear-gradient(to right, rgba(255,255,255,0.08) 1px, transparent 1px), linear-gradient(to bottom, rgba(255,255,255,0.08) 1px, transparent 1px); }

  .user-guide { position:absolute; background:#22d3ee; opacity:0.85; z-index:28; pointer-events:auto; }
  .user-guide.v { top:0; bottom:0; width:1px; cursor:ew-resize; }
  .user-guide.h { left:0; right:0; height:1px; cursor:ns-resize; }
  .user-guide:hover { opacity:1; box-shadow:0 0 6px #22d3ee; }

  .snap-line { position:absolute; background:#38bdf8; box-shadow:0 0 8px rgba(56,189,248,0.9); z-index:30; pointer-events:none; }
  .snap-line.vertical { top:0; bottom:0; width:1px; }
  .snap-line.horizontal { left:0; right:0; height:1px; }
  .dist-badge { position:absolute; background:#38bdf8; color:#090b0e; font-size:10px; font-weight:700; padding:1px 5px; border-radius:4px; z-index:32; pointer-events:none; }

  .layer-hitbox { position:absolute; pointer-events:auto; cursor:pointer; transform-origin:center center; }
  .layer-hitbox:hover { outline:1px dashed rgba(56,189,248,0.45); }
  .layer-hitbox.multi-member { outline:1px solid rgba(56,189,248,0.65); }

  .selection-box { position:absolute; pointer-events:auto; border:1.5px solid #38bdf8; box-shadow:0 0 0 1px rgba(0,0,0,0.45); cursor:grab; transform-origin:center center; z-index:20; }
  .selection-box:active { cursor:grabbing; }
  .selection-box.locked { border-style:dashed; border-color:#f59e0b; cursor:not-allowed; }

  .marquee-box { position:absolute; border:1px solid #38bdf8; background:rgba(56,189,248,0.14); pointer-events:none; z-index:35; }

  .corner { position:absolute; width:10px; height:10px; background:#ffffff; border:2px solid #38bdf8; border-radius:2px; z-index:25; }
  .corner.nw { top:-5px; left:-5px; cursor:nwse-resize; }
  .corner.ne { top:-5px; right:-5px; cursor:nesw-resize; }
  .corner.sw { bottom:-5px; left:-5px; cursor:nesw-resize; }
  .corner.se { bottom:-5px; right:-5px; cursor:nwse-resize; }

  .rot-stem { position:absolute; top:-22px; left:50%; width:1.5px; height:18px; background:#38bdf8; transform:translateX(-50%); }
  .rot-handle { position:absolute; top:-30px; left:50%; width:12px; height:12px; border-radius:50%; background:#38bdf8; border:2px solid #ffffff; transform:translateX(-50%); cursor:grab; z-index:25; }

  .crop-box { position:absolute; pointer-events:auto; border:2px solid #38bdf8; box-shadow:0 0 0 9999px rgba(0,0,0,0.58); cursor:move; z-index:40; }
  .crop-grid { position:absolute; inset:0; background-image:linear-gradient(to right, rgba(255,255,255,0.22) 1px, transparent 1px), linear-gradient(to bottom, rgba(255,255,255,0.22) 1px, transparent 1px); background-size:33.33% 33.33%; pointer-events:none; }
  .crop-dims { position:absolute; bottom:-24px; left:0; background:#38bdf8; color:#090b0e; font-size:10px; font-weight:700; padding:2px 7px; border-radius:4px; white-space:nowrap; }
</style>
