# Hermes Launcher

Small cross-platform desktop app (macOS / Windows / Linux) that launches the
**Hermes Dashboard** and **Web-UI** running on a remote server. It SSHes to the
server, makes sure each service is up, opens a local port-forward tunnel, and
opens your browser at `localhost:<port>`.

Built with **Tauri 2** (Rust backend + OS-native webview) and **SvelteKit**.
Installer is a few MB — no bundled browser.

## How it works

1. **Config tab** — set the server host/port/user, auth (SSH agent, key, or
   password), and for each service: local port, remote host/port, a start
   command, and a health-check command.
2. **Launch tab** — click *Launch*. The app connects over SSH, runs the
   health-check, runs the start command **only if the service is down**, opens
   the tunnel, and launches the browser.
3. **Logs tab** — live stream of SSH/remote command output.

**Authentication** defaults to **SSH agent** — it uses keys already loaded in
your `ssh-agent` (`ssh-add -l` to list, `ssh-add` to load one), the same way the
system `ssh` client works. This supports passphrase-protected keys and
hardware-backed keys (YubiKey/FIDO, macOS Secure Enclave) that never expose
private bytes on disk. A private-key file path and password auth are also
available.

Passwords are stored in the OS keychain (Keychain / Credential Manager /
libsecret), never on disk. Non-secret config lives in the OS config dir under
`com.hermes.uilauncher/config.json`.

## Develop

```bash
npm install
npm run tauri dev      # hot-reload dev window
```

## Build a release installer

```bash
npm run tauri build    # outputs to src-tauri/target/release/bundle/
```

Requires the Rust toolchain (`rustup`) and Node. On Linux you also need
`libwebkit2gtk` / `libsecret` dev packages.

## Layout

```
src/                     SvelteKit frontend
  routes/+page.svelte    tab shell + global styles
  lib/Launch.svelte      launch buttons + status
  lib/Config.svelte      settings form
  lib/Logs.svelte        live log view
  lib/store.svelte.ts    shared reactive state + event listeners
  lib/ipc.ts             typed wrappers over Tauri commands
src-tauri/src/
  ssh.rs                 russh connect + streaming exec
  tunnel.rs              local port-forward (direct-tcpip)
  config.rs              config file + keychain
  commands.rs            Tauri commands + launch orchestration
```
