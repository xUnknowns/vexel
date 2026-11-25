<script>
  import { X, FolderOpen, Youtube, Instagram, Smartphone, LayoutGrid, Monitor, Square } from 'lucide-svelte';

  let { show, onClose, onOpenFile, onCreatePreset } = $props();

  let category    = $state('open');  // 'open' | 'social' | 'video' | 'custom'
  let customW     = $state(1920);
  let customH     = $state(1080);

  const presets = {
    social: [
      { label: 'Square Post',   sub: '1:1',  w: 1080, h: 1080, color: '#6366f1' },
      { label: 'Portrait Post', sub: '4:5',  w: 1080, h: 1350, color: '#8b5cf6' },
      { label: 'Story',         sub: '9:16', w: 1080, h: 1920, color: '#ec4899' },
      { label: 'Banner',        sub: '3:1',  w: 1500, h: 500,  color: '#06b6d4' },
    ],
    video: [
      { label: 'YouTube Thumbnail', sub: '16:9',  w: 1280, h: 720,  color: '#ef4444' },
      { label: 'Full HD',           sub: '16:9',  w: 1920, h: 1080, color: '#f97316' },
      { label: '4K UHD',            sub: '16:9',  w: 3840, h: 2160, color: '#eab308' },
      { label: 'Cinematic',         sub: '21:9',  w: 2560, h: 1080, color: '#a3e635' },
    ],
  };

  function create(w, h, name) { onCreatePreset(w, h, name); }

  function handleKeydown(e) {
    if (e.key === 'Escape') onClose();
  }

  function blockBg(e) { e.stopPropagation(); }

  const categories = [
    { id: 'open',   label: 'Open Local File',      icon: FolderOpen },
    { id: 'social', label: 'Social Media',          icon: Instagram  },
    { id: 'video',  label: 'Video & Thumbnails',    icon: Monitor    },
    { id: 'custom', label: 'Custom Dimensions',     icon: LayoutGrid },
  ];
</script>

<svelte:window onkeydown={handleKeydown} />

