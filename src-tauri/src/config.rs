use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const APP_DIR: &str = "com.hermes.uilauncher";
const CONFIG_FILE: &str = "config.json";
const KEYRING_SERVICE: &str = "com.hermes.uilauncher";
const KEYRING_ACCOUNT: &str = "ssh-password";

fn default_port() -> u16 {
    22
}
fn default_remote_host() -> String {
    "localhost".to_string()
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
}
impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: default_port(),
            username: String::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct AuthConfig {
    /// "agent", "key", or "password"
    pub mode: String,
    /// path to private key (when mode == "key")
    pub key_path: String,
}
impl Default for AuthConfig {
    fn default() -> Self {
        // ssh-agent is the least-surprise default: it matches the system `ssh`
        // client and works with passphrase-protected and hardware-backed keys.
        Self {
            mode: "agent".to_string(),
            key_path: String::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct ServiceConfig {
    pub local_port: u16,
    pub remote_host: String,
    pub remote_port: u16,
    pub start_cmd: String,
    pub health_check_cmd: String,
}
impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            local_port: 0,
            remote_host: default_remote_host(),
            remote_port: 0,
            start_cmd: String::new(),
            health_check_cmd: String::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub auth: AuthConfig,
    pub dashboard: ServiceConfig,
    pub webui: ServiceConfig,
}

fn app_dir() -> Result<PathBuf> {
    Ok(dirs::config_dir().context("no config dir")?.join(APP_DIR))
}

fn config_path() -> Result<PathBuf> {
    Ok(app_dir()?.join(CONFIG_FILE))
}

fn known_hosts_path() -> Result<PathBuf> {
    Ok(app_dir()?.join("known_hosts.json"))
}

/// Restrict a path to owner-only on Unix (no-op elsewhere).
fn restrict(path: &Path, mode: u32) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode));
    }
    #[cfg(not(unix))]
    {
        let _ = (path, mode);
    }
}

pub fn load() -> AppConfig {
    let Ok(path) = config_path() else {
        return AppConfig::default();
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return AppConfig::default();
    };
    match serde_json::from_str(&text) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!(
                "config parse error ({}): {e}; using defaults",
                path.display()
            );
            AppConfig::default()
        }
    }
}

pub fn save(cfg: &AppConfig) -> Result<()> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("create config dir")?;
        restrict(parent, 0o700);
    }
    let json = serde_json::to_string_pretty(cfg)?;
    std::fs::write(&path, json).context("write config")?;
    restrict(&path, 0o600);
    Ok(())
}

/// Look up a pinned SSH host-key fingerprint for `host:port`, if any.
pub fn load_known_host(host: &str, port: u16) -> Option<String> {
    let path = known_hosts_path().ok()?;
    let text = std::fs::read_to_string(path).ok()?;
    let map: HashMap<String, String> = serde_json::from_str(&text).ok()?;
    map.get(&format!("{host}:{port}")).cloned()
}

/// Persist (trust-on-first-use) an SSH host-key fingerprint for `host:port`.
pub fn save_known_host(host: &str, port: u16, fingerprint: &str) -> Result<()> {
    let path = known_hosts_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("create config dir")?;
        restrict(parent, 0o700);
    }
    let mut map: HashMap<String, String> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    map.insert(format!("{host}:{port}"), fingerprint.to_string());
    std::fs::write(&path, serde_json::to_string_pretty(&map)?).context("write known_hosts")?;
    restrict(&path, 0o600);
    Ok(())
}

fn entry() -> Result<keyring::Entry> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT).context("keyring entry")
}

pub fn set_password(password: &str) -> Result<()> {
    entry()?.set_password(password).context("store password")
}

pub fn get_password() -> Option<String> {
    entry().ok().and_then(|e| e.get_password().ok())
}

pub fn has_password() -> bool {
    get_password().is_some()
}

pub fn delete_password() -> Result<()> {
    match entry()?.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e).context("delete password"),
    }
}
