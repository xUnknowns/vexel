<script>
  import { invoke } from '@tauri-apps/api/core';
  import { tempDir, join } from '@tauri-apps/api/path';
  import { open, save } from '@tauri-apps/plugin-dialog';
  import Dashboard from './lib/Dashboard.svelte';
  import CreateModal from './lib/CreateModal.svelte';
  import Editor from './lib/Editor.svelte';

  // ── Global state ────────────────────────────────────────────────────────
  let view             = $state('dashboard'); // 'dashboard' | 'editor'
  let showCreateModal  = $state(false);
  let history          = $state([]);
  let historyIndex     = $state(-1);
  let recentProjects   = $state([]);
  let projectName      = $state('Untitled Design');
  let imgWidth         = $state(0);
  let imgHeight        = $state(0);

  // ── Derived ─────────────────────────────────────────────────────────────
  let currentPath = $derived(historyIndex >= 0 ? history[historyIndex] : null);
  let canUndo     = $derived(historyIndex > 0);
  let canRedo     = $derived(historyIndex < history.length - 1);

  // ── Helpers ─────────────────────────────────────────────────────────────
  function fileName(p) {
    if (!p) return '';
    return p.split(/[/\\]/).pop() || p;
  }
  function fileExt(p) {
    const n = fileName(p);
    const d = n.lastIndexOf('.');
    return d !== -1 ? n.slice(d + 1).toLowerCase() : 'png';
  }
  async function makeTempPath(source) {
    const dir = await tempDir();
    const ext = ['png','jpg','jpeg','webp','bmp'].includes(fileExt(source)) ? fileExt(source) : 'png';
    return await join(dir, `vexel-${Date.now()}-${Math.random().toString(36).slice(2,8)}.${ext}`);
  }
  async function makeTempPng() {
    const dir = await tempDir();
    return await join(dir, `vexel-canvas-${Date.now()}.png`);
  }

  // ── State mutations ──────────────────────────────────────────────────────
  function pushHistory(path, w, h) {
    history      = [...history.slice(0, historyIndex + 1), path];
    historyIndex = history.length - 1;
    if (w) imgWidth  = w;
    if (h) imgHeight = h;
  }

  function undo() { if (canUndo) historyIndex -= 1; }
  function redo() { if (canRedo) historyIndex += 1; }

  async function loadImage(filePath) {
    try {
      const dims = await invoke('get_image_dimensions', { path: filePath });
      history      = [filePath];
      historyIndex = 0;
      imgWidth     = dims.width;
      imgHeight    = dims.height;
      projectName  = fileName(filePath).replace(/\.[^.]+$/, '');
      addRecent(filePath, dims.width, dims.height);
      showCreateModal = false;
      view = 'editor';
    } catch (e) { console.error('loadImage:', e); }
  }

  function addRecent(filePath, w, h) {
    recentProjects = [
      {
        path: filePath,
        name: fileName(filePath),
        width: w, height: h,
        time: new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
      },
      ...recentProjects.filter(r => r.path !== filePath)
    ].slice(0, 12);
  }

  // ── Handlers passed to children ──────────────────────────────────────────
  async function handleOpenFile() {
    const sel = await open({
      multiple: false,
      title: 'Open Image',
      filters: [{ name: 'Images', extensions: ['png','jpg','jpeg','webp','bmp','gif','tiff'] }]
    });
    if (typeof sel === 'string' && sel) await loadImage(sel);
  }

  async function handleCreatePreset(w, h, name) {
    try {
      const dest = await makeTempPng();
      await invoke('create_blank_canvas', { destPath: dest, width: w, height: h });
      history      = [dest];
      historyIndex = 0;
      imgWidth     = w;
      imgHeight    = h;
      projectName  = name;
      showCreateModal = false;
      view = 'editor';
    } catch (e) { console.error('createPreset:', e); }
  }

  async function handleExport() {
    if (!currentPath) return;
    const ext = fileExt(currentPath);
    const dest = await save({
      title: 'Export Image',
      defaultPath: `${projectName}.${ext}`,
      filters: [
        { name: 'PNG',  extensions: ['png'] },
        { name: 'JPEG', extensions: ['jpg','jpeg'] },
        { name: 'WebP', extensions: ['webp'] }
      ]
    });
    if (!dest) return;
    await invoke('export_file', { sourcePath: currentPath, destPath: dest });
  }
</script>

{#if view === 'editor'}
  <Editor
    {history}
    {historyIndex}
    {currentPath}
    {canUndo}
    {canRedo}
    {imgWidth}
    {imgHeight}
    {projectName}
    {makeTempPath}
    onUndo={undo}
    onRedo={redo}
    onPushHistory={pushHistory}
    onGoHome={() => (view = 'dashboard')}
    onExport={handleExport}
    onProjectNameChange={(n) => (projectName = n)}
  />
{:else}
  <Dashboard
    {recentProjects}
    onGoToEditor={() => (view = 'editor')}
    onShowModal={() => (showCreateModal = true)}
    onOpenFile={handleOpenFile}
    onLoadRecent={loadImage}
  />
  <CreateModal
    show={showCreateModal}
    onClose={() => (showCreateModal = false)}
    onOpenFile={handleOpenFile}
    onCreatePreset={handleCreatePreset}
  />
{/if}