{#if show}
  <!-- Backdrop -->
  <div class="backdrop" onclick={onClose} role="presentation">
    <!-- Modal -->
    <div class="modal" onclick={blockBg} role="dialog" aria-modal="true" aria-label="Create new design">
      <!-- Header -->
      <div class="modal-header">
        <span class="modal-title">New Design</span>
        <button class="close-btn" onclick={onClose} title="Close">
          <X size={18} strokeWidth={2} />
        </button>
      </div>

      <!-- Body -->
      <div class="modal-body">
        <!-- Left column: categories -->
        <aside class="cat-col">
          {#each categories as cat}
            <button
              class="cat-btn"
              class:active={category === cat.id}
              onclick={() => (category = cat.id)}
            >
              <svelte:component this={cat.icon} size={16} strokeWidth={1.75} />
              {cat.label}
            </button>
          {/each}
        </aside>

        <!-- Right column: presets / action -->
        <div class="preset-col">

          {#if category === 'open'}
            <div class="open-pane">
              <div class="open-icon-wrap">
                <FolderOpen size={36} strokeWidth={1.25} />
              </div>
              <h3>Open an image from your computer</h3>
              <p>Supports PNG, JPEG, WebP, BMP, GIF and TIFF.</p>
              <button class="btn-primary" onclick={onOpenFile}>Browse Files…</button>
            </div>

          {:else if category === 'social' || category === 'video'}
            <div class="preset-grid">
              {#each presets[category] as p}
                <button class="preset-card" onclick={() => create(p.w, p.h, p.label)}>
                  <div class="preset-thumb" style="background: {p.color}18; border-color: {p.color}44;">
                    <div
                      class="preset-ratio"
                      style="
                        width: min(72px, {(p.w/p.h) * 48}px);
                        height: min(72px, {(p.h/p.w) * 72}px);
                        background: {p.color}33;
                        border: 1.5px solid {p.color}66;
                        border-radius: 6px;
                      "
                    ></div>
                  </div>
                  <span class="preset-label">{p.label}</span>
                  <span class="preset-sub">{p.sub} · {p.w} × {p.h}</span>
                </button>
              {/each}
            </div>

          {:else if category === 'custom'}
            <div class="custom-pane">
              <h3>Custom Canvas</h3>
              <p>Enter your desired dimensions in pixels.</p>
              <div class="dim-row">
                <label class="dim-field">
                  <span>Width (px)</span>
                  <input type="number" min="1" max="8192" bind:value={customW} />
                </label>
                <span class="dim-sep">×</span>
                <label class="dim-field">
                  <span>Height (px)</span>
                  <input type="number" min="1" max="8192" bind:value={customH} />
                </label>
              </div>
              <button
                class="btn-primary"
                onclick={() => create(Number(customW), Number(customH), `${customW}×${customH} Canvas`)}
                disabled={!customW || !customH || customW < 1 || customH < 1}
              >
                Create Canvas
              </button>
            </div>
          {/if}

        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0,0,0,0.65);
    backdrop-filter: blur(6px);
    display: grid;
    place-items: center;
    z-index: 1000;
    animation: fadeIn 0.15s ease;
  }

  @keyframes fadeIn {
    from { opacity: 0; }
    to   { opacity: 1; }
  }

  .modal {
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: var(--r-xl);
    width: min(820px, 92vw);
    max-height: 88vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    box-shadow: 0 32px 80px rgba(0,0,0,0.5);
    animation: slideUp 0.18s cubic-bezier(0.34, 1.56, 0.64, 1);
  }

  @keyframes slideUp {
    from { opacity: 0; transform: translateY(16px) scale(0.97); }
    to   { opacity: 1; transform: translateY(0) scale(1); }
  }

  .modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 18px 22px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }
  .modal-title { font-size: 15px; font-weight: 700; }

  .close-btn {
    display: grid;
    place-items: center;
    width: 30px; height: 30px;
    border-radius: var(--r-sm);
    background: var(--bg-elevated);
    color: var(--text-secondary);
    border: 1px solid var(--border);
  }
  .close-btn:hover:not(:disabled) {
    color: var(--text-primary);
    background: var(--bg-card-hover);
  }

  .modal-body {
    display: flex;
    flex: 1;
    overflow: hidden;
  }

  /* Categories column */
  .cat-col {
    width: 220px;
    flex-shrink: 0;
    border-right: 1px solid var(--border);
    padding: 16px 12px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .cat-btn {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 10px 14px;
    border-radius: var(--r-sm);
    background: transparent;
    color: var(--text-secondary);
    font-size: 13px;
    font-weight: 500;
    text-align: left;
    border: 1px solid transparent;
  }
  .cat-btn:hover:not(:disabled) {
    background: var(--bg-elevated);
    color: var(--text-primary);
  }
  .cat-btn.active {
    background: var(--accent-subtle);
    color: var(--text-primary);
    border-color: var(--border-active);
    font-weight: 600;
  }

  /* Preset column */
  .preset-col {
    flex: 1;
    padding: 24px;
    overflow-y: auto;
    display: flex;
    align-items: flex-start;
  }

  /* Open pane */
  .open-pane {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
    text-align: center;
    width: 100%;
    padding: 32px 0;
  }
  .open-icon-wrap {
    color: var(--text-muted);
    opacity: 0.6;
  }
  .open-pane h3 { font-size: 16px; font-weight: 600; }
  .open-pane p  { font-size: 13px; color: var(--text-secondary); }

  /* Preset grid */
  .preset-grid {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 12px;
    width: 100%;
  }

  .preset-card {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    background: var(--bg-elevated);
    border: 1px solid var(--border);
    border-radius: var(--r-md);
    padding: 16px 12px;
    color: var(--text-secondary);
    text-align: center;
  }
  .preset-card:hover:not(:disabled) {
    border-color: var(--border-active);
    color: var(--text-primary);
    background: var(--bg-card-hover);
  }

  .preset-thumb {
    width: 100%;
    height: 80px;
    border-radius: var(--r-sm);
    display: grid;
    place-items: center;
    border: 1px solid;
  }
  .preset-label { font-size: 13px; font-weight: 600; }
  .preset-sub   { font-size: 11px; color: var(--text-muted); }

  /* Custom pane */
  .custom-pane {
    display: flex;
    flex-direction: column;
    gap: 16px;
    width: 100%;
    max-width: 420px;
  }
  .custom-pane h3 { font-size: 16px; font-weight: 600; }
  .custom-pane p  { font-size: 13px; color: var(--text-secondary); }

  .dim-row {
    display: flex;
    align-items: flex-end;
    gap: 12px;
  }
  .dim-field {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 12px;
    color: var(--text-secondary);
    flex: 1;
  }
  .dim-sep {
    padding-bottom: 10px;
    color: var(--text-muted);
    font-size: 18px;
    font-weight: 300;
  }
</style>
