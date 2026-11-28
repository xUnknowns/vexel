<script>
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { open } from '@tauri-apps/plugin-dialog';
  import Dashboard from './lib/Dashboard.svelte';
  import CreateModal from './lib/CreateModal.svelte';
  import Editor from './lib/Editor.svelte';
  import SettingsModal from './lib/SettingsModal.svelte';
  import TemplateModal from './lib/TemplateModal.svelte';
  import BrandKitModal from './lib/BrandKitModal.svelte';
  import BatchModal from './lib/BatchModal.svelte';
  import Splash from './lib/Splash.svelte';

  let view = $state('dashboard'); // 'dashboard' | 'editor'
  let showCreateModal = $state(false);
  let showSettingsModal = $state(false);
  let showBrandKitModal = $state(false);
  let showBatchModal = $state(false);
  let activeTemplate = $state(null);
  let allTemplates = $state([]);
  let project = $state(null); // ProjectResponse from Rust
  let cacheBuster = $state(0);
  let recentProjects = $state([]);
  let autosaveOffer = $state(null);
  let startupWarning = $state('');
  let appVersion = $state('…');
  let appSettings = $state(null);
  let showWhatsNew = $state(false);
  let feedbackStatus = $state('');

  let showSplash = $state(true);
  let appReady = $state(false);
  let splashProgress = $state(10);
  let splashStatus = $state('Starting Veltrax Studios…');
  let didBoot = false;

  let hasProject = $derived(Boolean(project && project.preview_path));
  let mcpIsActive = $derived(view === 'editor' && hasProject);

  async function refreshTemplates() {
    try {
      allTemplates = await invoke('list_all_templates');
    } catch (e) {
      console.error('list_all_templates:', e);
    }
  }

  async function handleImportVTemplate() {
    const sel = await open({
      multiple: false,
      title: 'Import .vtemplate File',
      filters: [{ name: 'Veltrax Template', extensions: ['vtemplate'] }],
    });
    if (typeof sel === 'string' && sel) {
      try {
        await invoke('import_vtemplate_file', { srcPath: sel });
        await refreshTemplates();
      } catch (e) {
        startupWarning = String(e);
      }
    }
  }

  async function refreshRecents() {
    try {
      const [items, warn] = await invoke('get_recent_projects');
      recentProjects = items || [];
      if (warn && !startupWarning) startupWarning = warn;
    } catch (e) {
      console.error('get_recent_projects:', e);
    }
  }

  async function bootApp() {
    // Show main window once WebView has painted #0d0d12 background
    getCurrentWindow().show().catch(() => {});

    splashStatus = 'Loading settings…';
    splashProgress = 25;
    try {
      const s = await invoke('get_app_settings');
      appSettings = s;
      if (s?.corrupt_warning) startupWarning = s.corrupt_warning;
      if (s && s.show_splash === false) showSplash = false;
    } catch {}
    try {
      appVersion = await invoke('get_app_version');
      showWhatsNew = appSettings?.last_seen_version !== appVersion;
    } catch {}

    splashStatus = 'Loading recent projects…';
    splashProgress = 55;
    await Promise.all([refreshRecents(), refreshTemplates()]);

    splashStatus = 'Checking library & recovery…';
    splashProgress = 80;
    try {
      const meta = await invoke('check_autosave');
      if (meta) autosaveOffer = meta;
    } catch {}
    invoke('get_user_library').catch(() => {});

    // Non-blocking MCP scan
    splashStatus = 'Ready';
    splashProgress = 100;
    appReady = true;
    invoke('scan_desktop_ai_clients').catch(() => {});
  }

  $effect(() => {
    if (!didBoot) {
      didBoot = true;
      bootApp();
    }
  });

  $effect(() => {
    invoke('set_editor_active', { isInEditor: view === 'editor' }).catch(console.error);
  });

  $effect(() => {
    const onWebviewError = (event) => {
      invoke('record_local_error', {
        source: 'webview',
        errorMessage: String(event.message || 'Uncaught WebView error'),
      }).catch(() => {});
    };
    const onUnhandledRejection = (event) => {
      invoke('record_local_error', {
        source: 'unhandled_promise',
        errorMessage: String(event.reason ?? 'Unhandled promise rejection'),
      }).catch(() => {});
    };
    window.addEventListener('error', onWebviewError);
    window.addEventListener('unhandledrejection', onUnhandledRejection);
    return () => {
      window.removeEventListener('error', onWebviewError);
      window.removeEventListener('unhandledrejection', onUnhandledRejection);
    };
  });

  $effect(() => {
    const unlisten = listen('mcp-project-updated', (event) => {
      project = event.payload;
      cacheBuster += 1;
    });
    const unlistenCreated = listen('mcp-project-created', (event) => {
      project = event.payload;
      cacheBuster += 1;
      showCreateModal = false;
      view = 'editor';
    });
    return () => {
      unlisten.then((fn) => fn());
      unlistenCreated.then((fn) => fn());
    };
  });

  function applyProjectUpdate(resp) {
    project = resp;
    cacheBuster += 1;
  }

  async function dismissWhatsNew() {
    showWhatsNew = false;
    if (!appSettings || !appVersion) return;
    appSettings = { ...appSettings, last_seen_version: appVersion };
    try {
      await invoke('save_app_settings', { settings: appSettings });
    } catch (e) {
      startupWarning = `No se pudo guardar la versión vista en Novedades: ${e}`;
    }
  }

  async function sendFeedback() {
    feedbackStatus = '';
    try {
      const diagnostics = await invoke('get_feedback_diagnostics', {
        screenWidth: window.screen?.width ?? null,
        screenHeight: window.screen?.height ?? null,
      });
      try {
        await navigator.clipboard.writeText(diagnostics);
      } catch {
        const temporaryField = document.createElement('textarea');
        temporaryField.value = diagnostics;
        temporaryField.setAttribute('readonly', '');
        temporaryField.style.position = 'fixed';
        temporaryField.style.opacity = '0';
        document.body.appendChild(temporaryField);
        temporaryField.select();
        const copied = document.execCommand('copy');
        temporaryField.remove();
        if (!copied) throw new Error('El portapapeles rechazó la copia del diagnóstico.');
      }
      const message = await invoke('open_feedback_site');
      feedbackStatus = `${message} El diagnóstico quedó en el portapapeles y no se envió.`;
    } catch (e) {
      feedbackStatus = `No se pudo completar feedback: ${e}`;
    }
  }

  async function loadImage(filePath) {
    try {
      const lower = filePath.toLowerCase();
      const resp = lower.endsWith('.veltrax')
        ? await invoke('load_project_file', { sourcePath: filePath })
        : await invoke('open_base_image', { imagePath: filePath });

      applyProjectUpdate(resp);
      await refreshRecents();
      showCreateModal = false;
      view = 'editor';
    } catch (e) {
      startupWarning = String(e);
      console.error('loadImage:', e);
    }
  }

  async function handleOpenFile() {
    const sel = await open({
      multiple: false,
      title: 'Open Image or Veltrax Project',
      filters: [
        { name: 'Images & Projects', extensions: ['png', 'jpg', 'jpeg', 'webp', 'bmp', 'gif', 'tiff', 'veltrax'] },
      ],
    });
    if (typeof sel === 'string' && sel) await loadImage(sel);
  }

  async function handleCreatePreset(w, h, name, transparentBg = false) {
    try {
      const resp = await invoke('create_project', {
        width: w,
        height: h,
        transparentBg,
        projectName: name,
      });
      applyProjectUpdate(resp);
      await refreshRecents();
      showCreateModal = false;
      view = 'editor';
    } catch (e) {
      console.error('createPreset:', e);
    }
  }

  async function handleRecoverAutosave() {
    try {
      const resp = await invoke('recover_autosave');
      applyProjectUpdate(resp);
      autosaveOffer = null;
      view = 'editor';
    } catch (e) {
      startupWarning = `No se pudo recuperar el autoguardado: ${e}`;
    }
  }

  async function handleDismissAutosave() {
    await invoke('clear_autosave').catch(() => {});
    autosaveOffer = null;
  }

  async function handleRemoveRecent(path) {
    await invoke('remove_recent_project', { path }).catch(console.error);
    await refreshRecents();
  }

  async function handleRelinkRecent(oldPath) {
    const sel = await open({
      multiple: false,
      title: 'Localizar archivo .veltrax',
      filters: [{ name: 'Veltrax Project', extensions: ['veltrax'] }],
    });
    if (typeof sel === 'string' && sel) {
      await invoke('relink_recent_project', { oldPath, newPath: sel }).catch((e) => {
        startupWarning = String(e);
      });
      await refreshRecents();
    }
  }
