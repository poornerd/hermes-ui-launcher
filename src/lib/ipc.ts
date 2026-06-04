import { invoke } from "@tauri-apps/api/core";
import type { AppConfig, ServiceName } from "./types";

export const loadConfig = () => invoke<AppConfig>("load_config");

export const saveConfig = (config: AppConfig, password?: string) =>
  invoke<void>("save_config", { config, password });

export const hasPassword = () => invoke<boolean>("has_password");

export const clearPassword = () => invoke<void>("clear_password");

export const testConnection = () => invoke<string>("test_connection");

export const launchService = (name: ServiceName) =>
  invoke<number>("launch_service", { name });

export const stopService = (name: ServiceName) =>
  invoke<void>("stop_service", { name });

export const disconnect = () => invoke<void>("disconnect");
