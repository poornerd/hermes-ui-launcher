<script lang="ts">
  import { configStore, statusStore } from "./store.svelte";
  import { launchService, stopService } from "./ipc";
  import type { ServiceName } from "./types";

  const services: { name: ServiceName; label: string }[] = [
    { name: "dashboard", label: "Hermes Dashboard" },
    { name: "webui", label: "Web-UI" },
  ];

  let busy = $state<Record<string, boolean>>({});
  let errors = $state<Record<string, string>>({});

  async function launch(name: ServiceName) {
    busy[name] = true;
    errors[name] = "";
    try {
      await launchService(name);
    } catch (e) {
      errors[name] = String(e);
      statusStore[name] = { status: "error", detail: String(e) };
    } finally {
      busy[name] = false;
    }
  }

  async function stop(name: ServiceName) {
    busy[name] = true;
    try {
      await stopService(name);
    } catch (e) {
      errors[name] = String(e);
    } finally {
      busy[name] = false;
    }
  }

  function dotClass(name: ServiceName) {
    return statusStore[name]?.status ?? "idle";
  }
</script>

<div class="grid">
  {#each services as svc}
    {@const cfg = configStore.config[svc.name]}
    <div class="card">
      <div class="card-head">
        <span class="dot {dotClass(svc.name)}"></span>
        <h2>{svc.label}</h2>
      </div>
      <p class="muted">
        localhost:{cfg.local_port} → {cfg.remote_host}:{cfg.remote_port}
      </p>
      <p class="status">{statusStore[svc.name]?.detail || statusStore[svc.name]?.status || "idle"}</p>
      {#if errors[svc.name]}
        <p class="err">{errors[svc.name]}</p>
      {/if}
      <div class="actions">
        <button class="primary" disabled={busy[svc.name]} onclick={() => launch(svc.name)}>
          {busy[svc.name] ? "Working…" : "Launch"}
        </button>
        <button disabled={busy[svc.name]} onclick={() => stop(svc.name)}>Stop</button>
      </div>
    </div>
  {/each}
</div>

<style>
  .grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px;
  }
  .card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 18px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .card-head {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  h2 {
    margin: 0;
    font-size: 1.05rem;
  }
  .muted {
    color: var(--muted);
    font-size: 0.85rem;
    margin: 0;
    font-family: ui-monospace, monospace;
  }
  .status {
    margin: 0;
    font-size: 0.85rem;
  }
  .err {
    color: #e5484d;
    font-size: 0.8rem;
    margin: 0;
    word-break: break-word;
  }
  .actions {
    display: flex;
    gap: 8px;
    margin-top: auto;
  }
  .dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--muted);
    flex: none;
  }
  .dot.running {
    background: #30a46c;
    box-shadow: 0 0 8px #30a46c;
  }
  .dot.starting {
    background: #f5a623;
  }
  .dot.error {
    background: #e5484d;
  }
</style>
