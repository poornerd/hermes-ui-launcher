use crate::config::{self, AuthConfig, ServerConfig};
use anyhow::{anyhow, Result};
use russh::client::{self, AuthResult, Handle};
use russh::keys::ssh_key::HashAlg;
use russh::keys::{load_secret_key, PrivateKeyWithHashAlg};
use std::sync::{Arc, Mutex};

/// SSH client handler implementing trust-on-first-use host-key verification.
pub struct Client {
    /// Pinned fingerprint for this host:port, if one was stored previously.
    expected: Option<String>,
    /// Fingerprint the server actually presented (recorded for persist/diagnostics).
    seen: Arc<Mutex<Option<String>>>,
}

impl client::Handler for Client {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::ssh_key::PublicKey,
    ) -> Result<bool, Self::Error> {
        let fingerprint = server_public_key.fingerprint(HashAlg::Sha256).to_string();
        *self.seen.lock().unwrap() = Some(fingerprint.clone());
        match &self.expected {
            // Pinned and matches → trust.
            Some(pinned) if *pinned == fingerprint => Ok(true),
            // Pinned but changed → reject (russh aborts the connection).
            Some(_) => Ok(false),
            // No pin yet → trust-on-first-use; connect() persists it after auth.
            None => Ok(true),
        }
    }
}

/// Connect and authenticate. Returns a shareable handle.
pub async fn connect(
    server: &ServerConfig,
    auth: &AuthConfig,
    password: Option<String>,
) -> Result<Arc<Handle<Client>>> {
    let config = Arc::new(client::Config::default());
    let expected = config::load_known_host(&server.host, server.port);
    let had_pin = expected.is_some();
    let seen: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let client = Client {
        expected,
        seen: seen.clone(),
    };

    let mut handle =
        match client::connect(config, (server.host.as_str(), server.port), client).await {
            Ok(h) => h,
            Err(e) => {
                // Distinguish a rejected (changed) host key from other connect errors.
                if had_pin {
                    if let Some(fp) = seen.lock().unwrap().clone() {
                        return Err(anyhow!(
                            "host key changed for {}:{} (server now offers {fp}). \
                         If this is expected, remove the entry from known_hosts.json and retry.",
                            server.host,
                            server.port
                        ));
                    }
                }
                return Err(e.into());
            }
        };

    let result: AuthResult = if auth.mode == "password" {
        let pw =
            password.ok_or_else(|| anyhow!("password auth selected but no password stored"))?;
        handle.authenticate_password(&server.username, pw).await?
    } else {
        if auth.key_path.ends_with(".pub") {
            return Err(anyhow!(
                "{} is a public key; select the private key (same path without .pub)",
                auth.key_path
            ));
        }
        let key = load_secret_key(&auth.key_path, None)
            .map_err(|e| anyhow!("load private key {}: {e}", auth.key_path))?;
        let hash = handle.best_supported_rsa_hash().await?.flatten();
        handle
            .authenticate_publickey(
                &server.username,
                PrivateKeyWithHashAlg::new(Arc::new(key), hash),
            )
            .await?
    };

    if !matches!(result, AuthResult::Success) {
        return Err(anyhow!("authentication failed for {}", server.username));
    }

    // Trust-on-first-use: persist the fingerprint now that auth succeeded.
    if !had_pin {
        if let Some(fp) = seen.lock().unwrap().clone() {
            if let Err(e) = config::save_known_host(&server.host, server.port, &fp) {
                eprintln!("warning: could not persist host key: {e}");
            }
        }
    }

    Ok(Arc::new(handle))
}

/// Drain complete `\n`-terminated lines from `buf`, decoding each whole line so
/// multibyte UTF-8 sequences spanning two channel chunks are not corrupted.
fn drain_lines<F: Fn(String)>(buf: &mut Vec<u8>, on_line: &F) {
    while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
        let line: Vec<u8> = buf.drain(..=pos).collect();
        on_line(String::from_utf8_lossy(&line).to_string());
    }
}

/// Run a command to completion, invoking `on_line` for each complete output line.
/// Returns the remote exit code.
pub async fn exec<F>(handle: &Handle<Client>, command: &str, on_line: F) -> Result<u32>
where
    F: Fn(String),
{
    let mut channel = handle.channel_open_session().await?;
    channel.exec(true, command).await?;
    let mut code: u32 = 0;
    let mut buf: Vec<u8> = Vec::new();
    while let Some(msg) = channel.wait().await {
        match msg {
            russh::ChannelMsg::Data { ref data } => {
                buf.extend_from_slice(data);
                drain_lines(&mut buf, &on_line);
            }
            russh::ChannelMsg::ExtendedData { ref data, .. } => {
                buf.extend_from_slice(data);
                drain_lines(&mut buf, &on_line);
            }
            russh::ChannelMsg::ExitStatus { exit_status } => {
                code = exit_status;
            }
            _ => {}
        }
    }
    // Flush any trailing partial line.
    if !buf.is_empty() {
        on_line(String::from_utf8_lossy(&buf).to_string());
    }
    Ok(code)
}
