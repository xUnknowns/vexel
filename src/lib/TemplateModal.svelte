<script>
  import { invoke } from '@tauri-apps/api/core';
  import { open, save } from '@tauri-apps/plugin-dialog';
  import { X, Sparkles, Upload, Download, Layers } from 'lucide-svelte';

  let {
    show = false,
    mode = 'fill', // 'fill' | 'save'
    template = null,
    layers = [],
    onClose,
    onCreatedProject,
    onSavedTemplate
  } = $props();

  // Fill mode state
  let fieldValues = $state({});
  let projectName = $state('');
  let livePreview = $state('');
  let brandKits = $state([]);
  let selectedBrandKitId = $state('');
  let busy = $state(false);
  let errorMsg = $state('');

  // Save mode state
  let tplName = $state('');
  let tplCategory = $state('Custom');
  let editableRows = $state([]);

  $effect(() => {
    if (!show) return;
    errorMsg = '';
    if (mode === 'fill' && template) {
      projectName = template.name;
      selectedBrandKitId = '';
      invoke('list_brand_kits')
        .then((s) => { brandKits = s?.kits || []; })
        .catch(() => {});
      const init = {};
      for (const f of template.fields || []) {
        init[f.variable_name || f.id] = f.default_value || '';
      }
      fieldValues = init;
      livePreview = template.preview_data_url || '';
      refreshPreview();
    } else if (mode === 'save') {
      tplName = '';
      tplCategory = 'Custom';
      editableRows = (layers || []).map((l, idx) => {
        const t = l.kind?.type;
        const kind = t === 'Image' ? 'image' : t === 'Shape' ? 'color' : 'text';
        const defaultVal =
          t === 'Text3D'
            ? l.kind.content || ''
            : t === 'Image'
              ? l.kind.source_path || ''
              : '#6366F1';
        return {
          selected: t === 'Text3D' || t === 'Image',
          layer_id: l.id,
          label: l.name || `Field ${idx + 1}`,
          kind,
          variable_name: (l.name || `var_${idx + 1}`).toLowerCase().replace(/[^a-z0-9_]/g, '_'),
          default_value: defaultVal,
          image_fit: 'cover',
        };
      });
    }
  });

  let previewTimer = null;
  function schedulePreview() {
    if (previewTimer) clearTimeout(previewTimer);
    previewTimer = setTimeout(refreshPreview, 250);
  }

  async function refreshPreview() {
    if (!template) return;
    try {
      const url = await invoke('preview_vtemplate_filled', {
        templateId: template.id,
        values: fieldValues,
        brandKitId: selectedBrandKitId || null,
      });
      if (url) livePreview = url;
    } catch {}
  }

  async function pickImageForField(key) {
    const sel = await open({
      multiple: false,
      title: 'Choose Replacement Image',
      filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp', 'bmp'] }],
    });
    if (typeof sel === 'string' && sel) {
      fieldValues = { ...fieldValues, [key]: sel };
      refreshPreview();
    }
  }

  async function handleCreateFromTemplate() {
    if (!template || busy) return;
    busy = true;
    errorMsg = '';
    try {
      const resp = await invoke('create_project_from_vtemplate', {
        templateId: template.id,
        values: fieldValues,
        projectName: projectName.trim() || template.name,
        brandKitId: selectedBrandKitId || null,
      });
      if (onCreatedProject) onCreatedProject(resp);
      onClose();
    } catch (e) {
      errorMsg = String(e);
    } finally {
      busy = false;
    }
  }

  async function handleExportTemplate() {
    if (!template) return;
    const dest = await save({
      title: 'Export .vtemplate File',
      defaultPath: `${template.name.replace(/[^a-z0-9_-]/gi, '_')}.vtemplate`,
      filters: [{ name: 'Veltrax Template', extensions: ['vtemplate'] }],
    });
    if (dest) {
      try {
        await invoke('export_vtemplate_file', { templateId: template.id, destPath: dest });
      } catch (e) {
        errorMsg = String(e);
      }
    }
  }

  async function handleSaveTemplate() {
    if (!tplName.trim() || busy) return;
    busy = true;
    errorMsg = '';
    try {
      const fields = editableRows
        .filter((r) => r.selected)
        .map((r, i) => ({
          id: `f_${i + 1}`,
          label: r.label,
          kind: r.kind,
          layer_id: r.layer_id,
          variable_name: r.variable_name || `var_${i + 1}`,
          default_value: r.default_value,
          image_fit: r.image_fit || 'cover',
        }));
      const saved = await invoke('save_current_as_vtemplate', {
        name: tplName.trim(),
        category: tplCategory,
        fields,
      });
      if (onSavedTemplate) onSavedTemplate(saved);
      onClose();
    } catch (e) {
      errorMsg = String(e);
    } finally {
      busy = false;
    }
  }
