<script>
  import { invoke } from '@tauri-apps/api/core';
  import {
    Bot, Cpu, Terminal, Code2, RefreshCw, Plug, Unplug,
    Copy, Check, Zap, FileCode2, ShieldCheck
  } from 'lucide-svelte';

  let { compact = false, mcpIsActive = false } = $props();

  let clients = $state([]);
  let snippets = $state({ exe_path: '', exe_command: '', json_snippet: '' });
  let scanning = $state(false);
  let statusBanner = $state('');
  let copiedKind = $state(''); // 'cmd' | 'json' | ''

  let connectedClient = $derived(clients.find((c) => c.connected) ?? null);
  let hasConnectedLimit = $derived(Boolean(connectedClient));

  const clientIcons = {
    claude: Bot,
    lmstudio: Cpu,
    codex: Terminal,
    cursor: Code2,
  };

  export async function refreshScan() {
    scanning = true;
    try {
      const [scanned, snips] = await Promise.all([
        invoke('scan_desktop_ai_clients'),
        invoke('get_mcp_snippets'),
      ]);
      clients = scanned;
      snippets = snips;
    } catch (e) {
      statusBanner = `Scan error: ${e}`;
    } finally {
      scanning = false;
    }
  }

  $effect(() => {
    refreshScan();
  });

  async function handleConnect(clientId) {
    if (hasConnectedLimit) return;
    try {
      statusBanner = await invoke('connect_ai_client', { clientId });
      await refreshScan();
    } catch (e) {
      statusBanner = `Connection error: ${e}`;
    }
  }

  async function handleReconnect(clientId) {
    try {
      statusBanner = await invoke('connect_ai_client', { clientId });
      await refreshScan();
    } catch (e) {
      statusBanner = `Reconnect error: ${e}`;
    }
  }

  async function handleDisconnect(clientId) {
    try {
      statusBanner = await invoke('disconnect_ai_client', { clientId });
      await refreshScan();
    } catch (e) {
      statusBanner = `Disconnect error: ${e}`;
    }
  }

  async function handleConnectAll() {
    if (hasConnectedLimit) return;
    try {
      const firstInstalled = clients.find((c) => c.installed);
      if (!firstInstalled) {
        statusBanner = 'No supported desktop AI client detected automatically. Use Manual Setup below.';
        return;
      }
      statusBanner = await invoke('connect_ai_client', { clientId: firstInstalled.id });
      await refreshScan();
    } catch (e) {
      statusBanner = `Auto-connect error: ${e}`;
    }
  }

  async function copyText(kind, text) {
    if (!text) return;
    try {
      await navigator.clipboard.writeText(text);
      copiedKind = kind;
      setTimeout(() => {
        if (copiedKind === kind) copiedKind = '';
      }, 2000);
    } catch {
      statusBanner = 'Failed to copy to clipboard.';
    }
  }
</script>

