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
      if (s?.corrupt_warning) startupWarning = s.corrupt_warning;
      if (s && s.show_splash === false) showSplash = false;
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
    const unlisten = listen('mcp-project-updated', (event) => {
      project = event.payload;
      cacheBuster += 1;
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  });

  function applyProjectUpdate(resp) {
    project = resp;
    cacheBuster += 1;
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
