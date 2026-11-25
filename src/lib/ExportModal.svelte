<script>
  import { X, Download, Image } from 'lucide-svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { save }   from '@tauri-apps/plugin-dialog';

  let {
    show        = false,
    currentPath = null,
    projectName = 'export',
    imgWidth    = 0,
    imgHeight   = 0,
    onClose
  } = $props();

  let format   = $state('png');     // 'png' | 'jpeg' | 'webp'
  let quality  = $state(90);        // 10–100 (jpeg/webp)
  let scale    = $state(1.0);       // 0.5 | 1.0 | 2.0
  let fileName = $state(projectName.replace(/[^a-z0-9_\-]/gi, '_'));
  let saving   = $state(false);
  let feedback = $state('');

  let finalW = $derived(Math.max(1, Math.round(imgWidth  * scale)));
  let finalH = $derived(Math.max(1, Math.round(imgHeight * scale)));

  const formats = [
    { id: 'png',  label: 'PNG',  badge: 'Lossless' },
    { id: 'jpeg', label: 'JPEG', badge: 'Compact'  },
    { id: 'webp', label: 'WebP', badge: 'Modern Web'},
  ];
  const scales = [
    { v: 0.5,  label: '0.5× Downscale' },
    { v: 1.0,  label: '1× Original'    },
    { v: 2.0,  label: '2× Upscale'     },
  ];

  async function handleSave() {
    if (!currentPath || saving) return;
    saving  = true;
    feedback = '';

    try {
      const ext  = format === 'jpeg' ? 'jpg' : format;
      const dest = await save({
        title:       'Export Image',
        defaultPath: `${fileName}.${ext}`,
        filters:     [
          { name: 'PNG Image',  extensions: ['png']       },
          { name: 'JPEG Image', extensions: ['jpg','jpeg'] },
          { name: 'WebP Image', extensions: ['webp']       },
        ],
      });
      if (!dest) { saving = false; return; }

      await invoke('export_image_configured', {
        sourcePath: currentPath,
        destPath:   dest,
        format,
        quality:    Number(quality),
        scale:      Number(scale),
      });

      feedback = `Saved as ${dest.split(/[/\\]/).pop()}`;
      setTimeout(() => onClose(), 1200);
    } catch (e) {
      feedback = `Export failed: ${e}`;
    } finally {
      saving = false;
    }
  }

  function handleKeydown(e) { if (e.key === 'Escape') onClose(); }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if show}
  <div class="backdrop" onclick={onClose} role="presentation">
    <div class="modal" onclick={(e) => e.stopPropagation()} role="dialog" aria-label="Export Image">

      <!-- Header -->
      <div class="modal-header">
        <span class="modal-title">
          <Download size={15} strokeWidth={2} /> Export Image
        </span>
        <button class="close-btn" onclick={onClose}><X size={16} strokeWidth={2} /></button>
      </div>

      <!-- Body -->
      <div class="modal-body">

        <!-- File Name -->
        <div class="field-group">
          <label class="field-label">File Name</label>
          <input
            type="text"
            bind:value={fileName}
            placeholder="export"
            spellcheck="false"
          />
        </div>

        <!-- Format -->
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
        </div>

        <!-- Quality (JPEG/WebP only) -->
        {#if format !== 'png'}
          <div class="field-group">
            <label class="field-label">
              <span>Quality</span>
              <span class="val">{quality}%</span>
            </label>
            <input type="range" min="10" max="100" step="1" bind:value={quality} />
          </div>
        {/if}

        <!-- Scale -->
        <div class="field-group">
          <label class="field-label">Scale / Resolution</label>
          <div class="scale-row">
            {#each scales as s}
              <button
                class="scale-btn"
                class:selected={scale === s.v}
                onclick={() => (scale = s.v)}
              >
                {s.label}
              </button>
            {/each}
          </div>
          <div class="dim-preview">
            Output: <strong>{finalW} × {finalH} px</strong>
          </div>
        </div>

        <!-- Feedback -->
        {#if feedback}
          <p class="feedback">{feedback}</p>
        {/if}
      </div>

      <!-- Footer -->
      <div class="modal-footer">
        <button class="btn-ghost" onclick={onClose} disabled={saving}>Cancel</button>
        <button
          class="btn-primary"
          onclick={handleSave}
          disabled={!currentPath || saving}
        >
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
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0,0,0,0.65);
    backdrop-filter: blur(6px);
    display: grid;
    place-items: center;
    z-index: 2000;
    animation: fadeIn 0.15s ease;
  }
  @keyframes fadeIn { from { opacity:0 } to { opacity:1 } }

  .modal {
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: var(--r-xl);
    width: min(480px, 94vw);
    display: flex;
    flex-direction: column;
    box-shadow: 0 32px 80px rgba(0,0,0,0.5);
    animation: slideUp 0.18s cubic-bezier(0.34,1.56,0.64,1);
  }
  @keyframes slideUp {
    from { opacity:0; transform: translateY(16px) scale(0.97) }
    to   { opacity:1; transform: translateY(0)   scale(1)    }
  }

  .modal-header {
    display: flex; align-items: center; justify-content: space-between;
    padding: 16px 20px;
    border-bottom: 1px solid var(--border);
    gap: 10px;
  }
  .modal-title {
    display: flex; align-items: center; gap: 8px;
    font-size: 15px; font-weight: 700;
  }
  .close-btn {
    display: grid; place-items: center;
    width: 28px; height: 28px;
    border-radius: var(--r-sm);
    background: var(--bg-elevated);
    color: var(--text-secondary);
    border: 1px solid var(--border);
  }
  .close-btn:hover:not(:disabled) { color: var(--text-primary); }

  .modal-body {
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 18px;
  }

  .field-group {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .field-label {
    display: flex;
    justify-content: space-between;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  .val { color: var(--text-primary); font-variant-numeric: tabular-nums; }

  /* Format buttons */
  .format-row { display: grid; grid-template-columns: repeat(3,1fr); gap: 8px; }
  .format-btn {
    display: flex; flex-direction: column; align-items: center; gap: 4px;
    padding: 12px 8px;
    border-radius: var(--r-md);
    background: var(--bg-elevated);
    border: 1.5px solid var(--border);
    color: var(--text-secondary);
  }
  .format-btn:hover:not(:disabled)  { border-color: var(--border-active); color: var(--text-primary); }
  .format-btn.selected {
    background: var(--accent-subtle);
    border-color: var(--accent);
    color: var(--text-primary);
  }
  .fmt-label { font-size: 15px; font-weight: 700; }
  .fmt-badge { font-size: 10px; color: inherit; opacity: 0.75; }

  /* Scale buttons */
  .scale-row { display: grid; grid-template-columns: repeat(3,1fr); gap: 8px; }
  .scale-btn {
    padding: 9px;
    border-radius: var(--r-sm);
    background: var(--bg-elevated);
    border: 1.5px solid var(--border);
    color: var(--text-secondary);
    font-size: 12px;
  }
  .scale-btn:hover:not(:disabled)  { border-color: var(--border-active); color: var(--text-primary); }
  .scale-btn.selected {
    background: var(--accent-subtle);
    border-color: var(--accent);
    color: var(--text-primary);
    font-weight: 600;
  }

  .dim-preview {
    font-size: 12px;
    color: var(--text-secondary);
    text-align: center;
  }
  .dim-preview strong { color: var(--text-primary); }

  .feedback {
    font-size: 12px;
    color: #34d399;
    text-align: center;
  }

  .modal-footer {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    padding: 14px 20px;
    border-top: 1px solid var(--border);
  }
  .modal-footer .btn-primary {
    display: flex; align-items: center; gap: 7px;
  }
</style>