<div class="mcp-wrap" class:compact>
  <div class="mcp-actions-top">
    <button
      class="btn-primary fw"
      onclick={handleConnectAll}
      disabled={scanning || hasConnectedLimit}
      title={hasConnectedLimit ? 'Disconnect active AI client first (1 client limit)' : 'Connect first detected desktop AI'}
    >
      <Zap size={14} strokeWidth={1.75} /> Auto-Connect All Detected
    </button>
    <button class="btn-ghost icon-only" onclick={refreshScan} disabled={scanning} title="Refresh Scan">
      <RefreshCw size={14} strokeWidth={1.75} class={scanning ? 'spin' : ''} />
    </button>
  </div>

  {#if connectedClient}
    {@const ActiveIcon = clientIcons[connectedClient.id] ?? Bot}
    <div class="connected-card">
      <div class="conn-head">
        <div class="conn-icon">
          <ActiveIcon size={18} strokeWidth={1.75} />
        </div>
        <div class="conn-info">
          <div class="conn-title-row">
            <span class="conn-name">{connectedClient.name}</span>
            <span class="badge connected"><ShieldCheck size={11} strokeWidth={2} /> Connected</span>
          </div>
          <span class="conn-path" title={connectedClient.config_path}>{connectedClient.config_path}</span>
        </div>
      </div>
      <div class="conn-footer">
        <span class="gate-pill" class:active={mcpIsActive}>
          Editor Gate: {mcpIsActive ? 'Active' : 'Standby'}
        </span>
        <div class="conn-btns">
          <button class="btn-ghost sm" onclick={() => handleReconnect(connectedClient.id)}>Reconnect</button>
          <button class="btn-ghost sm danger" onclick={() => handleDisconnect(connectedClient.id)}>
            <Unplug size={12} strokeWidth={1.75} /> Disconnect
          </button>
        </div>
      </div>
      <p class="limit-note">Active MCP limit reached (1/1). Disconnect to switch to another AI client.</p>
    </div>
  {/if}

  <div class="clients-list" class:grid={!compact}>
    {#each clients as client}
      {@const ClientIcon = clientIcons[client.id] ?? Bot}
      <div class="client-card" class:is-connected={client.connected}>
        <div class="client-top">
          <div class="client-id-row">
            <div class="client-icon-box">
              <ClientIcon size={16} strokeWidth={1.75} />
            </div>
            <div class="client-meta">
              <span class="client-name">{client.name}</span>
              {#if client.connected}
                <span class="badge connected">Connected</span>
              {:else if client.installed}
                <span class="badge installed">Installed</span>
              {:else}
                <span class="badge missing">Not Detected</span>
              {/if}
            </div>
          </div>
        </div>

        <div class="client-path" title={client.config_path}>{client.config_path}</div>

        <div class="client-actions">
          {#if client.connected}
            <button class="btn-ghost fw sm" onclick={() => handleReconnect(client.id)}>
              <RefreshCw size={12} strokeWidth={1.75} /> Reconnect
            </button>
          {:else if client.installed}
            <button
              class="btn-primary fw sm"
              onclick={() => handleConnect(client.id)}
              disabled={hasConnectedLimit}
            >
              <Plug size={12} strokeWidth={1.75} /> Connect
            </button>
          {:else}
            <button
              class="btn-ghost fw sm"
              onclick={() => handleConnect(client.id)}
              disabled={hasConnectedLimit}
            >
              <Plug size={12} strokeWidth={1.75} /> Force Create Config
            </button>
          {/if}
        </div>
      </div>
    {/each}
  </div>

  <div class="manual-card">
    <div class="manual-head">
      <FileCode2 size={15} strokeWidth={1.75} />
      <span>Other Desktop Apps / Manual Setup</span>
    </div>
    <p class="manual-desc">
      For ChatGPT Desktop UI, Perplexity Desktop, or custom MCP clients, copy the command or JSON snippet below:
    </p>
    <div class="copy-row">
      <button class="btn-ghost fw sm" onclick={() => copyText('cmd', snippets.exe_command)}>
        {#if copiedKind === 'cmd'}
          <Check size={13} strokeWidth={2} /> Copied!
        {:else}
          <Copy size={13} strokeWidth={1.75} /> Copy Executable Command
        {/if}
      </button>
      <button class="btn-ghost fw sm" onclick={() => copyText('json', snippets.json_snippet)}>
        {#if copiedKind === 'json'}
          <Check size={13} strokeWidth={2} /> Copied!
        {:else}
          <Copy size={13} strokeWidth={1.75} /> Copy JSON Snippet
        {/if}
      </button>
    </div>
    {#if snippets.json_snippet}
      <pre class="snippet-pre">{snippets.json_snippet}</pre>
    {/if}
  </div>

  {#if statusBanner}
    <p class="status-banner">{statusBanner}</p>
  {/if}
</div>

<style>
  .mcp-wrap { display:flex; flex-direction:column; gap:14px; }
  .mcp-actions-top { display:flex; gap:8px; align-items:center; }
  .fw { flex:1; display:flex; align-items:center; justify-content:center; gap:6px; }
  .icon-only { width:34px; height:34px; padding:0; display:grid; place-items:center; flex-shrink:0; }

  .connected-card { background:rgba(16,185,129,0.08); border:1.5px solid rgba(16,185,129,0.4); border-radius:var(--r-md); padding:12px; display:flex; flex-direction:column; gap:10px; }
  .conn-head { display:flex; align-items:center; gap:10px; }
  .conn-icon { width:36px; height:36px; border-radius:var(--r-sm); background:rgba(16,185,129,0.18); color:#34d399; display:grid; place-items:center; flex-shrink:0; }
  .conn-info { min-width:0; flex:1; display:flex; flex-direction:column; gap:2px; }
  .conn-title-row { display:flex; align-items:center; gap:8px; flex-wrap:wrap; }
  .conn-name { font-size:13px; font-weight:700; color:var(--text-primary); }
  .conn-path { font-size:11px; color:var(--text-secondary); font-family:var(--mono); overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .conn-footer { display:flex; align-items:center; justify-content:space-between; gap:8px; flex-wrap:wrap; }
  .conn-btns { display:flex; gap:6px; }
  .gate-pill { font-size:11px; padding:2px 8px; border-radius:999px; background:rgba(245,158,11,0.15); color:#fbbf24; }
  .gate-pill.active { background:rgba(16,185,129,0.18); color:#34d399; }
  .limit-note { font-size:11px; color:var(--text-secondary); }

  .clients-list { display:flex; flex-direction:column; gap:10px; }
  .clients-list.grid { display:grid; grid-template-columns:repeat(2,1fr); gap:12px; }
  .client-card { background:var(--bg-elevated); border:1px solid var(--border); border-radius:var(--r-md); padding:12px; display:flex; flex-direction:column; gap:8px; }
  .client-card.is-connected { border-color:rgba(16,185,129,0.45); }
  .client-id-row { display:flex; align-items:center; gap:9px; }
  .client-icon-box { width:32px; height:32px; border-radius:var(--r-sm); background:var(--bg-card); border:1px solid var(--border); display:grid; place-items:center; color:var(--text-primary); flex-shrink:0; }
  .client-meta { display:flex; flex-direction:column; gap:2px; }
  .client-name { font-size:13px; font-weight:600; color:var(--text-primary); }
  .client-path { font-size:10px; font-family:var(--mono); color:var(--text-muted); overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .client-actions { display:flex; gap:6px; margin-top:2px; }

  .badge { display:inline-flex; align-items:center; gap:4px; font-size:10px; font-weight:600; padding:1px 7px; border-radius:999px; width:fit-content; }
  .badge.installed { background:rgba(16,185,129,0.16); color:#34d399; }
  .badge.connected { background:var(--accent-subtle); color:#a5b4fc; }
  .badge.missing { background:rgba(255,255,255,0.06); color:var(--text-muted); }

  .manual-card { background:var(--bg-elevated); border:1px solid var(--border); border-radius:var(--r-md); padding:12px; display:flex; flex-direction:column; gap:8px; }
  .manual-head { display:flex; align-items:center; gap:7px; font-size:12px; font-weight:600; color:var(--text-primary); }
  .manual-desc { font-size:11px; color:var(--text-secondary); line-height:1.45; }
  .copy-row { display:flex; gap:8px; flex-wrap:wrap; }
  .snippet-pre { font-family:var(--mono); font-size:10px; background:var(--bg-canvas); border:1px solid var(--border); border-radius:var(--r-sm); padding:8px; color:#c7d2fe; overflow-x:auto; user-select:text; line-height:1.4; }
  .status-banner { font-size:11px; color:#34d399; background:rgba(16,185,129,0.08); border:1px solid rgba(16,185,129,0.25); padding:8px 10px; border-radius:var(--r-sm); white-space:pre-wrap; }

  .sm { font-size:11px; padding:6px 10px; }
  .danger:hover:not(:disabled) { color:#f87171; border-color:rgba(248,113,113,0.4); }
  :global(.spin) { animation:spin 0.8s linear infinite; }
  @keyframes spin { from { transform:rotate(0deg); } to { transform:rotate(360deg); } }
</style>
