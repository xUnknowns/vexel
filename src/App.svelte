<script>
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { open } from '@tauri-apps/plugin-dialog';
  import Dashboard from './lib/Dashboard.svelte';
  import CreateModal from './lib/CreateModal.svelte';
  import Editor from './lib/Editor.svelte';

  let view = $state('dashboard'); // 'dashboard' | 'editor'
  let showCreateModal = $state(false);
  let project = $state(null); // ProjectResponse from Rust
  let cacheBuster = $state(0);
  let recentProjects = $state([]);

  let hasProject = $derived(Boolean(project && project.preview_path));
  let mcpIsActive = $derived(view === 'editor' && hasProject);

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

  function fileName(p) {
    return p ? p.split(/[/\\]/).pop() || p : '';
  }

  function addRecent(filePath, w, h, name) {
    recentProjects = [
      {
        path: filePath,
        name: name || fileName(filePath),
        width: w,
        height: h,
        time: new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
      },
      ...recentProjects.filter((r) => r.path !== filePath),
    ].slice(0, 12);
  }

  async function loadImage(filePath) {
    try {
      const lower = filePath.toLowerCase();
      const resp = lower.endsWith('.veltrax')
        ? await invoke('load_project_file', { sourcePath: filePath })
        : await invoke('open_base_image', { imagePath: filePath });

      applyProjectUpdate(resp);
      addRecent(filePath, resp.canvas_width, resp.canvas_height, resp.project_name);
      showCreateModal = false;
      view = 'editor';
    } catch (e) {
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
      showCreateModal = false;
      view = 'editor';
    } catch (e) {
      console.error('createPreset:', e);
    }
  }
</script>

{#if view === 'editor'}
  <Editor
    {project}
    {cacheBuster}
    {mcpIsActive}
    onProjectUpdate={applyProjectUpdate}
    onGoHome={() => (view = 'dashboard')}
  />
{:else}
  <Dashboard
    {recentProjects}
    {mcpIsActive}
    {hasProject}
    onGoToEditor={() => (view = 'editor')}
    onShowModal={() => (showCreateModal = true)}
    onOpenFile={handleOpenFile}
    onLoadRecent={loadImage}
    onCreatePreset={handleCreatePreset}
  />
  <CreateModal
    show={showCreateModal}
    onClose={() => (showCreateModal = false)}
    onOpenFile={handleOpenFile}
    onCreatePreset={handleCreatePreset}
  />
{/if}
