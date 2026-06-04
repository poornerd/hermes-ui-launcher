<script lang="ts">
  import { configStore } from "./store.svelte";
  import { saveConfig, testConnection, hasPassword, clearPassword } from "./ipc";
  import type { ServiceName } from "./types";
  import { open } from "@tauri-apps/plugin-dialog";
  import { homeDir, join } from "@tauri-apps/api/path";

  async function browseKey() {
    let defaultPath: string | undefined;
    try {
      defaultPath = await join(await homeDir(), ".ssh");
    } catch {
      defaultPath = undefined;
    }
    const picked = await open({
      multiple: false,
      directory: false,
      defaultPath,
      title: "Select SSH private key",
    });
    if (typeof picked === "string") {
      // The private key is what's needed; if the user picked the .pub, use its private counterpart.
      configStore.config.auth.key_path = picked.replace(/\.pub$/, "");
    }
  }

  let password = $state("");
  let pwStored = $state(false);
  let savedMsg = $state("");
  let testMsg = $state("");
  let testing = $state(false);

  $effect(() => {
    hasPassword().then((v) => (pwStored = v));
  });

  const cfg = $derived(configStore.config);

  async function save() {
    savedMsg = "";
    try {
      await saveConfig($state.snapshot(configStore.config), password || undefined);
      // Switching to key auth removes any stored password from the keychain.
      if (configStore.config.auth.mode === "key" && pwStored) {
        await clearPassword();
        pwStored = false;
      }
      savedMsg = "Saved";
      if (password) {
        pwStored = true;
        password = "";
      }
      setTimeout(() => (savedMsg = ""), 2000);
    } catch (e) {
      savedMsg = "Error: " + String(e);
    }
  }

  async function clearStoredPassword() {
    try {
      await clearPassword();
      pwStored = false;
      password = "";
    } catch (e) {
      savedMsg = "Error: " + String(e);
    }
  }

  async function test() {
    testing = true;
    testMsg = "";
    try {
      await saveConfig($state.snapshot(configStore.config), password || undefined);
      testMsg = await testConnection();
    } catch (e) {
      testMsg = "Failed: " + String(e);
    } finally {
      testing = false;
    }
  }

  const serviceLabels: Record<ServiceName, string> = {
    dashboard: "Dashboard",
    webui: "Web-UI",
  };
</script>

<div class="form">
  <section>
    <h3>Server</h3>
    <div class="row">
      <label>Host<input bind:value={cfg.server.host} placeholder="example.com" /></label>
      <label class="narrow">Port<input type="number" bind:value={cfg.server.port} /></label>
      <label>Username<input bind:value={cfg.server.username} placeholder="ubuntu" /></label>
    </div>
  </section>

  <section>
    <h3>Authentication</h3>
    <div class="row">
      <label class="narrow">
        Method
        <select bind:value={cfg.auth.mode}>
          <option value="key">SSH key</option>
          <option value="password">Password</option>
        </select>
      </label>
      {#if cfg.auth.mode === "key"}
        <label class="grow">
          Private key path
          <span class="pick">
            <input bind:value={cfg.auth.key_path} placeholder="~/.ssh/id_ed25519" />
            <button type="button" class="browse" onclick={browseKey}>Browse…</button>
          </span>
        </label>
      {:else}
        <label class="grow">
          Password {#if pwStored}<span class="muted">(saved — leave blank to keep)</span>{/if}
          <span class="pick">
            <input type="password" bind:value={password} placeholder="••••••••" />
            {#if pwStored}
              <button type="button" class="browse" onclick={clearStoredPassword}>Clear</button>
            {/if}
          </span>
        </label>
      {/if}
    </div>
  </section>

  {#each ["dashboard", "webui"] as const as name}
    <section>
      <h3>{serviceLabels[name]}</h3>
      <div class="row">
        <label class="narrow">Local port<input type="number" bind:value={cfg[name].local_port} /></label>
        <label class="narrow">Remote host<input bind:value={cfg[name].remote_host} /></label>
        <label class="narrow">Remote port<input type="number" bind:value={cfg[name].remote_port} /></label>
      </div>
      <label>Start command<input bind:value={cfg[name].start_cmd} placeholder="cd ~/hermes && ./start-{name}.sh" /></label>
      <label>Health check (exit 0 = up)<input bind:value={cfg[name].health_check_cmd} /></label>
    </section>
  {/each}

  <div class="bar">
    <button class="primary" onclick={save}>Save</button>
    <button onclick={test} disabled={testing}>{testing ? "Testing…" : "Test connection"}</button>
    {#if savedMsg}<span class="ok">{savedMsg}</span>{/if}
    {#if testMsg}<span class="ok">{testMsg}</span>{/if}
  </div>
</div>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: 18px;
  }
  section {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 14px 16px;
  }
  h3 {
    margin: 0 0 10px;
    font-size: 0.95rem;
  }
  .row {
    display: flex;
    gap: 12px;
    flex-wrap: wrap;
    margin-bottom: 10px;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 0.78rem;
    color: var(--muted);
    flex: 1;
    min-width: 120px;
  }
  label.narrow {
    flex: 0 0 130px;
  }
  label.grow {
    flex: 2;
  }
  .pick {
    display: flex;
    gap: 8px;
  }
  .pick input {
    flex: 1;
  }
  .browse {
    flex: none;
    white-space: nowrap;
  }
  input,
  select {
    padding: 8px 10px;
    border-radius: 8px;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text);
    font-size: 0.9rem;
    font-family: inherit;
  }
  input:focus,
  select:focus {
    outline: none;
    border-color: var(--accent);
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .ok {
    color: #30a46c;
    font-size: 0.85rem;
  }
  .muted {
    color: var(--muted);
    font-weight: 400;
  }
</style>
