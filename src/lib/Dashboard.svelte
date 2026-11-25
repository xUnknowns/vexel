<script>
  import {
    Plus, Paintbrush2, Home, Bot,
    FolderOpen, Youtube, Instagram, Smartphone, LayoutGrid, Layers
  } from 'lucide-svelte';
  import { convertFileSrc } from '@tauri-apps/api/core';
  import McpPanel from './McpPanel.svelte';

  let {
    recentProjects = [],
    mcpIsActive = false,
    hasProject = false,
    onGoToEditor,
    onShowModal,
    onOpenFile,
    onLoadRecent,
    onCreatePreset
  } = $props();

  let subView = $state('home'); // 'home' | 'mcp'

  const quickPresets = [
    { label: 'Open Image', sub: null, icon: FolderOpen, action: 'open' },
    { label: 'Logo PNG', sub: '1024 × 1024', icon: Layers, action: 'preset', w: 1024, h: 1024, name: 'Transparent Logo', transparent: true },
    { label: 'YouTube', sub: '1280 × 720', icon: Youtube, action: 'preset', w: 1280, h: 720, name: 'YouTube Thumbnail', transparent: false },
    { label: 'Instagram', sub: '1080 × 1080', icon: Instagram, action: 'preset', w: 1080, h: 1080, name: 'Instagram Post', transparent: false },
    { label: 'Story', sub: '1080 × 1920', icon: Smartphone, action: 'preset', w: 1080, h: 1920, name: 'Story', transparent: false },
    { label: 'Custom Size', sub: null, icon: LayoutGrid, action: 'modal' },
  ];

  function handleQuick(item) {
    if (item.action === 'open') onOpenFile();
    else if (item.action === 'modal') onShowModal();
    else if (item.action === 'preset' && onCreatePreset) {
      onCreatePreset(item.w, item.h, item.name, Boolean(item.transparent));
    }
  }
</script>

