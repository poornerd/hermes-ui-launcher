import { listen } from "@tauri-apps/api/event";
import { defaultConfig, type AppConfig, type LogEntry, type ServiceStatus } from "./types";

// App-wide reactive state (Svelte 5 universal reactivity). Mutate properties,
// never reassign the exported bindings.
export const ui = $state<{ tab: "launch" | "config" | "logs" }>({ tab: "launch" });

export const configStore = $state<{ config: AppConfig; loaded: boolean }>({
  config: defaultConfig(),
  loaded: false,
});

export const logStore = $state<{ lines: LogEntry[] }>({ lines: [] });

export const statusStore = $state<Record<string, ServiceStatus>>({
  dashboard: { status: "idle", detail: "" },
  webui: { status: "idle", detail: "" },
});

let listenersReady = false;

/** Attach Tauri event listeners once for the lifetime of the app. */
export async function initListeners() {
  if (listenersReady) return;
  listenersReady = true;

  await listen<{ service: string; line: string }>("log", (e) => {
    const t = new Date().toLocaleTimeString();
    logStore.lines.push({ t, service: e.payload.service, line: e.payload.line });
    if (logStore.lines.length > 2000) logStore.lines.splice(0, logStore.lines.length - 2000);
  });

  await listen<{ service: string; status: string; detail: string }>("status", (e) => {
    statusStore[e.payload.service] = {
      status: e.payload.status as ServiceStatus["status"],
      detail: e.payload.detail,
    };
  });
}