</script>

{#if showSplash}
  <Splash
    ready={appReady}
    progress={splashProgress}
    statusText={splashStatus}
    onDone={() => (showSplash = false)}
  />
{/if}

{#if view === 'editor'}
  <Editor
    {project}
    {cacheBuster}
    {mcpIsActive}
    onProjectUpdate={applyProjectUpdate}
    onGoHome={() => {
      refreshRecents();
      view = 'dashboard';
    }}
  />
{:else}
  <Dashboard
    {recentProjects}
    templates={allTemplates}
    {mcpIsActive}
    {hasProject}
    {autosaveOffer}
    {startupWarning}
    {appVersion}
    {feedbackStatus}
    onSelectTemplate={(tpl) => (activeTemplate = tpl)}
    onImportTemplate={handleImportVTemplate}
    onRecoverAutosave={handleRecoverAutosave}
    onDismissAutosave={handleDismissAutosave}
    onDismissWarning={() => (startupWarning = '')}
    onRemoveRecent={handleRemoveRecent}
    onRelinkRecent={handleRelinkRecent}
    onOpenSettings={() => (showSettingsModal = true)}
    onOpenBrandKit={() => (showBrandKitModal = true)}
    onOpenBatch={() => (showBatchModal = true)}
    onGoToEditor={() => (view = 'editor')}
    onShowModal={() => (showCreateModal = true)}
    onOpenFile={handleOpenFile}
    onLoadRecent={loadImage}
    onCreatePreset={handleCreatePreset}
    onSendFeedback={sendFeedback}
  />
  <CreateModal
    show={showCreateModal}
    templates={allTemplates}
    onSelectTemplate={(tpl) => {
      showCreateModal = false;
      activeTemplate = tpl;
    }}
    onClose={() => (showCreateModal = false)}
    onOpenFile={handleOpenFile}
    onCreatePreset={handleCreatePreset}
  />
  <TemplateModal
    show={Boolean(activeTemplate)}
    mode="fill"
    template={activeTemplate}
    onClose={() => (activeTemplate = null)}
    onCreatedProject={(resp) => {
      applyProjectUpdate(resp);
      refreshRecents();
      view = 'editor';
    }}
  />
  <SettingsModal
    show={showSettingsModal}
    onClose={() => (showSettingsModal = false)}
    onDataImported={refreshRecents}
  />
  <BrandKitModal
    show={showBrandKitModal}
    availableFonts={[
      { id: 'bold_display', label: 'Bold Display' },
      { id: 'modern_sans', label: 'Modern Sans' },
      { id: 'impact_condensed', label: 'Impact Condensed' },
      { id: 'tech_mono', label: 'Tech Mono' },
      { id: 'elegant_serif', label: 'Elegant Serif' },
    ]}
    onClose={() => (showBrandKitModal = false)}
  />
  <BatchModal
    show={showBatchModal}
    templates={allTemplates}
    onClose={() => (showBatchModal = false)}
  />
{/if}

{#if showWhatsNew}
  <div class="whats-new-backdrop" role="presentation">
    <section class="whats-new-card" role="dialog" aria-modal="true" aria-labelledby="whats-new-title" tabindex="-1">
      <div class="whats-new-label">Veltrax Studios · Beta v{appVersion}</div>
      <h2 id="whats-new-title">Novedades</h2>
      <ul>
        <li>Preparación para beta pública: diagnóstico local con datos técnicos redactados.</li>
        <li>Registro rotativo y recuperación local del estado ante pánicos de Rust.</li>
        <li>La descarga del modelo para quitar fondos solo comienza cuando solicitas esa acción.</li>
      </ul>
      <p>No se envían diagnósticos ni imágenes automáticamente.</p>
      <button class="whats-new-button" onclick={dismissWhatsNew}>Entendido</button>
    </section>
  </div>
{/if}

<style>
  .whats-new-backdrop { position:fixed; inset:0; z-index:4000; display:grid; place-items:center; padding:24px; background:rgba(0,0,0,.72); backdrop-filter:blur(5px); }
  .whats-new-card { width:min(480px,96vw); padding:24px; border:1px solid var(--border-active); border-radius:var(--r-xl); background:var(--bg-card); color:var(--text-primary); box-shadow:0 28px 80px rgba(0,0,0,.55); }
  .whats-new-label { color:#7dd3fc; font-size:11px; font-weight:700; letter-spacing:.06em; text-transform:uppercase; }
  .whats-new-card h2 { margin:10px 0 14px; font-size:23px; }
  .whats-new-card ul { padding-left:20px; color:var(--text-secondary); font-size:13px; line-height:1.7; }
  .whats-new-card p { margin:14px 0; color:var(--text-muted); font-size:12px; }
  .whats-new-button { float:right; padding:9px 16px; border:0; border-radius:var(--r-sm); background:var(--accent); color:white; font-weight:700; cursor:pointer; }
</style>
