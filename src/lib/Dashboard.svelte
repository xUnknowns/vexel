<script>
  import {
    Plus, Paintbrush2, Home, Bot,
    FolderOpen, Youtube, Instagram, Smartphone, LayoutGrid, Plug
  } from 'lucide-svelte';
  import { convertFileSrc, invoke } from '@tauri-apps/api/core';

  let {
    recentProjects = [],
    mcpIsActive    = false,
    onGoToEditor,
    onShowModal,
    onOpenFile,
    onLoadRecent,
    onCreatePreset
  } = $props();

  let subView       = $state('home'); // 'home' | 'mcp'
  let connectResult = $state('');

  const quickPresets = [
    { label: 'Open Image',  sub: null,          icon: FolderOpen, action: 'open' },
    { label: 'YouTube',     sub: '1280 × 720',  icon: Youtube,    action: 'preset', w: 1280, h: 720,  name: 'YouTube Thumbnail' },
    { label: 'Instagram',   sub: '1080 × 1080', icon: Instagram,  action: 'preset', w: 1080, h: 1080, name: 'Instagram Post' },
    { label: 'Story',       sub: '1080 × 1920', icon: Smartphone, action: 'preset', w: 1080, h: 1920, name: 'Story' },
    { label: 'Custom Size', sub: null,          icon: LayoutGrid, action: 'modal' },
  ];

  function handleQuick(item) {
    if (item.action === 'open')        onOpenFile();
    else if (item.action === 'modal')  onShowModal();
    else if (item.action === 'preset' && onCreatePreset) {
      onCreatePreset(item.w, item.h, item.name);
    }
  }

  async function connectClaude() {
    try {
      connectResult = await invoke('connect_claude_desktop');
    } catch (e) {
      connectResult = `Error: ${e}`;
    }
  }
</script>

<div class="dashboard-shell">
  <!-- ── Global Left Sidebar (72px) ─────────────────────────── -->
  <aside class="g-sidebar">
    <div class="sidebar-logo" title="Veltrax Studios by Crafters Apex">
      <div class="logo-mark">V</div>
    </div>

    <nav class="g-nav">
      <button class="g-nav-btn create" onclick={onShowModal} title="Create new design">
        <span class="g-nav-icon"><Plus size={18} strokeWidth={2} /></span>
        <span class="g-nav-label">Create</span>
      </button>

      <button class="g-nav-btn" onclick={onGoToEditor} title="Open Editor">
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
      <div class="engine-dot" title="Rust engine online"></div>
    </div>
  </aside>

  <!-- ── Main Area ───────────────────────────────────────────── -->
  <main class="d-main">
    {#if subView === 'home'}
      <div class="d-scroll">
        <section class="hero">
          <h1 class="hero-title">What will you create today?</h1>
          <p class="hero-sub">Open an image or start from a preset — Veltrax Studios' Rust engine is ready.</p>

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
                    <img src={`${convertFileSrc(item.path)}?v=1`} alt={item.name} loading="lazy" />
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
            <p>Open an image or create a new canvas to get started.</p>
            <button class="btn-primary" onclick={onOpenFile}>Open Image</button>
          </section>
        {/if}
      </div>

    {:else if subView === 'mcp'}
      <div class="mcp-view">
        <div class="mcp-card">
          <div class="mcp-badge">Model Context Protocol</div>
          <h2>Connect AI (MCP)</h2>
          <p>
            Connect Veltrax Studios' Rust image engine directly to <strong>Claude Desktop</strong>.
            Once connected, Claude can inspect your active canvas and apply edits in real time.
          </p>

          <div class="mcp-status">
            <span class="status-dot" class:active={mcpIsActive} class:standby={!mcpIsActive}></span>
            <span>
              {#if mcpIsActive}
                <strong style="color:var(--text-primary)">Active &amp; Ready</strong>
              {:else}
                Standby — Open an image in the Editor to enable AI
              {/if}
            </span>
          </div>

          <div class="mcp-commands">
            <h3>Exposed MCP tools</h3>
            <ul>
              <li><code>inspect_canvas</code></li>
              <li><code>adjust_image</code></li>
              <li><code>apply_filter</code></li>
              <li><code>transform_image</code></li>
              <li><code>crop_image</code></li>
              <li><code>resize_image</code></li>
            </ul>
          </div>

          <div class="mcp-actions">
            <button class="btn-primary icon-btn" onclick={connectClaude}>
              <Plug size={15} strokeWidth={2} /> Connect to Claude Desktop
            </button>
          </div>

          {#if connectResult}
            <pre class="connect-result">{connectResult}</pre>
          {/if}
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

  .sidebar-footer { margin-top:auto; }
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
    padding:20px 18px 14px; min-width:100px; color:var(--text-secondary);
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
  .r-thumb { aspect-ratio:16/10; background:var(--bg-elevated); overflow:hidden; display:flex; align-items:center; justify-content:center; }
  .r-thumb img { width:100%; height:100%; object-fit:cover; display:block; }
  .r-meta { padding:10px 12px; display:flex; flex-direction:column; gap:3px; }
  .r-name { font-size:13px; font-weight:600; white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }
  .r-dim { font-size:11px; color:var(--text-muted); }

  .empty-state { display:flex; flex-direction:column; align-items:center; justify-content:center; gap:14px; padding:80px 24px; color:var(--text-secondary); text-align:center; flex:1; }
  .empty-icon { opacity:0.3; }
  .empty-state h3 { font-size:17px; color:var(--text-primary); }
  .empty-state p { font-size:13px; max-width:300px; }

  .mcp-view { flex:1; padding:40px 48px; display:flex; align-items:flex-start; overflow-y:auto; }
  .mcp-card { max-width:580px; display:flex; flex-direction:column; gap:20px; }
  .mcp-badge { display:inline-block; font-size:11px; font-weight:600; text-transform:uppercase; letter-spacing:0.06em; color:#a5b4fc; background:var(--accent-subtle); border:1px solid var(--border-active); padding:5px 10px; border-radius:999px; width:fit-content; }
  .mcp-card h2 { font-size:24px; font-weight:700; }
  .mcp-card p { font-size:14px; color:var(--text-secondary); line-height:1.65; }
  .mcp-card strong { color:var(--text-primary); }
  .mcp-status { display:flex; align-items:center; gap:9px; font-size:13px; color:var(--text-secondary); }
  .status-dot { width:8px; height:8px; border-radius:50%; flex-shrink:0; }
  .status-dot.active { background:#10b981; box-shadow:0 0 8px rgba(16,185,129,0.6); }
  .status-dot.standby { background:#f59e0b; box-shadow:0 0 8px rgba(245,158,11,0.5); }
  .mcp-commands { display:flex; flex-direction:column; gap:10px; }
  .mcp-commands h3 { font-size:13px; font-weight:600; color:var(--text-secondary); }
  .mcp-commands ul { display:flex; flex-wrap:wrap; gap:8px; list-style:none; }
  .mcp-commands li { background:var(--bg-elevated); border:1px solid var(--border); border-radius:var(--r-sm); padding:5px 10px; }
  code { font-family:var(--mono); font-size:12px; color:#c7d2fe; }
  .mcp-actions { display:flex; gap:10px; }
  .icon-btn { display:flex; align-items:center; gap:7px; }
  .connect-result {
    font-family:var(--mono); font-size:12px; color:#34d399;
    background:var(--bg-card); border:1px solid var(--border);
    border-radius:var(--r-sm); padding:12px; white-space:pre-wrap; line-height:1.5;
  }
</style>
