# Hermes Launcher — Future Ideas / Backlog

Detailed notes for work deferred from the initial build. Each item is scoped
enough to pick up cold. Paths are relative to the repo root.

---

## 1. Cross-platform CI build matrix (GitHub Actions)

### Why
Confirm the app actually builds and bundles on macOS, Windows, and Linux (it's
only been built on macOS so far), and produce downloadable installers on tag.
Tauri's pure-Rust SSH stack (russh) should cross-compile cleanly, but Linux
needs system webkit/secret-service dev packages that must be installed in CI.

### What to build
`.github/workflows/build.yml`, using the official `tauri-apps/tauri-action`.

**Matrix:**
```yaml
strategy:
  fail-fast: false
  matrix:
    include:
      - { platform: macos-latest,   args: "--target aarch64-apple-darwin" }
      - { platform: macos-latest,   args: "--target x86_64-apple-darwin" }
      - { platform: ubuntu-22.04,   args: "" }
      - { platform: windows-latest, args: "" }
```

**Per-job steps:**
1. `actions/checkout`.
2. `actions/setup-node` (Node 22) + `npm ci`.
3. `dtolnay/rust-toolchain@stable`; on macOS add the two darwin targets.
4. **Linux only** — install system deps before building:
   ```
   sudo apt-get update
   sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev \
     librsvg2-dev patchelf libsecret-1-dev
   ```
   (`libsecret-1-dev` is required for the `keyring` crate's secret-service backend.)
5. `swatinem/rust-cache` keyed on `src-tauri` for faster rebuilds.
6. `tauri-apps/tauri-action` with `projectPath` defaulting to repo root.

**Two triggers / modes:**
- **PR / push to master** → build only (no release), as a compile gate. Set
  tauri-action to build without publishing.
- **Tag `v*`** → build + create a GitHub Release with installers attached
  (tauri-action `tagName`/`releaseName`, `GITHUB_TOKEN`).

**Also worth adding (cheap, same workflow or a `check.yml`):**
- `cargo fmt --check` and `cargo clippy -- -D warnings` on `src-tauri`.
- `npm run check` (svelte-check) as a frontend gate.
- `cargo audit` (RustSec advisories) — the security review flagged this as a
  release gate; install via `taiki-e/install-action@cargo-audit`.

### Code-signing (later, optional)
Unsigned installers warn on macOS/Windows. When ready:
- macOS: Apple Developer cert + notarization secrets
  (`APPLE_CERTIFICATE`, `APPLE_ID`, `APPLE_PASSWORD`, team id) fed to tauri-action.
- Windows: Authenticode cert (or `azure-trusted-signing`).
Skip for internal use; add before public distribution.

### Acceptance / verification
- Green build on all four matrix legs for a PR.
- Push a `v0.1.0` tag → Release appears with `.dmg`/`.app.tar.gz` (mac),
  `.msi`/`.exe` (win), `.deb`/`.AppImage` (linux).
- Linux job fails loudly if `libsecret`/webkit deps are missing (confirms the
  dependency list is complete).

### Effort
~1–2 hours for the build+gate workflow. Signing/notarization is a separate
half-day once certs exist.

### Reference
- Tauri CI guide: https://v2.tauri.app/distribute/pipelines/github/
- `tauri-apps/tauri-action`: https://github.com/tauri-apps/tauri-action

---

## 2. Interactive host-key confirmation

### Why
Host-key verification today is silent trust-on-first-use. In
`src-tauri/src/ssh.rs`, `Client::check_server_key` records the fingerprint the
server presented and returns `Ok(true)` whenever there's no stored pin — the key
is accepted with no user involvement, then `connect()` persists it after auth
succeeds. That's convenient but defeats the point of fingerprint verification: a
man-in-the-middle on the very first connection is trusted forever. The system
`ssh` client instead prints the SHA256 fingerprint and asks the user to confirm.
`tauri-plugin-dialog` is already a dependency (`src-tauri/Cargo.toml`), so the
confirm UI is available.

### What to build
On first contact with an unknown host, surface the SHA256 fingerprint and require
explicit confirmation before pinning it to `known_hosts.json`.

- **The hard part is the async boundary.** `check_server_key` runs inside russh's
  `client::Handler` trait during the handshake; it cannot easily pause to await a
  frontend round-trip. Two workable shapes:
  1. **Probe-then-connect** — do a throwaway connect whose handler captures the
     fingerprint and aborts before auth (the `seen` mutex already does the
     capture), show the fingerprint via the dialog plugin, and only on confirm
     run the real `connect()` (which will TOFU-pin it). Simplest; one extra TCP
     handshake.
  2. **One-shot channel** — hand the handler a `tokio::sync::oneshot` (or a
     callback) that emits a `host-key-prompt` event to the frontend and blocks on
     the reply inside `check_server_key`. Cleaner UX (single connection) but more
     plumbing through `connect()`'s signature and `commands.rs`.
- Keep the existing **changed-key rejection** path (`Some(_) => Ok(false)` plus
  the "host key changed" error in `connect()`) exactly as-is — that's the
  security-critical branch and must stay.
- Wire a new command in `src-tauri/src/commands.rs` + `src/lib/ipc.ts` if going
  with the event/channel approach.

### Acceptance / verification
- First connect to a new host → dialog shows the SHA256 fingerprint; user must
  confirm before the tunnel proceeds and before `save_known_host` writes the pin.
- Decline → connection aborts, nothing persisted.
- Second connect to the same (now-pinned) host → no prompt.
- A genuinely changed host key still produces the existing hard error.

### Effort
~half a day. Probe-then-connect is faster to ship; the oneshot approach is the
nicer long-term design.

---

## 3. Multiple server profiles

### Why
Config is single-server today: `AppConfig` (`src-tauri/src/config.rs`) holds one
`ServerConfig` + one `AuthConfig` + the two services, mirrored by `AppConfig` in
`src/lib/types.ts`. Anyone juggling more than one host (staging vs prod, two
boxes) has to retype everything. Both the security review and the original plan
noted profiles as deferred.

### What to build
A named-profile model with a dropdown to switch the active one.

- **Config model** — wrap today's fields in a profile and add a selection. Either
  `profiles: Vec<Profile>` + `active: usize`, or a `HashMap<String, Profile>` +
  `active: String`, where `Profile { server, auth, dashboard, webui }`.
- **Migration** — on load, if an old flat config is found (no `profiles` key),
  fold it into a single profile named "Default" so existing users lose nothing.
  `config::load()` already falls back to defaults on parse error; extend it to
  detect-and-migrate instead of discarding.
- **Keychain** — passwords are stored under a fixed `KEYRING_ACCOUNT`
  (`"ssh-password"`); make the account per-profile (e.g. `ssh-password:<profile>`)
  so passwords don't collide across profiles.
- **known_hosts** — `known_hosts.json` is keyed by `host:port`, so it is already
  profile-agnostic; no change needed.
- **Frontend** — a profile `<select>` plus add/rename/delete in
  `src/lib/Config.svelte`; the active profile drives the existing form. Launch/Logs
  read the active profile's services.

### Acceptance / verification
- Existing single-server config still loads (migrated to "Default") with all
  fields and the stored password intact.
- Add a second profile, switch, Launch → connects to the second host; switch
  back → first host's settings restored.
- Per-profile passwords don't leak across profiles in the keychain.

### Effort
~1 day. The model + migration is the bulk; UI is a dropdown plus a few buttons.

---

## 4. App icon

### Why
The bundle still ships the default Tauri icon. `src-tauri/tauri.conf.json`
references the stock `icons/*` set (`32x32.png`, `128x128.png`, `128x128@2x.png`,
`icon.icns`, `icon.ico`) under `bundle.icon` — these are the placeholder
artwork. A real icon is table-stakes before any distribution.

### What to build
- Supply a square source PNG (1024×1024 recommended, transparent background).
- Run `npm run tauri icon <path-to-source.png>` — Tauri regenerates the full set
  (all PNG sizes, `.icns` for macOS, `.ico` for Windows) into `src-tauri/icons/`,
  overwriting the placeholders. No `tauri.conf.json` change needed since the paths
  stay the same.
- Commit the regenerated icon files.

### Acceptance / verification
- `npm run tauri build` produces installers whose Dock/taskbar/window icon is the
  new artwork on each platform.
- `src-tauri/icons/` no longer contains the default Tauri logo.

### Effort
~15 minutes once branded artwork exists (artwork itself is the real cost).

---

## 5. Tunnel health indicator

### Why
The Launch-tab status dot (`src/lib/Launch.svelte`, `dotClass`) only updates in
response to a user action — Launch/Stop emit `status` events; nothing reflects a
tunnel that died on its own. On the backend, `tunnel::serve`
(`src-tauri/src/tunnel.rs`) only logs `"tunnel connection failed (SSH handle
dead?)"` on a per-connection basis, and the SSH `Handle` can close (`is_closed()`)
without the UI noticing until the next launch. Users can stare at a green dot for
a tunnel that's actually down.

### What to build
A periodic liveness check that drives the dot.

- **Backend** — a check that the SSH handle is live (`handle.is_closed()` is
  already used in `commands.rs::ensure_connected`) and the per-service listener
  task is still running (`Inner.tunnels` holds the `JoinHandle`; `is_finished()`
  is already consulted in `launch_inner`). Expose it either as:
  - a polled command (`check_health` in `commands.rs` + `ipc.ts`) the frontend
    calls on an interval, or
  - a backend `tokio` interval task that emits `status` events (reuses the
    existing `status(app, name, …)` emitter and the store's `status` listener).
- **Frontend** — feed the result into `statusStore`; the dot CSS (`.dot.running`
  /`.starting`/`.error`) already covers the states, so no new styling.
- Keep the interval modest (e.g. 5–10s) and cheap — checking task/handle state is
  in-process; avoid running a remote command every tick.

### Acceptance / verification
- Kill the remote service or drop the network → within one interval the dot turns
  error/idle without the user clicking anything.
- Healthy tunnel → dot stays green across intervals.
- No spurious flicker when idle (no tunnel configured → dot stays idle).

### Effort
~half a day. The interval-task + event approach is the cleaner of the two.

---

## 6. known_hosts management UI

### Why
`src-tauri/src/config.rs` can read and write pinned host keys
(`load_known_host` / `save_known_host` over `known_hosts.json`) but there's no way
to *view or remove* them from inside the app. When a host key legitimately
changes (server rebuild, key rotation), the current recovery path is the error
message telling the user to "remove the entry from known_hosts.json and retry" —
i.e. hand-edit a JSON file in the OS config dir. That's a poor experience and
error-prone.

### What to build
A small management view plus the backend commands to back it.

- **Backend** — add `list_known_hosts() -> Vec<(host:port, fingerprint)>` and
  `forget_known_host(key)` to `config.rs`, exposed as commands in `commands.rs`
  and typed wrappers in `src/lib/ipc.ts`. `save_known_host` already shows the
  read-modify-write pattern over the same `HashMap`; deletion mirrors it.
- **Frontend** — a list (each row: `host:port`, SHA256 fingerprint, a "Forget"
  button) in a new section or tab. After forgetting, the next connect re-pins via
  TOFU (or via the confirm dialog from item 2, if that lands first).
- Restrict-permissions on write (`restrict(path, 0o600)`) is already handled in
  `config.rs`; the delete path must preserve it.

### Acceptance / verification
- Pinned hosts appear in the list with the correct fingerprint.
- "Forget" removes the entry from `known_hosts.json`; the row disappears.
- Reconnecting to a forgotten host re-pins it (and, with item 2, prompts first).

### Effort
~half a day. Two thin commands plus a simple list view.