</script>

{#if show}
  <div class="backdrop" onclick={onClose} role="presentation">
    <div class="modal" onclick={(e) => e.stopPropagation()} role="dialog" aria-label="Template">
      <div class="modal-header">
        <span class="modal-title">
          <Sparkles size={15} strokeWidth={2} />
          {mode === 'fill' ? (template?.name || 'Use Template') : 'Save as Editable Template'}
        </span>
        <button class="close-btn" onclick={onClose}><X size={16} strokeWidth={2} /></button>
      </div>

      {#if mode === 'fill' && template}
        <div class="fill-body">
          <div class="preview-pane">
            {#if livePreview}
              <img src={livePreview} alt={template.name} />
            {:else}
              <Layers size={40} />
            {/if}
            <div class="dim-badge">{template.canvas_width} × {template.canvas_height} px · {template.category}</div>
            <button class="btn-ghost sm" onclick={handleExportTemplate}>
              <Download size={12} /> Share / Export .vtemplate
            </button>
          </div>

          <div class="form-pane">
            <label class="field">
              <span>Project Name</span>
              <input type="text" bind:value={projectName} placeholder={template.name} />
            </label>

            {#if brandKits.length > 0}
              <label class="field">
                <span>Apply Brand Kit (optional)</span>
                <select
                  class="select-input"
                  value={selectedBrandKitId}
                  onchange={(e) => {
                    selectedBrandKitId = e.currentTarget.value;
                    refreshPreview();
                  }}
                >
                  <option value="">None (Use template defaults)</option>
                  {#each brandKits as k}
                    <option value={k.id}>{k.name}</option>
                  {/each}
                </select>
              </label>
            {/if}

            {#if (template.fields || []).length === 0}
              <p class="note">This template has no variable fields. Open it directly in the full editor.</p>
            {:else}
              {#each template.fields as f}
                {@const key = f.variable_name || f.id}
                <div class="field">
                  <span>{f.label} <code class="var-tag">{`{{${key}}}`}</code></span>
                  {#if f.kind === 'text'}
                    <textarea
                      rows="2"
                      value={fieldValues[key] ?? ''}
                      oninput={(e) => {
                        fieldValues = { ...fieldValues, [key]: e.currentTarget.value };
                        schedulePreview();
                      }}
                    ></textarea>
                  {:else if f.kind === 'color'}
                    <div style="display:flex; gap:8px; align-items:center;">
                      <input
                        type="color"
                        value={fieldValues[key] || '#6366f1'}
                        oninput={(e) => {
                          fieldValues = { ...fieldValues, [key]: e.currentTarget.value };
                          schedulePreview();
                        }}
                        style="width:42px; height:32px; padding:2px;"
                      />
                      <input
                        type="text"
                        value={fieldValues[key] || '#6366f1'}
                        oninput={(e) => {
                          fieldValues = { ...fieldValues, [key]: e.currentTarget.value };
                          schedulePreview();
                        }}
                      />
                    </div>
                  {:else if f.kind === 'image'}
                    <div style="display:flex; gap:8px; align-items:center;">
                      <button class="btn-ghost sm" onclick={() => pickImageForField(key)}>
                        <Upload size={12} /> Choose Image ({f.image_fit})
                      </button>
                      <span class="path-trim">{(fieldValues[key] || '').split(/[/\\]/).pop() || 'Default'}</span>
                    </div>
                  {/if}
                </div>
              {/each}
            {/if}

            {#if errorMsg}
              <p class="err">{errorMsg}</p>
            {/if}
          </div>
        </div>

        <div class="modal-footer">
          <button class="btn-ghost" onclick={onClose} disabled={busy}>Cancel</button>
          <button class="btn-primary" onclick={handleCreateFromTemplate} disabled={busy}>
            {busy ? 'Creating…' : 'Open in Full Editor'}
          </button>
        </div>

      {:else if mode === 'save'}
        <div class="save-body">
          <div class="grid-2">
            <label class="field">
              <span>Template Name</span>
              <input type="text" bind:value={tplName} placeholder="My YouTube Layout" />
            </label>
            <label class="field">
              <span>Category</span>
              <select class="select-input" bind:value={tplCategory}>
                <option value="YouTube">YouTube</option>
                <option value="Social">Social</option>
                <option value="Gaming">Gaming</option>
                <option value="Logo">Logo</option>
                <option value="Custom">Custom</option>
              </select>
            </label>
          </div>

          <div class="sec-title">Editable Fields ({editableRows.filter((r) => r.selected).length} selected)</div>
          <div class="rows-list">
            {#each editableRows as row}
              <div class="edit-row">
                <input type="checkbox" bind:checked={row.selected} />
                <input type="text" bind:value={row.label} placeholder="Field label" style="flex:1;" />
                <select class="select-input" bind:value={row.kind} style="width:90px;">
                  <option value="text">Text</option>
                  <option value="image">Image</option>
                  <option value="color">Color</option>
                </select>
                {#if row.kind === 'image'}
                  <select class="select-input" bind:value={row.image_fit} style="width:90px;">
                    <option value="cover">Cover</option>
                    <option value="contain">Contain</option>
                  </select>
                {/if}
              </div>
            {/each}
          </div>

          {#if errorMsg}
            <p class="err">{errorMsg}</p>
          {/if}
        </div>

        <div class="modal-footer">
          <button class="btn-ghost" onclick={onClose} disabled={busy}>Cancel</button>
          <button class="btn-primary" onclick={handleSaveTemplate} disabled={busy || !tplName.trim()}>
            {busy ? 'Saving…' : 'Save Template'}
          </button>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .backdrop { position:fixed; inset:0; background:rgba(0,0,0,0.68); backdrop-filter:blur(6px); display:grid; place-items:center; z-index:2100; }
  .modal { background:var(--bg-card); border:1px solid var(--border); border-radius:var(--r-xl); width:min(760px,94vw); max-height:88vh; display:flex; flex-direction:column; overflow:hidden; box-shadow:0 32px 80px rgba(0,0,0,0.5); }
  .modal-header { display:flex; align-items:center; justify-content:space-between; padding:16px 20px; border-bottom:1px solid var(--border); }
  .modal-title { display:flex; align-items:center; gap:8px; font-size:15px; font-weight:700; }
  .close-btn { display:grid; place-items:center; width:28px; height:28px; border-radius:var(--r-sm); background:var(--bg-elevated); color:var(--text-secondary); border:1px solid var(--border); }
  .fill-body { display:grid; grid-template-columns:280px 1fr; gap:18px; padding:20px; overflow-y:auto; }
  .preview-pane { display:flex; flex-direction:column; align-items:center; gap:10px; background:var(--bg-elevated); border:1px solid var(--border); border-radius:var(--r-md); padding:14px; }
  .preview-pane img { width:100%; max-height:220px; object-fit:contain; border-radius:6px; background:#090b0e; }
  .dim-badge { font-size:11px; color:var(--text-muted); }
  .form-pane { display:flex; flex-direction:column; gap:12px; }
  .save-body { padding:20px; display:flex; flex-direction:column; gap:14px; overflow-y:auto; }
  .grid-2 { display:grid; grid-template-columns:1fr 1fr; gap:12px; }
  .field { display:flex; flex-direction:column; gap:5px; font-size:12px; color:var(--text-secondary); }
  .var-tag { font-family:var(--mono); font-size:10px; color:#818cf8; margin-left:4px; }
  .path-trim { font-size:11px; color:var(--text-muted); overflow:hidden; text-overflow:ellipsis; white-space:nowrap; max-width:160px; }
  .sec-title { font-size:11px; font-weight:700; text-transform:uppercase; letter-spacing:0.05em; color:var(--text-secondary); }
  .rows-list { display:flex; flex-direction:column; gap:8px; max-height:240px; overflow-y:auto; }
  .edit-row { display:flex; align-items:center; gap:8px; background:var(--bg-elevated); border:1px solid var(--border); border-radius:var(--r-sm); padding:6px 10px; }
  .edit-row input[type="checkbox"] { width:auto; accent-color:var(--accent); }
  .note { font-size:12px; color:var(--text-muted); }
  .err { font-size:12px; color:#f87171; }
  .sm { font-size:11px; padding:5px 10px; display:inline-flex; align-items:center; gap:5px; }
  .modal-footer { display:flex; justify-content:flex-end; gap:10px; padding:14px 20px; border-top:1px solid var(--border); }
</style>
