<script>
  let {
    ready = false,
    progress = 0,
    statusText = 'Loading…',
    onDone
  } = $props();

  const letters = 'Veltrax'.split('');
  let minElapsed = $state(false);
  let closing = $state(false);
  let skipped = $state(false);
  let waitingSlow = $derived(minElapsed && !ready && !skipped);

  function finish() {
    if (closing) return;
    closing = true;
    setTimeout(() => {
      if (onDone) onDone();
    }, 300);
  }

  function skip() {
    skipped = true;
    finish();
  }

  $effect(() => {
    const reduced =
      typeof window !== 'undefined' &&
      window.matchMedia?.('(prefers-reduced-motion: reduce)')?.matches;
    const minMs = reduced ? 1000 : 2800;
    const t = setTimeout(() => {
      minElapsed = true;
    }, minMs);
    return () => clearTimeout(t);
  });

  $effect(() => {
    if ((minElapsed && ready) || skipped) {
      finish();
    }
  });
</script>

<svelte:window onkeydown={skip} />

<div
  class="splash-stage run"
  class:closing
  onclick={skip}
  role="presentation"
>
  <div class="glow"></div>
  <div class="group">
    <svg class="mark" viewBox="190 280 880 680" aria-hidden="true">
      <path
        pathLength="1"
        d="M250 298 L432 298 Q470 300 492 335 L630 560 L768 335 Q790 300 828 298 L1000 298 Q1050 305 1050 350 L770 785 Q748 812 718 812 L540 812 Q508 812 485 785 L208 372 Q200 330 250 298 Z"
      />
      <path
        class="b"
        pathLength="1"
        d="M265 534 L468 838 Q490 868 525 868 L735 868 Q770 868 790 840 L993 534 Q1015 548 1013 575 L790 918 Q770 943 745 943 L512 943 Q485 943 465 918 L245 585 Q238 555 265 534 Z"
      />
    </svg>
    <div class="word">
      <span class="name" aria-label="Veltrax">
        {#each letters as ch, i}
          <span style="--i:{i}">{ch}</span>
        {/each}
      </span>
      <span class="sub">Studios</span>
    </div>
  </div>

  <div class="bar" class:waiting={waitingSlow}>
    <i style="transform:scaleX({Math.max(0.05, Math.min(1, progress / 100))});"></i>
  </div>
  <div class="status">{waitingSlow ? 'Still starting up…' : statusText}</div>
</div>

<style>
  .splash-stage {
    --bg: #0d0d12;
    --accent: #6366f1;
    --accent-2: #818cf8;
    --soft: #a5b4fc;
    --text: #fff;
    position: fixed;
    inset: 0;
    z-index: 9999;
    display: grid;
    place-items: center;
    background: var(--bg);
    color: var(--text);
    font-family: Inter, 'Segoe UI', system-ui, -apple-system, sans-serif;
    overflow: hidden;
    user-select: none;
    cursor: pointer;
    opacity: 1;
    transition: opacity 0.3s ease;
  }
  .splash-stage.closing {
    opacity: 0;
    pointer-events: none;
  }
  .glow {
    position: absolute;
    width: min(90vw, 820px);
    aspect-ratio: 1;
    border-radius: 50%;
    background: radial-gradient(closest-side, rgba(99, 102, 241, 0.28), rgba(99, 102, 241, 0));
    opacity: 0;
  }
  .group {
    position: relative;
    display: flex;
    align-items: center;
    gap: clamp(14px, 3vw, 30px);
  }
  .mark {
    width: clamp(88px, 20vw, 150px);
    height: auto;
    overflow: visible;
    flex: none;
  }
  .mark path {
    fill: var(--accent);
    fill-opacity: 0;
    stroke: var(--accent-2);
    stroke-width: 5;
    stroke-linejoin: round;
    stroke-opacity: 1;
    stroke-dasharray: 1;
    stroke-dashoffset: 1;
  }
  .word {
    overflow: hidden;
    max-width: 0;
    padding: 10px 0;
    white-space: nowrap;
  }
  .name {
    display: block;
    font-weight: 800;
    font-size: clamp(40px, 10vw, 76px);
    letter-spacing: -0.02em;
    line-height: 1;
  }
  .name span {
    display: inline-block;
    opacity: 0;
    transform: translateY(20px);
    filter: blur(8px);
  }
  .sub {
    display: block;
    margin-top: 10px;
    font-weight: 500;
    font-size: clamp(14px, 3vw, 22px);
    color: var(--soft);
    letter-spacing: 0.6em;
    opacity: 0;
    padding-left: 0.2em;
  }
  .bar {
    position: absolute;
    left: 50%;
    bottom: 12%;
    width: min(260px, 60vw);
    height: 3px;
    margin-left: calc(min(260px, 60vw) / -2);
    background: rgba(255, 255, 255, 0.08);
    border-radius: 3px;
    overflow: hidden;
    opacity: 0;
  }
  .bar i {
    display: block;
    height: 100%;
    width: 100%;
    background: linear-gradient(90deg, var(--accent), var(--soft));
    transform-origin: left;
    transition: transform 0.35s cubic-bezier(0.4, 0, 0.2, 1);
  }
  .bar.waiting i {
    animation: waitpulse 1.1s ease-in-out infinite;
  }
  .status {
    position: absolute;
    left: 0;
    right: 0;
    bottom: calc(12% - 28px);
    text-align: center;
    font-size: 12px;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.45);
    opacity: 0;
  }

  .run .glow { animation: glow 4.2s ease-out forwards; }
  .run .mark path {
    animation:
      draw 1.3s cubic-bezier(0.65, 0, 0.35, 1) 0.35s forwards,
      fillin 0.6s ease 1.5s forwards,
      strokeout 0.5s ease 1.6s forwards;
  }
  .run .mark path.b { animation-delay: 0.65s, 1.65s, 1.75s; }
  .run .mark { animation: pulse 1s ease 1.5s; }
  .run .word { animation: open 1s cubic-bezier(0.22, 1, 0.36, 1) 2s forwards; }
  .run .name span {
    animation: rise 0.7s cubic-bezier(0.22, 1, 0.36, 1) forwards;
    animation-delay: calc(2.1s + var(--i) * 0.055s);
  }
  .run .sub { animation: subin 1s cubic-bezier(0.22, 1, 0.36, 1) 2.6s forwards; }
  .run .bar { animation: fade 0.4s ease 0.4s forwards; }
  .run .status { animation: fade 0.4s ease 0.6s forwards; }

  @keyframes draw { to { stroke-dashoffset: 0; } }
  @keyframes fillin { to { fill-opacity: 1; } }
  @keyframes strokeout { to { stroke-opacity: 0; } }
  @keyframes pulse {
    40% { filter: drop-shadow(0 0 22px rgba(129, 140, 248, 0.9)); }
    100% { filter: drop-shadow(0 0 0 rgba(129, 140, 248, 0)); }
  }
  @keyframes open { to { max-width: 640px; } }
  @keyframes rise { to { opacity: 1; transform: none; filter: none; } }
  @keyframes subin {
    from { letter-spacing: 0.9em; opacity: 0; }
    to { letter-spacing: 0.42em; opacity: 1; }
  }
  @keyframes glow {
    0% { opacity: 0; transform: scale(0.7); }
    50% { opacity: 1; }
    100% { opacity: 0.8; transform: scale(1); }
  }
  @keyframes fade { to { opacity: 1; } }
  @keyframes waitpulse {
    0%, 100% { opacity: 0.6; }
    50% { opacity: 1; }
  }

  @media (prefers-reduced-motion: reduce) {
    .run * {
      animation-duration: 0.01s !important;
      animation-delay: 0s !important;
    }
  }
</style>
