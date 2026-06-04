use crate::ssh::Client;
use anyhow::{Context, Result};
use russh::client::Handle;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpSocket};

/// Bind 127.0.0.1:local_port synchronously, returning the bind error eagerly
/// (e.g. "port already in use") so the caller can surface it before declaring
/// the service launched.
pub fn bind_listener(local_port: u16) -> Result<TcpListener> {
    let addr = SocketAddr::from(([127, 0, 0, 1], local_port));
    let socket = TcpSocket::new_v4().context("create socket")?;
    socket
        .bind(addr)
        .with_context(|| format!("bind 127.0.0.1:{local_port} (port already in use?)"))?;
    socket.listen(1024).context("listen")
}

/// Forward every accepted connection on `listener` to remote_host:remote_port
/// over the SSH handle (direct-tcpip). Runs until the task is aborted. `on_log`
/// reports lifecycle and per-connection errors.
pub async fn serve<F>(
    listener: TcpListener,
    handle: Arc<Handle<Client>>,
    remote_host: String,
    remote_port: u16,
    on_log: F,
) -> Result<()>
where
    F: Fn(String) + Clone + Send + Sync + 'static,
{
    on_log(format!(
        "tunnel listening on 127.0.0.1 -> {remote_host}:{remote_port}"
    ));

    loop {
        let (mut socket, peer) = listener.accept().await?;
        let handle = handle.clone();
        let remote_host = remote_host.clone();
        let log = on_log.clone();
        tokio::spawn(async move {
            match handle
                .channel_open_direct_tcpip(
                    remote_host,
                    remote_port as u32,
                    "127.0.0.1",
                    peer.port() as u32,
                )
                .await
            {
                Ok(channel) => {
                    let mut stream = channel.into_stream();
                    let _ = tokio::io::copy_bidirectional(&mut socket, &mut stream).await;
                }
                Err(e) => log(format!("tunnel connection failed (SSH handle dead?): {e}")),
            }
        });
    }
}
