<script>
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { open } from '@tauri-apps/plugin-dialog';
  import { X, Layers, FolderOpen, Play, Square } from 'lucide-svelte';

  let { show = false, templates = [], onClose } = $props();

  let selectedTemplateId = $state('');
  let tableData = $state('title,subtitle,tag\nHOW TO BUILD APPS,Part 1 · Rust + Svelte,NEW\nDESIGN SYSTEMS,Fast UI Workflows,PRO');
  let filenamePattern = $state('{title}');
  let format = $state('png');
  let quality = $state(90);
  let destDir = $state('');
  let running = $state(false);
  let progress = $state(null);
  let summary = $state(null);
  let errorMsg = $state('');
  let activeJobId = $state('');

  $effect(() => {
    if (show && !selectedTemplateId && templates.length > 0) {
      selectedTemplateId = templates[0].id;
    }
  });

  $effect(() => {
    const unlisten = listen('batch-progress', (ev) => {
      if (ev.payload?.job_id === activeJobId) progress = ev.payload;
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  });

  async function pickDestDir() {
    const sel = await open({ directory: true, multiple: false, title: 'Select Batch Output Folder' });
    if (typeof sel === 'string' && sel) destDir = sel;
  }

  async function runBatch() {
    if (!selectedTemplateId || !tableData.trim() || running) return;
    if (!destDir) {
      await pickDestDir();
      if (!destDir) return;
    }
    running = true;
    activeJobId = crypto.randomUUID();
    errorMsg = '';
    summary = null;
    progress = null;
    try {
      summary = await invoke('batch_render_template', {
        jobId: activeJobId,
        templateId: selectedTemplateId,
        tableData,
        filenamePattern,
        destDir,
        format,
        quality: Number(quality),
      });
    } catch (e) {
      errorMsg = String(e);
    } finally {
      running = false;
    }
  }

  function cancelBatch() {
    if (activeJobId) invoke('cancel_batch_export', { jobId: activeJobId }).catch(() => {});
  }
</script>

{#if show}
  <div class="backdrop" onclick={(e) => { if (!running && e.target === e.currentTarget) onClose(); }} role="presentation">
    <div class="modal" onclick={(e) => e.stopPropagation()} role="dialog" aria-label="Batch Generator">
      <div class="modal-header">
        <span class="modal-title"><Layers size={15} strokeWidth={2} /> Batch Template Generator</span>
        <button class="close-btn" onclick={onClose}><X size={16} strokeWidth={2} /></button>
      </div>

      <div class="modal-body">
        <div class="grid-2">
          <label class="field">
            <span>Template</span>
            <select class="select-input" bind:value={selectedTemplateId}>
              {#each templates as t}
                <option value={t.id}>{t.name} ({t.canvas_width}×{t.canvas_height})</option>
              {/each}
            </select>
          </label>
          <label class="field">
            <span>Filename Pattern (e.g. {"{title}"} or {"{index}"})</span>
            <input type="text" bind:value={filenamePattern} placeholder="{title}" />
          </label>
        </div>

        <label class="field">
          <span>Data Table (CSV with header row, or JSON array)</span>
          <textarea rows="6" bind:value={tableData} style="font-family:var(--mono); font-size:11px;"></textarea>
        </label>

        <div class="grid-3">
          <label class="field">
            <span>Format</span>
            <select class="select-input" bind:value={format}>
              <option value="png">PNG</option>
              <option value="jpeg">JPEG</option>
              <option value="webp">WebP</option>
            </select>
          </label>
          <label class="field">
            <span>Quality ({quality}%)</span>
            <input type="range" min="10" max="100" bind:value={quality} />
          </label>
          <div class="field">
            <span>Output Folder</span>
            <button class="btn-ghost sm" onclick={pickDestDir}>
              <FolderOpen size={12} /> {destDir ? destDir.split(/[/\\]/).pop() : 'Choose Folder…'}
            </button>
          </div>
        </div>

        {#if progress && running}
          <div class="prog-box">
            <div class="prog-head">
              <span>Rendering: {progress.current_item}</span>
              <span>{progress.done} / {progress.total}</span>
            </div>
            <div class="prog-track">
              <div class="prog-fill" style="width:{progress.total ? Math.round((progress.done / progress.total) * 100) : 0}%;"></div>
            </div>
          </div>
        {/if}

        {#if summary}
          <div class="sum-box">
            <strong>{summary.canceled ? 'Canceled' : 'Completed'}:</strong> {summary.saved.length} exported
            {#if summary.errors.length > 0}
              · <span style="color:#f87171;">{summary.errors.length} error(s): {summary.errors.join('; ')}</span>
            {/if}
          </div>
        {/if}

        {#if errorMsg}
          <p class="err">{errorMsg}</p>
        {/if}
      </div>

      <div class="modal-footer">
        {#if running}
          <button class="btn-ghost" onclick={cancelBatch}><Square size={12} /> Cancel</button>
        {/if}
        <button class="btn-ghost" onclick={onClose} disabled={running}>Close</button>
        <button class="btn-primary" onclick={runBatch} disabled={running || !selectedTemplateId}>
          <Play size={12} /> {running ? 'Generating…' : 'Generate Batch'}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .backdrop { position:fixed; inset:0; background:rgba(0,0,0,0.68); backdrop-filter:blur(6px); display:grid; place-items:center; z-index:2200; }
  .modal { background:var(--bg-card); border:1px solid var(--border); border-radius:var(--r-xl); width:min(680px,94vw); max-height:88vh; display:flex; flex-direction:column; overflow:hidden; box-shadow:0 32px 80px rgba(0,0,0,0.5); }
  .modal-header { display:flex; align-items:center; justify-content:space-between; padding:16px 20px; border-bottom:1px solid var(--border); }
  .modal-title { display:flex; align-items:center; gap:8px; font-size:15px; font-weight:700; }
  .close-btn { display:grid; place-items:center; width:28px; height:28px; border-radius:var(--r-sm); background:var(--bg-elevated); color:var(--text-secondary); border:1px solid var(--border); }
  .modal-body { padding:20px; display:flex; flex-direction:column; gap:14px; overflow-y:auto; }
  .grid-2 { display:grid; grid-template-columns:1fr 1fr; gap:12px; }
  .grid-3 { display:grid; grid-template-columns:1fr 1fr 1fr; gap:12px; align-items:end; }
  .field { display:flex; flex-direction:column; gap:5px; font-size:11px; color:var(--text-secondary); }
  .prog-box { display:flex; flex-direction:column; gap:6px; background:var(--bg-elevated); padding:10px 12px; border-radius:var(--r-sm); border:1px solid var(--border); }
  .prog-head { display:flex; justify-content:space-between; font-size:11px; color:var(--text-secondary); }
  .prog-track { height:6px; background:var(--bg-canvas); border-radius:4px; overflow:hidden; }
  .prog-fill { height:100%; background:var(--accent); transition:width 0.15s; }
  .sum-box { font-size:12px; color:#34d399; background:rgba(16,185,129,0.08); border:1px solid rgba(16,185,129,0.3); padding:10px 12px; border-radius:var(--r-sm); }
  .err { font-size:12px; color:#f87171; }
  .sm { font-size:11px; padding:6px 10px; display:inline-flex; align-items:center; gap:5px; }
  .modal-footer { display:flex; justify-content:flex-end; gap:10px; padding:14px 20px; border-top:1px solid var(--border); }
</style>
