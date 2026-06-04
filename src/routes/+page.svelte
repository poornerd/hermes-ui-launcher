<script lang="ts">
  import { onMount } from "svelte";
  import { ui, configStore, initListeners } from "$lib/store.svelte";
  import { loadConfig } from "$lib/ipc";
  import Launch from "$lib/Launch.svelte";
  import Config from "$lib/Config.svelte";
  import Logs from "$lib/Logs.svelte";

  const tabs = [
    { id: "launch", label: "Launch" },
    { id: "config", label: "Config" },
    { id: "logs", label: "Logs" },
  ] as const;

  onMount(async () => {
    await initListeners();
    try {
      const c = await loadConfig();
      configStore.config = c;
    } catch {
      // keep defaults
    }
    configStore.loaded = true;
  });
</script>

<div class="app">
  <header>
    <div class="brand">
      <span class="logo">⬡</span>
      <span>Hermes Launcher</span>
    </div>
    <nav>
      {#each tabs as t}
        <button class:active={ui.tab === t.id} onclick={() => (ui.tab = t.id)}>
          {t.label}
        </button>
      {/each}
    </nav>
  </header>

  <main>
    {#if ui.tab === "launch"}
      <Launch />
    {:else if ui.tab === "config"}
      <Config />
    {:else}
      <Logs />
    {/if}
  </main>
</div>

<style>
  :global(:root) {
    --bg: #15171c;
    --surface: #1c1f26;
    --border: #2a2e38;
    --text: #e6e8ed;
    --muted: #8a90a0;
    --accent: #4f8cff;
  }
  :global(body) {
    margin: 0;
    background: var(--bg);
    color: var(--text);
    font-family: Inter, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
    -webkit-font-smoothing: antialiased;
  }
  :global(button) {
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text);
    padding: 8px 14px;
    border-radius: 8px;
    font-size: 0.88rem;
    cursor: pointer;
    font-family: inherit;
    transition: border-color 0.15s, background 0.15s;
  }
  :global(button:hover:not(:disabled)) {
    border-color: var(--accent);
  }
  :global(button:disabled) {
    opacity: 0.5;
    cursor: default;
  }
  :global(button.primary) {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
    font-weight: 500;
  }
  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 18px;
    border-bottom: 1px solid var(--border);
    -webkit-app-region: drag;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 8px;
    font-weight: 600;
    font-size: 0.95rem;
  }
  .logo {
    color: var(--accent);
    font-size: 1.1rem;
  }
  nav {
    display: flex;
    gap: 6px;
    -webkit-app-region: no-drag;
  }
  nav button {
    background: transparent;
    border-color: transparent;
  }
  nav button.active {
    background: var(--surface);
    border-color: var(--border);
    color: var(--accent);
  }
  main {
    flex: 1;
    overflow-y: auto;
    padding: 18px;
  }
</style>
