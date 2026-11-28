<script>
  import { invoke } from '@tauri-apps/api/core';
  import { open, save } from '@tauri-apps/plugin-dialog';
  import { X, Download, Upload, Settings, ShieldCheck } from 'lucide-svelte';

  let { show = false, onClose, onDataImported } = $props();

  let settings = $state({
    window_width: 1280,
    window_height: 800,
    last_export_format: 'png',
    last_export_dir: '',
    recent_presets: [],
    show_splash: true,
  });
  let busy = $state(false);
  let feedback = $state('');

  $effect(() => {
    if (show) {
      feedback = '';
      invoke('get_app_settings')
        .then((s) => {
          if (s) settings = s;
        })
        .catch(() => {});
    }
  });

  async function handleSaveSettings() {
    busy = true;
    feedback = '';
    try {
      await invoke('save_app_settings', { settings });
      feedback = 'Ajustes guardados en %APPDATA%\\Veltrax Studios\\settings.json';
    } catch (e) {
      feedback = `Error: ${e}`;
    } finally {
      busy = false;
    }
  }

  async function handleExportZip() {
    busy = true;
    feedback = '';
    try {
      const dest = await save({
        title: 'Exportar respaldo completo de Veltrax Studios',
        defaultPath: 'veltrax-backup.zip',
        filters: [{ name: 'ZIP Archive', extensions: ['zip'] }],
      });
      if (!dest) {
        busy = false;
        return;
      }
      await invoke('export_user_data_zip', { destZip: dest });
      feedback = `Respaldo exportado a ${dest.split(/[/\\]/).pop()}`;
    } catch (e) {
      feedback = `Error al exportar: ${e}`;
    } finally {
      busy = false;
    }
  }

  async function handleImportZip() {
    busy = true;
    feedback = '';
    try {
      const src = await open({
        multiple: false,
        title: 'Importar respaldo (.zip) a Veltrax Studios',
        filters: [{ name: 'ZIP Archive', extensions: ['zip'] }],
      });
      if (typeof src !== 'string' || !src) {
        busy = false;
        return;
      }
      const count = await invoke('import_user_data_zip', { srcZip: src });
      feedback = `Importados ${count} archivos correctamente.`;
      if (onDataImported) onDataImported();
    } catch (e) {
      feedback = `Error al importar: ${e}`;
    } finally {
      busy = false;
    }
  }
</script>

{#if show}
  <div class="backdrop" onclick={onClose} role="presentation">
    <div class="modal" onclick={(e) => e.stopPropagation()} role="dialog" aria-label="Ajustes y Respaldo">
      <div class="modal-header">
        <span class="modal-title"><Settings size={15} strokeWidth={2} /> Ajustes y Respaldo</span>
        <button class="close-btn" onclick={onClose}><X size={16} strokeWidth={2} /></button>
      </div>

      <div class="modal-body">
        <div class="info-box">
          <ShieldCheck size={16} strokeWidth={2} />
          <div>
            <strong>Datos protegidos en %APPDATA%\Veltrax Studios\</strong>
            <p>Tus proyectos, biblioteca, plantillas, fuentes y brand kit sobreviven a actualizaciones y reinstalaciones.</p>
          </div>
        </div>

        <div class="section">
          <div class="sec-title">Respaldo manual (ZIP)</div>
          <p class="sec-desc">Exporta o restaura todos tus proyectos, assets, biblioteca, plantillas, fuentes y ajustes.</p>
          <div class="btn-row">
            <button class="btn-primary fw" onclick={handleExportZip} disabled={busy}>
              <Download size={14} strokeWidth={2} /> Exportar todos mis datos (.zip)
            </button>
            <button class="btn-ghost fw" onclick={handleImportZip} disabled={busy}>
              <Upload size={14} strokeWidth={2} /> Importar respaldo (.zip)
            </button>
          </div>
        </div>

        <div class="section">
          <div class="sec-title">Preferencias persistentes</div>
          <div class="grid-2">
            <label class="field">
              <span>Formato exportación por defecto</span>
              <select class="select-input" bind:value={settings.last_export_format}>
                <option value="png">PNG</option>
                <option value="jpeg">JPEG</option>
                <option value="webp">WebP</option>
                <option value="svg">SVG</option>
                <option value="pdf">PDF</option>
              </select>
            </label>
            <label class="field">
              <span>Carpeta de exportación</span>
              <input type="text" bind:value={settings.last_export_dir} placeholder="Por defecto (última usada)" />
            </label>
          </div>
          <label style="display:flex; align-items:center; gap:8px; font-size:12px; color:var(--text-secondary); cursor:pointer;">
            <input type="checkbox" bind:checked={settings.show_splash} />
            <span>Mostrar animación de inicio al abrir la app</span>
          </label>
          <button class="btn-ghost sm" onclick={handleSaveSettings} disabled={busy}>Guardar ajustes</button>
        </div>

        {#if feedback}
          <p class="feedback">{feedback}</p>
        {/if}
      </div>

      <div class="modal-footer">
        <button class="btn-ghost" onclick={onClose}>Cerrar</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .backdrop { position:fixed; inset:0; background:rgba(0,0,0,0.65); backdrop-filter:blur(6px); display:grid; place-items:center; z-index:2200; }
  .modal { background:var(--bg-card); border:1px solid var(--border); border-radius:var(--r-xl); width:min(520px,94vw); display:flex; flex-direction:column; box-shadow:0 32px 80px rgba(0,0,0,0.5); }
  .modal-header { display:flex; align-items:center; justify-content:space-between; padding:16px 20px; border-bottom:1px solid var(--border); }
  .modal-title { display:flex; align-items:center; gap:8px; font-size:15px; font-weight:700; }
  .close-btn { display:grid; place-items:center; width:28px; height:28px; border-radius:var(--r-sm); background:var(--bg-elevated); color:var(--text-secondary); border:1px solid var(--border); }
  .modal-body { padding:20px; display:flex; flex-direction:column; gap:18px; }
  .info-box { display:flex; gap:10px; align-items:flex-start; background:rgba(56,189,248,0.08); border:1px solid rgba(56,189,248,0.28); border-radius:var(--r-md); padding:12px; color:#7dd3fc; font-size:12px; line-height:1.45; }
  .info-box p { color:var(--text-secondary); margin-top:2px; font-size:11px; }
  .section { display:flex; flex-direction:column; gap:8px; }
  .sec-title { font-size:12px; font-weight:700; text-transform:uppercase; letter-spacing:0.05em; color:var(--text-secondary); }
  .sec-desc { font-size:12px; color:var(--text-muted); }
  .btn-row { display:flex; gap:8px; flex-wrap:wrap; }
  .fw { flex:1; display:flex; align-items:center; justify-content:center; gap:6px; }
  .grid-2 { display:grid; grid-template-columns:1fr 1fr; gap:10px; }
  .field { display:flex; flex-direction:column; gap:4px; font-size:11px; color:var(--text-secondary); }
  .feedback { font-size:12px; color:#34d399; text-align:center; }
  .modal-footer { display:flex; justify-content:flex-end; padding:14px 20px; border-top:1px solid var(--border); }
  .sm { align-self:flex-start; font-size:11px; padding:6px 12px; }
</style>