<div class="dashboard-shell">
  <aside class="g-sidebar">
    <div class="sidebar-logo" title="Veltrax Studios by Crafters Apex">
      <div class="logo-mark">V</div>
    </div>

    <nav class="g-nav">
      <button class="g-nav-btn create" onclick={onShowModal} title="Create new design">
        <span class="g-nav-icon"><Plus size={18} strokeWidth={2} /></span>
        <span class="g-nav-label">Create</span>
      </button>

      <button class="g-nav-btn" onclick={onGoToEditor} title={hasProject ? 'Return to Editor' : 'Open Editor'}>
        <span class="g-nav-icon"><Paintbrush2 size={19} strokeWidth={1.75} /></span>
        <span class="g-nav-label">Editor</span>
      </button>

      <button
        class="g-nav-btn"
        class:active={subView === 'home'}
        onclick={() => (subView = 'home')}
        title="Home"
      >
        <span class="g-nav-icon"><Home size={19} strokeWidth={1.75} /></span>
        <span class="g-nav-label">Home</span>
      </button>

      <button
        class="g-nav-btn"
        class:active={subView === 'mcp'}
        onclick={() => (subView = 'mcp')}
        title="AI (MCP)"
      >
        <span class="g-nav-icon"><Bot size={19} strokeWidth={1.75} /></span>
        <span class="g-nav-label">AI (MCP)</span>
      </button>
    </nav>

    <div class="sidebar-footer">
      <span class="ver-tag" title="Veltrax Studios v1.0">v1.0</span>
      <div class="engine-dot" title="Rust layer engine online"></div>
    </div>
  </aside>

  <main class="d-main">
    {#if subView === 'home'}
      <div class="d-scroll">
        <section class="hero">
          <h1 class="hero-title">What will you create today?</h1>
          <p class="hero-sub">Build YouTube thumbnails, transparent PNG logos, and layered edits in Veltrax Studios.</p>

          <div class="quick-row">
            {#each quickPresets as item}
              <button
                class="quick-btn"
                onclick={() => handleQuick(item)}
                title={item.sub ?? item.label}
              >
                <div class="quick-icon">
                  <svelte:component this={item.icon} size={22} strokeWidth={1.75} />
                </div>
                <span class="quick-label">{item.label}</span>
                {#if item.sub}
                  <span class="quick-sub">{item.sub}</span>
                {/if}
              </button>
            {/each}
          </div>
        </section>

        {#if recentProjects.length > 0}
          <section class="recent">
            <div class="section-head">
              <h2>Continue Editing</h2>
              <span class="count">{recentProjects.length} in session</span>
            </div>

            <div class="recent-grid">
              {#each recentProjects as item}
                <button class="r-card" onclick={() => onLoadRecent(item.path)}>
                  <div class="r-thumb">
                    {#if !item.path.toLowerCase().endsWith('.veltrax')}
                      <img src={`${convertFileSrc(item.path)}?v=1`} alt={item.name} loading="lazy" />
                    {:else}
                      <Layers size={28} strokeWidth={1.5} />
                    {/if}
                  </div>
                  <div class="r-meta">
                    <span class="r-name">{item.name}</span>
                    {#if item.width}
                      <span class="r-dim">{item.width} × {item.height} px</span>
                    {/if}
                  </div>
                </button>
              {/each}
            </div>
          </section>
        {:else}
          <section class="empty-state">
            <div class="empty-icon"><FolderOpen size={36} strokeWidth={1.25} /></div>
            <h3>No recent projects yet</h3>
            <p>Open an image, load a .veltrax project, or create a transparent canvas.</p>
            <button class="btn-primary" onclick={onOpenFile}>Open Image / Project</button>
          </section>
        {/if}
      </div>

    {:else if subView === 'mcp'}
      <div class="mcp-view">
        <div class="mcp-card">
          <div class="mcp-badge">Model Context Protocol</div>
          <h2>Connect AI Assistant (MCP)</h2>
          <p>
            Universal MCP bridge to connect any compatible desktop AI (Claude Desktop, LM Studio, ChatGPT / Codex, Cursor, Perplexity) to Veltrax Studios.
          </p>

          <McpPanel compact={false} {mcpIsActive} />
        </div>
      </div>
    {/if}
  </main>
</div>

<style>
  .dashboard-shell { display:flex; height:100vh; background:var(--bg-canvas); overflow:hidden; }

  .g-sidebar {
    width:72px; flex-shrink:0; background:var(--bg-sidebar);
    border-right:1px solid var(--border);
    display:flex; flex-direction:column; align-items:center; padding:14px 0 20px; gap:4px;
  }
  .sidebar-logo { margin-bottom:18px; }
  .logo-mark {
    width:36px; height:36px; border-radius:10px;
    background:linear-gradient(135deg, var(--accent), #818cf8);
    display:grid; place-items:center; font-weight:800; font-size:18px; color:#fff;
    box-shadow:0 4px 14px var(--accent-glow);
  }
  .g-nav { display:flex; flex-direction:column; align-items:center; gap:2px; width:100%; padding:0 8px; }
  .g-nav-btn {
    display:flex; flex-direction:column; align-items:center; gap:4px; width:100%;
    padding:10px 4px; border-radius:var(--r-sm); background:transparent;
    color:var(--text-secondary); border:1px solid transparent;
  }
  .g-nav-btn:hover:not(:disabled) { background:var(--bg-elevated); color:var(--text-primary); border-color:var(--border); }
  .g-nav-btn.active { background:var(--accent-subtle); color:var(--text-primary); border-color:var(--border-active); }
  .g-nav-btn.create { background:var(--accent); color:#fff; margin-bottom:8px; }
  .g-nav-btn.create:hover:not(:disabled) { background:var(--accent-hover); box-shadow:0 4px 14px var(--accent-glow); border-color:transparent; }
  .g-nav-icon { display:flex; align-items:center; justify-content:center; }
  .g-nav-label { font-size:10px; font-weight:500; letter-spacing:0.01em; }

  .sidebar-footer { margin-top:auto; display:flex; flex-direction:column; align-items:center; gap:6px; }
  .ver-tag { font-size:10px; font-weight:600; color:var(--text-muted); }
  .engine-dot { width:8px; height:8px; border-radius:50%; background:#10b981; box-shadow:0 0 8px rgba(16,185,129,0.6); }

  .d-main { flex:1; min-width:0; overflow:hidden; display:flex; flex-direction:column; }
  .d-scroll { flex:1; overflow-y:auto; padding:40px 48px 60px; display:flex; flex-direction:column; gap:52px; }

  .hero { display:flex; flex-direction:column; gap:20px; }
  .hero-title { font-size:30px; font-weight:700; color:var(--text-primary); letter-spacing:-0.5px; }
  .hero-sub { font-size:14px; color:var(--text-secondary); margin-top:-12px; }

  .quick-row { display:flex; gap:14px; flex-wrap:wrap; }
  .quick-btn {
    display:flex; flex-direction:column; align-items:center; gap:8px;
    background:var(--bg-card); border:1px solid var(--border); border-radius:var(--r-xl);
    padding:20px 18px 14px; min-width:104px; color:var(--text-secondary);
  }
  .quick-btn:hover:not(:disabled) { background:var(--bg-card-hover); border-color:var(--border-active); color:var(--text-primary); box-shadow:0 6px 20px rgba(0,0,0,0.3); }
  .quick-icon { width:48px; height:48px; border-radius:50%; background:var(--bg-elevated); display:grid; place-items:center; }
  .quick-btn:hover .quick-icon { background:var(--accent-subtle); }
  .quick-label { font-size:12px; font-weight:600; color:inherit; text-align:center; }
  .quick-sub { font-size:10px; color:var(--text-muted); text-align:center; }

  .recent { display:flex; flex-direction:column; gap:16px; }
  .section-head { display:flex; align-items:center; gap:10px; }
  .section-head h2 { font-size:17px; font-weight:600; }
  .count { font-size:12px; color:var(--text-muted); }
  .recent-grid { display:grid; grid-template-columns:repeat(auto-fill, minmax(180px, 1fr)); gap:14px; }
  .r-card { display:flex; flex-direction:column; background:var(--bg-card); border:1px solid var(--border); border-radius:var(--r-md); overflow:hidden; color:var(--text-primary); text-align:left; }
  .r-card:hover:not(:disabled) { border-color:var(--border-active); box-shadow:0 8px 24px rgba(0,0,0,0.3); }
  .r-thumb { aspect-ratio:16/10; background:var(--bg-elevated); overflow:hidden; display:flex; align-items:center; justify-content:center; color:var(--text-muted); }
  .r-thumb img { width:100%; height:100%; object-fit:cover; display:block; }
  .r-meta { padding:10px 12px; display:flex; flex-direction:column; gap:3px; }
  .r-name { font-size:13px; font-weight:600; white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }
  .r-dim { font-size:11px; color:var(--text-muted); }

  .empty-state { display:flex; flex-direction:column; align-items:center; justify-content:center; gap:14px; padding:80px 24px; color:var(--text-secondary); text-align:center; flex:1; }
  .empty-icon { opacity:0.3; }
  .empty-state h3 { font-size:17px; color:var(--text-primary); }
  .empty-state p { font-size:13px; max-width:340px; }

  .mcp-view { flex:1; padding:40px 48px; display:flex; align-items:flex-start; overflow-y:auto; }
  .mcp-card { max-width:600px; display:flex; flex-direction:column; gap:18px; }
  .mcp-badge { display:inline-block; font-size:11px; font-weight:600; text-transform:uppercase; letter-spacing:0.06em; color:#a5b4fc; background:var(--accent-subtle); border:1px solid var(--border-active); padding:5px 10px; border-radius:999px; width:fit-content; }
  .mcp-card h2 { font-size:24px; font-weight:700; }
  .mcp-card p { font-size:14px; color:var(--text-secondary); line-height:1.65; }
  .mcp-card strong { color:var(--text-primary); }

  .diag-grid { display:flex; flex-direction:column; gap:10px; background:var(--bg-card); border:1px solid var(--border); border-radius:var(--r-md); padding:16px; }
  .diag-row { display:flex; align-items:center; gap:10px; font-size:13px; color:var(--text-secondary); }
  .status-dot { width:8px; height:8px; border-radius:50%; flex-shrink:0; }
  .status-dot.active { background:#10b981; box-shadow:0 0 8px rgba(16,185,129,0.6); }
  .status-dot.standby { background:#f59e0b; box-shadow:0 0 8px rgba(245,158,11,0.5); }
  .status-dot.off { background:#ef4444; box-shadow:0 0 8px rgba(239,68,68,0.5); }

  .mcp-actions { display:flex; gap:10px; flex-wrap:wrap; }
  .icon-btn { display:flex; align-items:center; gap:7px; }
  .restart-warn { font-size:12px; color:#fbbf24; background:rgba(245,158,11,0.12); border:1px solid rgba(245,158,11,0.35); border-radius:var(--r-sm); padding:10px 12px; }
  .connect-result {
    font-family:var(--mono); font-size:12px; color:#34d399;
    background:var(--bg-card); border:1px solid var(--border);
    border-radius:var(--r-sm); padding:12px; white-space:pre-wrap; line-height:1.5;
  }
</style>
