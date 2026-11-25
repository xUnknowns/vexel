<script>
  let {
    label = '',
    value = $bindable(0),
    min = -9999,
    max = 9999,
    step = 1,
    suffix = '',
    disabled = false,
    onScrubStart = () => {},
    onLiveChange = () => {},
    onCommit = () => {},
  } = $props();

  let scrubbing = $state(false);
  let startX = 0;
  let startVal = 0;

  function clampRound(v) {
    const clamped = Math.max(min, Math.min(max, v));
    const decimals = String(step).includes('.') ? String(step).split('.')[1].length : 0;
    return Number(clamped.toFixed(decimals));
  }

  function onLabelDown(e) {
    if (disabled) return;
    e.preventDefault();
    scrubbing = true;
    startX = e.clientX;
    startVal = Number(value) || 0;
    onScrubStart();
    e.currentTarget.setPointerCapture?.(e.pointerId);
  }

  function onLabelMove(e) {
    if (!scrubbing) return;
    const dx = e.clientX - startX;
    const mult = e.shiftKey ? 10 : 1;
    const next = clampRound(startVal + Math.round(dx / 2) * step * mult);
    if (next !== value) {
      value = next;
      onLiveChange(next);
    }
  }

  function onLabelUp() {
    if (!scrubbing) return;
    scrubbing = false;
    onCommit(Number(value));
  }

  function onInputKeydown(e) {
    if (disabled) return;
    if (e.key === 'ArrowUp' || e.key === 'ArrowDown') {
      e.preventDefault();
      const mult = e.shiftKey ? 10 : 1;
      const dir = e.key === 'ArrowUp' ? 1 : -1;
      const next = clampRound((Number(value) || 0) + dir * step * mult);
      value = next;
      onCommit(next);
    } else if (e.key === 'Enter') {
      onCommit(clampRound(Number(value) || 0));
    }
  }
</script>

<div class="scrub-field" class:disabled>
  <span
    class="scrub-label"
    class:scrubbing
    onpointerdown={onLabelDown}
    onpointermove={onLabelMove}
    onpointerup={onLabelUp}
    title="Drag horizontally to scrub"
    role="presentation"
  >
    {label}
  </span>
  <div class="scrub-input-wrap">
    <input
      type="number"
      {min}
      {max}
      {step}
      {disabled}
      bind:value
      onfocus={() => onScrubStart()}
      oninput={() => onLiveChange(clampRound(Number(value) || 0))}
      onblur={() => onCommit(clampRound(Number(value) || 0))}
      onkeydown={onInputKeydown}
    />
    {#if suffix}
      <span class="scrub-suffix">{suffix}</span>
    {/if}
  </div>
</div>

<style>
  .scrub-field { display:flex; flex-direction:column; gap:3px; min-width:0; }
  .scrub-field.disabled { opacity:0.45; pointer-events:none; }
  .scrub-label { font-size:10px; font-weight:600; color:var(--text-secondary); cursor:ew-resize; user-select:none; width:fit-content; }
  .scrub-label:hover, .scrub-label.scrubbing { color:#38bdf8; }
  .scrub-input-wrap { position:relative; display:flex; align-items:center; }
  .scrub-input-wrap input { padding:5px 8px; font-size:12px; font-variant-numeric:tabular-nums; }
  .scrub-suffix { position:absolute; right:7px; font-size:10px; color:var(--text-muted); pointer-events:none; }
</style>
