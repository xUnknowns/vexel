<script>
  import { invoke, convertFileSrc } from '@tauri-apps/api/core';
  import { open } from '@tauri-apps/plugin-dialog';
  import { X, Palette, Plus, Trash2, Upload } from 'lucide-svelte';

  let { show = false, availableFonts = [], onClose, onUpdated } = $props();

  let store = $state({ active_id: 'default', kits: [] });
  let activeKit = $derived(store.kits.find((k) => k.id === store.active_id) || store.kits[0] || null);
  let newKitName = $state('');
  let newColorName = $state('Accent');
  let newColorHex = $state('#38BDF8');
  let feedback = $state('');

  async function loadStore() {
    try {
      store = await invoke('list_brand_kits');
    } catch (e) {
      feedback = String(e);
    }
  }

  $effect(() => {
    if (show) {
      feedback = '';
      loadStore();
    }
  });

  async function switchKit(id) {
    store = await invoke('set_active_brand_kit', { kitId: id });
    if (onUpdated) onUpdated(store);
  }

  async function createKit() {
    if (!newKitName.trim()) return;
    const profile = {
      id: `kit_${Date.now()}`,
      name: newKitName.trim(),
      colors: [
        { name: 'Primary', hex: '#6366F1' },
        { name: 'Accent', hex: '#38BDF8' },
      ],
      fonts: ['bold_display', 'modern_sans'],
      logos: [],
    };
    store = await invoke('save_brand_kit_profile', { profile });
    newKitName = '';
    if (onUpdated) onUpdated(store);
  }

  async function saveActiveKit(updated) {
    store = await invoke('save_brand_kit_profile', { profile: updated });
    feedback = 'Brand Kit saved';
    if (onUpdated) onUpdated(store);
  }

  async function deleteActiveKit() {
    if (!activeKit || store.kits.length <= 1) return;
    try {
      store = await invoke('delete_brand_kit_profile', { kitId: activeKit.id });
      if (onUpdated) onUpdated(store);
    } catch (e) {
      feedback = String(e);
    }
  }

  function addColor() {
    if (!activeKit) return;
    const updated = {
      ...activeKit,
      colors: [...activeKit.colors, { name: newColorName.trim() || 'Color', hex: newColorHex.toUpperCase() }],
    };
    saveActiveKit(updated);
  }

  function removeColor(idx) {
    if (!activeKit) return;
    const updated = { ...activeKit, colors: activeKit.colors.filter((_, i) => i !== idx) };
    saveActiveKit(updated);
  }

  function toggleFont(fontId) {
    if (!activeKit) return;
    const exists = activeKit.fonts.includes(fontId);
    const fonts = exists ? activeKit.fonts.filter((f) => f !== fontId) : [...activeKit.fonts, fontId];
    saveActiveKit({ ...activeKit, fonts });
  }

  async function uploadLogo() {
    if (!activeKit) return;
    const sel = await open({
      multiple: false,
      title: 'Upload Brand Logo',
      filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp'] }],
    });
    if (typeof sel === 'string' && sel) {
      store = await invoke('add_brand_logo', { kitId: activeKit.id, sourcePath: sel });
      if (onUpdated) onUpdated(store);
    }
  }

  function removeLogo(idx) {
    if (!activeKit) return;
    const updated = { ...activeKit, logos: activeKit.logos.filter((_, i) => i !== idx) };
    saveActiveKit(updated);
  }
</script>

