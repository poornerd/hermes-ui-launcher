// Mirrors the Rust AppConfig (serde snake_case keys).
export interface ServerConfig {
  host: string;
  port: number;
  username: string;
}

export interface AuthConfig {
  mode: "key" | "password";
  key_path: string;
}

export interface ServiceConfig {
  local_port: number;
  remote_host: string;
  remote_port: number;
  start_cmd: string;
  health_check_cmd: string;
}

export interface AppConfig {
  server: ServerConfig;
  auth: AuthConfig;
  dashboard: ServiceConfig;
  webui: ServiceConfig;
}

export function defaultService(localPort: number, remotePort: number): ServiceConfig {
  return {
    local_port: localPort,
    remote_host: "localhost",
    remote_port: remotePort,
    start_cmd: "",
    health_check_cmd: `nc -z localhost ${remotePort}`,
  };
}

export function defaultConfig(): AppConfig {
  return {
    server: { host: "", port: 22, username: "" },
    auth: { mode: "key", key_path: "" },
    dashboard: defaultService(8080, 8080),
    webui: defaultService(3000, 3000),
  };
}

export type ServiceName = "dashboard" | "webui";

export interface LogEntry {
  t: string;
  service: string;
  line: string;
}

export interface ServiceStatus {
  status: "idle" | "starting" | "running" | "error";
  detail: string;
}
