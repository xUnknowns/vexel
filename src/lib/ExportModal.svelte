<script>
  import { X, Download } from 'lucide-svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { save } from '@tauri-apps/plugin-dialog';

  let {
    show = false,
    hasProject = false,
    projectName = 'export',
    imgWidth = 0,
    imgHeight = 0,
    transparentBg = false,
    onClose
  } = $props();

  let format = $state('png');
  let quality = $state(90);
  let sizeMode = $state('original'); // 'original' | '0.5x' | '2x' | 'yt_thumb' | 'yt_banner' | 'tiktok' | 'ig_post' | 'logo' | 'x_header'
  let fileName = $state('export');
  let saving = $state(false);
  let feedback = $state('');

  $effect(() => {
    if (show) {
      fileName = (projectName || 'export').replace(/[^a-z0-9_\-]/gi, '_');
      feedback = '';
    }
  });

  const sizePresets = [
    { id: 'original', label: '1× Original', w: null, h: null, scale: 1.0 },
    { id: '0.5x', label: '0.5× Half', w: null, h: null, scale: 0.5 },
    { id: '2x', label: '2× Upscale', w: null, h: null, scale: 2.0 },
    { id: 'yt_thumb', label: 'YouTube Thumb', w: 1280, h: 720, scale: 1.0 },
    { id: 'yt_banner', label: 'YouTube Banner', w: 2560, h: 1440, scale: 1.0 },
    { id: 'tiktok', label: 'TikTok / Reels', w: 1080, h: 1920, scale: 1.0 },
    { id: 'ig_post', label: 'Instagram 1:1', w: 1080, h: 1080, scale: 1.0 },
    { id: 'logo', label: 'Logo PNG 1024', w: 1024, h: 1024, scale: 1.0 },
    { id: 'x_header', label: 'X / Twitter Header', w: 1500, h: 500, scale: 1.0 },
    { id: 'twitch', label: 'Twitch Panel', w: 320, h: 160, scale: 1.0 },
    { id: 'linkedin', label: 'LinkedIn Banner', w: 1584, h: 396, scale: 1.0 },
    { id: 'fb_cover', label: 'Facebook Cover', w: 820, h: 312, scale: 1.0 },
    { id: 'discord', label: 'Discord Banner', w: 960, h: 540, scale: 1.0 },
    { id: 'favicon32', label: 'Favicon 32px', w: 32, h: 32, scale: 1.0 },
    { id: 'favicon64', label: 'Favicon 64px', w: 64, h: 64, scale: 1.0 },
  ];

  let activePreset = $derived(sizePresets.find((p) => p.id === sizeMode) ?? sizePresets[0]);
  let finalW = $derived(activePreset.w ?? Math.max(1, Math.round(imgWidth * activePreset.scale)));
  let finalH = $derived(activePreset.h ?? Math.max(1, Math.round(imgHeight * activePreset.scale)));

  const formats = [
    { id: 'png', label: 'PNG', badge: 'Transparent' },
    { id: 'jpeg', label: 'JPEG', badge: 'Compact' },
    { id: 'webp', label: 'WebP', badge: 'Modern Web' },
  ];

  async function handleSave() {
    if (!hasProject || saving) return;
    saving = true;
    feedback = '';

    try {
      const ext = format === 'jpeg' ? 'jpg' : format;
      const dest = await save({
        title: 'Export Image',
        defaultPath: `${fileName}.${ext}`,
        filters: [
          { name: 'PNG Image', extensions: ['png'] },
          { name: 'JPEG Image', extensions: ['jpg', 'jpeg'] },
          { name: 'WebP Image', extensions: ['webp'] },
        ],
      });
      if (!dest) {
        saving = false;
        return;
      }

      await invoke('export_configured', {
        destPath: dest,
        format,
        quality: Number(quality),
        scale: Number(activePreset.scale),
        targetWidth: activePreset.w,
        targetHeight: activePreset.h,
      });

      feedback = `Saved as ${dest.split(/[/\\]/).pop()}`;
      setTimeout(() => onClose(), 1000);
    } catch (e) {
      feedback = `Export failed: ${e}`;
    } finally {
      saving = false;
    }
  }

  function handleKeydown(e) {
    if (e.key === 'Escape') onClose();
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if show}
  <div class="backdrop" onclick={onClose} role="presentation">
    <div class="modal" onclick={(e) => e.stopPropagation()} role="dialog" aria-label="Export Image">
      <div class="modal-header">
        <span class="modal-title"><Download size={15} strokeWidth={2} /> Export Image</span>
        <button class="close-btn" onclick={onClose}><X size={16} strokeWidth={2} /></button>
      </div>

      <div class="modal-body">
        <div class="field-group">
          <label class="field-label">File Name</label>
          <input type="text" bind:value={fileName} placeholder="export" spellcheck="false" />
        </div>

        <div class="field-group">
          <label class="field-label">Format</label>
          <div class="format-row">
            {#each formats as f}
              <button
                class="format-btn"
                class:selected={format === f.id}
                onclick={() => (format = f.id)}
              >
                <span class="fmt-label">{f.label}</span>
                <span class="fmt-badge">{f.badge}</span>
              </button>
            {/each}
          </div>
          {#if transparentBg && format === 'jpeg'}
            <p class="warn-note">JPEG does not support alpha transparency — choose PNG for transparent logos.</p>
          {/if}
        </div>

        {#if format !== 'png'}
          <div class="field-group">
            <label class="field-label">
              <span>Quality</span>
              <span class="val">{quality}%</span>
            </label>
            <input type="range" min="10" max="100" step="1" bind:value={quality} />
          </div>
        {/if}

        <div class="field-group">
          <label class="field-label">Platform &amp; Resolution Presets</label>
          <div class="scale-row">
            {#each sizePresets as s}
              <button
                class="scale-btn"
                class:selected={sizeMode === s.id}
                onclick={() => (sizeMode = s.id)}
              >
                {s.label}
              </button>
            {/each}
          </div>
          <div class="dim-preview">Output: <strong>{finalW} × {finalH} px</strong></div>
        </div>

        {#if feedback}
          <p class="feedback">{feedback}</p>
        {/if}
      </div>

      <div class="modal-footer">
        <button class="btn-ghost" onclick={onClose} disabled={saving}>Cancel</button>
        <button class="btn-primary" onclick={handleSave} disabled={!hasProject || saving}>
          {#if saving}
            Saving…
          {:else}
            <Download size={14} strokeWidth={2} /> Download / Save File
          {/if}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .backdrop { position:fixed; inset:0; background:rgba(0,0,0,0.65); backdrop-filter:blur(6px); display:grid; place-items:center; z-index:2000; }
  .modal { background:var(--bg-card); border:1px solid var(--border); border-radius:var(--r-xl); width:min(540px,94vw); display:flex; flex-direction:column; box-shadow:0 32px 80px rgba(0,0,0,0.5); }
  .modal-header { display:flex; align-items:center; justify-content:space-between; padding:16px 20px; border-bottom:1px solid var(--border); }
  .modal-title { display:flex; align-items:center; gap:8px; font-size:15px; font-weight:700; }
  .close-btn { display:grid; place-items:center; width:28px; height:28px; border-radius:var(--r-sm); background:var(--bg-elevated); color:var(--text-secondary); border:1px solid var(--border); }
  .modal-body { padding:20px; display:flex; flex-direction:column; gap:16px; max-height:75vh; overflow-y:auto; }
  .field-group { display:flex; flex-direction:column; gap:8px; }
  .field-label { display:flex; justify-content:space-between; font-size:12px; font-weight:600; color:var(--text-secondary); text-transform:uppercase; letter-spacing:0.05em; }
  .val { color:var(--text-primary); font-variant-numeric:tabular-nums; }
  .format-row { display:grid; grid-template-columns:repeat(3,1fr); gap:8px; }
  .format-btn { display:flex; flex-direction:column; align-items:center; gap:4px; padding:11px 8px; border-radius:var(--r-md); background:var(--bg-elevated); border:1.5px solid var(--border); color:var(--text-secondary); }
  .format-btn.selected { background:var(--accent-subtle); border-color:var(--accent); color:var(--text-primary); }
  .fmt-label { font-size:15px; font-weight:700; }
  .fmt-badge { font-size:10px; opacity:0.75; }
  .warn-note { font-size:11px; color:#f59e0b; }
  .scale-row { display:grid; grid-template-columns:repeat(3,1fr); gap:6px; }
  .scale-btn { padding:8px 6px; border-radius:var(--r-sm); background:var(--bg-elevated); border:1.5px solid var(--border); color:var(--text-secondary); font-size:11px; }
  .scale-btn.selected { background:var(--accent-subtle); border-color:var(--accent); color:var(--text-primary); font-weight:600; }
  .dim-preview { font-size:12px; color:var(--text-secondary); text-align:center; }
  .dim-preview strong { color:var(--text-primary); }
  .feedback { font-size:12px; color:#34d399; text-align:center; }
  .modal-footer { display:flex; justify-content:flex-end; gap:10px; padding:14px 20px; border-top:1px solid var(--border); }
  .modal-footer .btn-primary { display:flex; align-items:center; gap:7px; }
</style>
