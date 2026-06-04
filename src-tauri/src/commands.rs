use crate::config::{self, AppConfig, ServiceConfig};
use crate::ssh::{self, Client};
use crate::tunnel;
use russh::client::Handle;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_opener::OpenerExt;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

#[derive(Clone, Serialize)]
struct LogLine {
    service: String,
    line: String,
}

#[derive(Clone, Serialize)]
struct StatusUpdate {
    service: String,
    status: String, // "idle" | "starting" | "running" | "error"
    detail: String,
}

#[derive(Default)]
pub struct AppState {
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    handle: Option<Arc<Handle<Client>>>,
    tunnels: HashMap<String, JoinHandle<()>>,
    procs: HashMap<String, JoinHandle<()>>,
    /// Services with a launch in progress (prevents duplicate concurrent launches).
    launching: HashSet<String>,
}

/// Mask `KEY=VALUE` and `--flag VALUE` for sensitive keys so inline secrets in
/// commands or service output are not surfaced/buffered in the UI log.
fn redact(line: &str) -> String {
    const KEYS: [&str; 6] = ["token", "secret", "password", "passwd", "apikey", "auth"];
    let is_sensitive = |k: &str| {
        let norm = k.trim_start_matches('-').to_ascii_lowercase().replace(['_', '-'], "");
        KEYS.iter().any(|s| norm.ends_with(s))
    };
    let mut out: Vec<String> = Vec::new();
    let mut redact_next = false;
    for tok in line.split(' ') {
        if redact_next && !tok.is_empty() {
            out.push("***".into());
            redact_next = false;
            continue;
        }
        if let Some((k, _)) = tok.split_once('=') {
            if is_sensitive(k) {
                out.push(format!("{k}=***"));
                continue;
            }
        }
        if tok.starts_with("--") && is_sensitive(tok) {
            redact_next = true;
        }
        out.push(tok.to_string());
    }
    out.join(" ")
}

/// Build a log-emitting closure bound to a service name.
fn logger(app: &AppHandle, service: &str) -> impl Fn(String) + Clone + Send + Sync + 'static {
    let app = app.clone();
    let service = service.to_string();
    move |line| {
        for part in line.split_inclusive('\n') {
            let clean = redact(part.trim_end());
            let _ = app.emit("log", LogLine { service: service.clone(), line: clean });
        }
    }
}

fn status(app: &AppHandle, service: &str, status: &str, detail: &str) {
    let _ = app.emit(
        "status",
        StatusUpdate {
            service: service.to_string(),
            status: status.to_string(),
            detail: detail.to_string(),
        },
    );
}

fn pick_service<'a>(cfg: &'a AppConfig, name: &str) -> Option<&'a ServiceConfig> {
    match name {
        "dashboard" => Some(&cfg.dashboard),
        "webui" => Some(&cfg.webui),
        _ => None,
    }
}

/// Return a live SSH handle, connecting if necessary. On reconnect (the previous
/// handle was closed), tear down tunnels/procs that referenced the dead handle.
async fn ensure_connected(
    inner: &mut Inner,
    cfg: &AppConfig,
    app: &AppHandle,
) -> Result<Arc<Handle<Client>>, String> {
    if let Some(h) = &inner.handle {
        if !h.is_closed() {
            return Ok(h.clone());
        }
    }
    // First connect or reconnecting: abort anything bound to the old handle.
    for (_, t) in inner.tunnels.drain() {
        t.abort();
    }
    for (_, p) in inner.procs.drain() {
        p.abort();
    }

    let log = logger(app, "ssh");
    log(format!("connecting to {}@{}...", cfg.server.username, cfg.server.host));
    let password = if cfg.auth.mode == "password" {
        config::get_password()
    } else {
        None
    };
    let handle = ssh::connect(&cfg.server, &cfg.auth, password)
        .await
        .map_err(|e| e.to_string())?;
    log("connected".to_string());
    inner.handle = Some(handle.clone());
    Ok(handle)
}

#[tauri::command]
pub fn load_config() -> AppConfig {
    config::load()
}

