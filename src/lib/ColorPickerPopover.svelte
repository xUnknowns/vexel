<script>
  import { Pipette, Plus, X } from 'lucide-svelte';

  let {
    label = '',
    color = $bindable('#FFFFFF'),
    brandColors = [],
    recentColors = [],
    compact = false,
    disabled = false,
    onChange = () => {},
    onSaveBrandColor = () => {},
    onRemoveBrandColor = () => {},
  } = $props();

  let open = $state(false);

  function pick(hex) {
    const normalized = hex.toUpperCase();
    color = normalized;
    onChange(normalized);
  }

  async function useEyedropper() {
    if (typeof window !== 'undefined' && 'EyeDropper' in window) {
      try {
        // @ts-ignore
        const dropper = new window.EyeDropper();
        const res = await dropper.open();
        if (res?.sRGBHex) pick(res.sRGBHex);
      } catch {
        // User cancelled eyedropper
      }
    }
  }
</script>

<div class="cp-wrap" class:disabled>
  {#if label && !compact}
    <span class="cp-label">{label}</span>
  {/if}
  <button
    class="cp-trigger"
    class:compact
    onclick={() => (open = !open)}
    title={label || 'Pick color'}
    type="button"
    {disabled}
  >
    <span class="cp-swatch" style="background:{color};"></span>
    {#if !compact}
      <span class="cp-hex">{color}</span>
    {/if}
  </button>

  {#if open}
    <div class="cp-backdrop" onclick={() => (open = false)} role="presentation"></div>
    <div class="cp-popover" onclick={(e) => e.stopPropagation()} role="dialog" aria-label="Color Picker">
      <div class="cp-row">
        <input
          type="color"
          class="cp-native"
          value={color.slice(0, 7)}
          oninput={(e) => pick(e.currentTarget.value)}
        />
        <input
          type="text"
          class="cp-text"
          value={color}
          maxlength="7"
          oninput={(e) => {
            const v = e.currentTarget.value;
            if (/^#[0-9A-Fa-f]{6}$/.test(v)) pick(v);
          }}
        />
        <button class="cp-icon-btn" onclick={useEyedropper} title="Eyedropper" type="button">
          <Pipette size={13} strokeWidth={1.75} />
        </button>
      </div>

      {#if recentColors.length > 0}
        <div class="cp-sec">
          <span class="cp-sec-lbl">Recent</span>
          <div class="cp-swatches">
            {#each recentColors as rc}
              <button class="sw" style="background:{rc};" onclick={() => pick(rc)} title={rc} type="button"></button>
            {/each}
          </div>
        </div>
      {/if}

      <div class="cp-sec">
        <div class="cp-sec-head">
          <span class="cp-sec-lbl">Brand Kit</span>
          <button class="cp-add-brand" onclick={() => onSaveBrandColor(color)} title="Save current color to Brand Kit" type="button">
            <Plus size={11} strokeWidth={2} /> Add
          </button>
        </div>
        <div class="cp-swatches">
          {#each brandColors as bc}
            <div class="sw-wrap">
              <button class="sw" style="background:{bc};" onclick={() => pick(bc)} title={bc} type="button"></button>
              <button class="sw-del" onclick={() => onRemoveBrandColor(bc)} title="Remove" type="button">
                <X size={8} strokeWidth={2.5} />
              </button>
            </div>
          {/each}
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .cp-wrap { position:relative; display:flex; flex-direction:column; gap:3px; }
  .cp-wrap.disabled { opacity:0.45; pointer-events:none; }
  .cp-label { font-size:10px; font-weight:600; color:var(--text-secondary); }
  .cp-trigger { display:flex; align-items:center; gap:7px; background:var(--bg-input); border:1px solid var(--border-input); border-radius:var(--r-sm); padding:4px 8px; color:var(--text-primary); height:30px; }
  .cp-trigger.compact { padding:3px; width:28px; height:28px; justify-content:center; }
  .cp-trigger:hover { border-color:#38bdf8; }
  .cp-swatch { width:16px; height:16px; border-radius:4px; border:1px solid rgba(255,255,255,0.2); flex-shrink:0; }
  .cp-hex { font-size:11px; font-family:var(--mono); }

  .cp-backdrop { position:fixed; inset:0; z-index:900; }
  .cp-popover { position:absolute; top:calc(100% + 6px); right:0; width:218px; background:var(--bg-card); border:1px solid var(--border); border-radius:var(--r-md); padding:10px; display:flex; flex-direction:column; gap:10px; box-shadow:0 16px 40px rgba(0,0,0,0.65); z-index:950; }
  .cp-row { display:flex; align-items:center; gap:6px; }
  .cp-native { width:30px; height:28px; border:none; background:transparent; padding:0; cursor:pointer; flex-shrink:0; }
  .cp-text { font-family:var(--mono); font-size:11px; padding:5px 7px; }
  .cp-icon-btn { width:28px; height:28px; display:grid; place-items:center; border-radius:var(--r-sm); background:var(--bg-elevated); border:1px solid var(--border); color:var(--text-secondary); flex-shrink:0; }
  .cp-icon-btn:hover { color:#38bdf8; border-color:#38bdf8; }

  .cp-sec { display:flex; flex-direction:column; gap:5px; }
  .cp-sec-head { display:flex; align-items:center; justify-content:space-between; }
  .cp-sec-lbl { font-size:10px; font-weight:700; text-transform:uppercase; letter-spacing:0.05em; color:var(--text-muted); }
  .cp-add-brand { display:flex; align-items:center; gap:3px; font-size:10px; color:#38bdf8; background:transparent; padding:2px 4px; }

  .cp-swatches { display:flex; flex-wrap:wrap; gap:6px; }
  .sw-wrap { position:relative; }
  .sw { width:20px; height:20px; border-radius:5px; border:1.5px solid rgba(255,255,255,0.18); padding:0; }
  .sw:hover { transform:scale(1.1); border-color:#38bdf8; }
  .sw-del { position:absolute; top:-4px; right:-4px; width:12px; height:12px; border-radius:50%; background:#111; color:#fff; display:grid; place-items:center; opacity:0; padding:0; }
  .sw-wrap:hover .sw-del { opacity:1; }
</style>