{#if show}
  <div class="backdrop" onclick={onClose} role="presentation">
    <div class="modal" onclick={(e) => e.stopPropagation()} role="dialog" aria-label="Brand Kit Manager">
      <div class="modal-header">
        <span class="modal-title"><Palette size={15} strokeWidth={2} /> Brand Kits</span>
        <button class="close-btn" onclick={onClose}><X size={16} strokeWidth={2} /></button>
      </div>

      <div class="modal-body">
        <div class="kit-bar">
          <select class="select-input" value={store.active_id} onchange={(e) => switchKit(e.currentTarget.value)} style="flex:1;">
            {#each store.kits as k}
              <option value={k.id}>{k.name}</option>
            {/each}
          </select>
          <input type="text" bind:value={newKitName} placeholder="New kit name…" style="width:150px;" />
          <button class="btn-primary sm" onclick={createKit} disabled={!newKitName.trim()}><Plus size={12} /> New</button>
          {#if store.kits.length > 1}
            <button class="btn-ghost sm danger" onclick={deleteActiveKit} title="Delete current kit"><Trash2 size={12} /></button>
          {/if}
        </div>

        {#if activeKit}
          <div class="section">
            <div class="sec-title">Named Brand Colors ({activeKit.colors.length})</div>
            <div class="swatches-list">
              {#each activeKit.colors as c, idx}
                <div class="color-chip">
                  <span class="dot" style="background:{c.hex};"></span>
                  <span>{c.name}</span>
                  <code>{c.hex}</code>
                  <button class="del-x" onclick={() => removeColor(idx)}><X size={10} /></button>
                </div>
              {/each}
            </div>
            <div class="add-row">
              <input type="color" bind:value={newColorHex} style="width:36px; height:30px; padding:2px;" />
              <input type="text" bind:value={newColorName} placeholder="Color name (e.g. Primary)" style="flex:1;" />
              <button class="btn-ghost sm" onclick={addColor}><Plus size={12} /> Add Color</button>
            </div>
          </div>

          <div class="section">
            <div class="sec-title">Brand Fonts</div>
            <div class="font-chips">
              {#each availableFonts as f}
                <button
                  class="font-chip"
                  class:selected={activeKit.fonts.includes(f.id)}
                  onclick={() => toggleFont(f.id)}
                >
                  {f.label}
                </button>
              {/each}
            </div>
          </div>

          <div class="section">
            <div class="sec-title" style="display:flex; justify-content:space-between; align-items:center;">
              <span>Brand Logos ({activeKit.logos.length})</span>
              <button class="btn-ghost sm" onclick={uploadLogo}><Upload size={12} /> Upload Logo</button>
            </div>
            {#if activeKit.logos.length > 0}
              <div class="logos-grid">
                {#each activeKit.logos as lg, idx}
                  <div class="logo-card">
                    <img src={`${convertFileSrc(lg.path)}?v=1`} alt={lg.name} />
                    <span>{lg.name}</span>
                    <button class="del-x" onclick={() => removeLogo(idx)}><X size={10} /></button>
                  </div>
                {/each}
              </div>
            {:else}
              <p class="note">Upload PNG logos here to reuse across templates.</p>
            {/if}
          </div>
        {/if}

        {#if feedback}
          <p class="feedback">{feedback}</p>
        {/if}
      </div>

      <div class="modal-footer">
        <button class="btn-ghost" onclick={onClose}>Close</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .backdrop { position:fixed; inset:0; background:rgba(0,0,0,0.68); backdrop-filter:blur(6px); display:grid; place-items:center; z-index:2200; }
  .modal { background:var(--bg-card); border:1px solid var(--border); border-radius:var(--r-xl); width:min(620px,94vw); max-height:88vh; display:flex; flex-direction:column; overflow:hidden; box-shadow:0 32px 80px rgba(0,0,0,0.5); }
  .modal-header { display:flex; align-items:center; justify-content:space-between; padding:16px 20px; border-bottom:1px solid var(--border); }
  .modal-title { display:flex; align-items:center; gap:8px; font-size:15px; font-weight:700; }
  .close-btn { display:grid; place-items:center; width:28px; height:28px; border-radius:var(--r-sm); background:var(--bg-elevated); color:var(--text-secondary); border:1px solid var(--border); }
  .modal-body { padding:20px; display:flex; flex-direction:column; gap:18px; overflow-y:auto; }
  .kit-bar { display:flex; gap:8px; align-items:center; }
  .section { display:flex; flex-direction:column; gap:8px; }
  .sec-title { font-size:11px; font-weight:700; text-transform:uppercase; letter-spacing:0.05em; color:var(--text-secondary); }
  .swatches-list { display:flex; flex-wrap:wrap; gap:6px; }
  .color-chip { display:flex; align-items:center; gap:6px; background:var(--bg-elevated); border:1px solid var(--border); border-radius:999px; padding:4px 10px; font-size:11px; }
  .dot { width:12px; height:12px; border-radius:50%; border:1px solid rgba(255,255,255,0.2); }
  .add-row { display:flex; gap:8px; align-items:center; }
  .font-chips { display:flex; flex-wrap:wrap; gap:6px; }
  .font-chip { font-size:11px; padding:5px 10px; border-radius:var(--r-sm); background:var(--bg-elevated); border:1px solid var(--border); color:var(--text-secondary); cursor:pointer; }
  .font-chip.selected { background:var(--accent-subtle); border-color:var(--border-active); color:var(--text-primary); font-weight:600; }
  .logos-grid { display:grid; grid-template-columns:repeat(auto-fill, minmax(110px, 1fr)); gap:8px; }
  .logo-card { position:relative; display:flex; flex-direction:column; align-items:center; gap:4px; background:var(--bg-elevated); border:1px solid var(--border); border-radius:var(--r-sm); padding:8px; font-size:11px; }
  .logo-card img { height:48px; width:100%; object-fit:contain; }
  .del-x { background:transparent; border:none; color:var(--text-muted); cursor:pointer; padding:2px; }
  .note { font-size:12px; color:var(--text-muted); }
  .feedback { font-size:12px; color:#34d399; text-align:center; }
  .sm { font-size:11px; padding:5px 10px; display:inline-flex; align-items:center; gap:5px; }
  .modal-footer { display:flex; justify-content:flex-end; padding:14px 20px; border-top:1px solid var(--border); }
</style>