#[tauri::command]
pub fn save_config(config: AppConfig, password: Option<String>) -> Result<(), String> {
    config::save(&config).map_err(|e| e.to_string())?;
    if let Some(pw) = password {
        if !pw.is_empty() {
            config::set_password(&pw).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn has_password() -> bool {
    config::has_password()
}

#[tauri::command]
pub fn clear_password() -> Result<(), String> {
    config::delete_password().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn test_connection(app: AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    let cfg = config::load();
    let handle = {
        let mut inner = state.inner.lock().await;
        ensure_connected(&mut inner, &cfg, &app).await?
    };
    let code = ssh::exec(&handle, "echo connection-ok", logger(&app, "ssh"))
        .await
        .map_err(|e| e.to_string())?;
    if code == 0 {
        Ok("Connection OK".to_string())
    } else {
        Err(format!("remote echo exited with code {code}"))
    }
}

#[tauri::command]
pub async fn launch_service(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
) -> Result<u16, String> {
    let cfg = config::load();
    let svc = pick_service(&cfg, &name)
        .ok_or_else(|| format!("unknown service '{name}'"))?
        .clone();
    if svc.local_port == 0 || svc.remote_port == 0 {
        return Err(format!("configure ports for '{name}' first"));
    }

    // Claim the launch slot so a second concurrent launch of the SAME service
    // no-ops, while different services proceed in parallel.
    {
        let mut inner = state.inner.lock().await;
        if !inner.launching.insert(name.clone()) {
            return Err(format!("{name} is already launching"));
        }
    }

    let result = launch_inner(&app, &state, &cfg, &svc, &name).await;

    // Always release the slot and emit a terminal status, on every exit path.
    {
        let mut inner = state.inner.lock().await;
        inner.launching.remove(&name);
    }
    match &result {
        Ok(_) => status(&app, &name, "running", "tunnel open"),
        Err(e) => status(&app, &name, "error", e),
    }
    result
}

async fn launch_inner(
    app: &AppHandle,
    state: &State<'_, AppState>,
    cfg: &AppConfig,
    svc: &ServiceConfig,
    name: &str,
) -> Result<u16, String> {
    // Connect (lock held only to read/refresh the shared handle).
    let handle = {
        let mut inner = state.inner.lock().await;
        ensure_connected(&mut inner, cfg, app).await?
    };

    status(app, name, "starting", "checking service");
    let log = logger(app, name);

    // Health check: run start_cmd only if the service is not already up.
    // An exec error is treated as "not up", never aborting the launch.
    let healthy = if svc.health_check_cmd.trim().is_empty() {
        false
    } else {
        log(format!("health check: {}", svc.health_check_cmd));
        match ssh::exec(&handle, &svc.health_check_cmd, logger(app, name)).await {
            Ok(code) => code == 0,
            Err(e) => {
                log(format!("health check error: {e}"));
                false
            }
        }
    };

    if healthy {
        log(format!("{name} already running"));
    } else if !svc.start_cmd.trim().is_empty() {
        log(format!("starting {name}"));
        // Run the start command as a long-lived task; output streams to logs.
        let start_handle = handle.clone();
        let start_cmd = svc.start_cmd.clone();
        let start_log = logger(app, name);
        let err_app = app.clone();
        let err_name = name.to_string();
        let proc = tokio::spawn(async move {
            match ssh::exec(start_handle.as_ref(), &start_cmd, start_log).await {
                Ok(code) if code != 0 => {
                    status(&err_app, &err_name, "error", &format!("start command exited {code}"))
                }
                Ok(_) => {}
                Err(e) => {
                    status(&err_app, &err_name, "error", &format!("start command error: {e}"))
                }
            }
        });
        {
            let mut inner = state.inner.lock().await;
            if let Some(old) = inner.procs.insert(name.to_string(), proc) {
                old.abort();
            }
        }
        // Poll health until up (or give up after ~20s). Errors = "not up yet".
        if !svc.health_check_cmd.trim().is_empty() {
            let mut up = false;
            for _ in 0..20 {
                tokio::time::sleep(Duration::from_secs(1)).await;
                if let Ok(0) = ssh::exec(&handle, &svc.health_check_cmd, |_| {}).await {
                    up = true;
                    break;
                }
            }
            if !up {
                log(format!("warning: {name} health check still failing"));
            }
        }
    } else {
        log(format!("no start command for {name}; assuming reachable"));
    }

    // Decide whether a tunnel needs creating (drop a dead one first).
    let need_tunnel = {
        let mut inner = state.inner.lock().await;
        if inner.tunnels.get(name).map_or(false, |t| t.is_finished()) {
            inner.tunnels.remove(name);
        }
        !inner.tunnels.contains_key(name)
    };

    if !need_tunnel {
        log(format!("tunnel for {name} already open"));
    } else {
        // Bind synchronously so "port in use" surfaces before we declare success.
        let listener = tunnel::bind_listener(svc.local_port).map_err(|e| e.to_string())?;
        let tunnel_handle = handle.clone();
        let remote_host = svc.remote_host.clone();
        let remote_port = svc.remote_port;
        let serve_log = logger(app, name);
        let err_app = app.clone();
        let err_name = name.to_string();
        let task = tokio::spawn(async move {
            if let Err(e) =
                tunnel::serve(listener, tunnel_handle, remote_host, remote_port, serve_log).await
            {
                status(&err_app, &err_name, "error", &format!("tunnel: {e}"));
            }
        });
        let mut inner = state.inner.lock().await;
        inner.tunnels.insert(name.to_string(), task);
    }

    let url = format!("http://localhost:{}", svc.local_port);
    log(format!("opening {url}"));
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())?;

    Ok(svc.local_port)
}

#[tauri::command]
pub async fn stop_service(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
) -> Result<(), String> {
    let mut inner = state.inner.lock().await;
    if let Some(t) = inner.tunnels.remove(&name) {
        t.abort();
    }
    if let Some(p) = inner.procs.remove(&name) {
        p.abort();
    }
    status(&app, &name, "idle", "stopped");
    Ok(())
}

#[tauri::command]
pub async fn disconnect(state: State<'_, AppState>) -> Result<(), String> {
    let mut inner = state.inner.lock().await;
    for (_, t) in inner.tunnels.drain() {
        t.abort();
    }
    for (_, p) in inner.procs.drain() {
        p.abort();
    }
    inner.handle = None;
    Ok(())
}
