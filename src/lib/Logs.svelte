<script lang="ts">
  import { logStore } from "./store.svelte";

  let viewport = $state<HTMLDivElement | null>(null);
  let stick = $state(true);

  // Auto-scroll to bottom when new lines arrive, unless the user scrolled up.
  $effect(() => {
    logStore.lines.length;
    if (stick && viewport) {
      queueMicrotask(() => viewport && (viewport.scrollTop = viewport.scrollHeight));
    }
  });

  function onScroll() {
    if (!viewport) return;
    stick = viewport.scrollHeight - viewport.scrollTop - viewport.clientHeight < 40;
  }

  function clear() {
    logStore.lines.length = 0;
  }
</script>

<div class="logs">
  <div class="toolbar">
    <span class="muted">{logStore.lines.length} lines</span>
    <button onclick={clear}>Clear</button>
  </div>
  <div class="viewport" bind:this={viewport} onscroll={onScroll}>
    {#if logStore.lines.length === 0}
      <p class="empty">No output yet. Launch a service to see SSH logs here.</p>
    {/if}
    {#each logStore.lines as l}
      <div class="line">
        <span class="ts">{l.t}</span>
        <span class="tag tag-{l.service}">{l.service}</span>
        <span class="txt">{l.line}</span>
      </div>
    {/each}
  </div>
</div>

<style>
  .logs {
    display: flex;
    flex-direction: column;
    height: 100%;
    gap: 8px;
  }
  .toolbar {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .viewport {
    flex: 1;
    overflow-y: auto;
    background: #0c0d10;
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 10px;
    font-family: ui-monospace, "SF Mono", Menlo, monospace;
    font-size: 0.8rem;
    line-height: 1.5;
  }
  .empty {
    color: var(--muted);
  }
  .line {
    display: flex;
    gap: 8px;
    white-space: pre-wrap;
    word-break: break-word;
  }
  .ts {
    color: #5b6270;
    flex: none;
  }
  .tag {
    flex: none;
    color: #8a90a0;
    min-width: 72px;
  }
  .tag-dashboard {
    color: #6aa3ff;
  }
  .tag-webui {
    color: #c084fc;
  }
  .tag-ssh {
    color: #4ade80;
  }
  .txt {
    color: #d6d8de;
  }
  .muted {
    color: var(--muted);
  }
</style>
