# Hermes Launcher — Future Ideas / Backlog

Detailed notes for work deferred from the initial build. Each item is scoped
enough to pick up cold. Paths are relative to the repo root.

---

## 1. SSH-agent authentication

### Why
The app currently supports two auth modes:
- **password** (from OS keychain), and
- **key file** — reads a private key from a path via `load_secret_key(&path, None)` in `src-tauri/src/ssh.rs`.

Two real gaps this creates:
1. **Passphrase-protected keys fail.** `load_secret_key(path, None)` passes `None`
   for the passphrase, so an encrypted `id_rsa` cannot be loaded. Many users keep
   a passphrase on their key.
2. **Hardware-backed keys are unusable.** YubiKey / FIDO (`sk-ssh-ed25519`) and
   macOS Secure Enclave keys never expose private bytes on disk, so the file-load
   path can't use them at all.

`ssh-agent` solves both: the agent holds already-unlocked keys (incl. hardware
keys) in memory and signs challenges on request. This is how the system `ssh`
client works by default, so it's the least-surprise behavior.

### What to build
Add a third auth mode `"agent"` and make it the default on macOS/Linux.

**Backend — `src-tauri/src/ssh.rs`:**
- russh ships an agent client: `russh::keys::agent::client::AgentClient`.
  Connect via `AgentClient::connect_env()` (reads `$SSH_AUTH_SOCK`).
- Flow in `connect()` for `mode == "agent"`:
  1. `let mut agent = AgentClient::connect_env().await?;`
  2. `let identities = agent.request_identities().await?;` (Vec of public keys).
  3. If empty → return a clear error: "ssh-agent has no keys loaded (run `ssh-add`)".
  4. For each identity, try
     `handle.authenticate_publickey_with(&username, pubkey, hash_alg, &mut agent).await`
     (russh exposes an agent-signing auth method; verify the exact name against
     the installed russh 0.61 API — it may be `authenticate_future` /
     an agent-backed `PrivateKeyWithHashAlg` equivalent). Stop on first `Success`.
  5. If none succeed → "no agent key accepted by server".
- Keep `password` and `key` modes exactly as-is (fallbacks).

**Config model — `src-tauri/src/config.rs` + `src/lib/types.ts`:**
- `AuthConfig.mode` already free-form string; add `"agent"` as a valid value.
- No new fields needed (agent mode ignores `key_path`).

**Frontend — `src/lib/Config.svelte`:**
- Add a third `<option value="agent">SSH agent</option>` to the auth method select.
- When `mode === "agent"`, hide both the key-path Browse row and the password row
  (show a hint: "Uses keys loaded in your ssh-agent").
- Make `"agent"` the default in `defaultConfig()` (`src/lib/types.ts`) on first run.

### Windows caveat
On Windows the agent is either the OpenSSH agent (named pipe
`\\.\pipe\openssh-ssh-agent`) or Pageant. `connect_env()` relies on
`$SSH_AUTH_SOCK`, which the OpenSSH agent service does not set the same way.
Plan: gate a Windows-specific named-pipe connect (russh may need
`AgentClient::connect_named_pipe`), and fall back to key/password if no agent is
reachable. Don't block the macOS/Linux feature on perfect Windows support —
detect "no agent" and surface a helpful message.

### Acceptance / verification
- `ssh-add -l` shows a key → launch with mode=agent connects with no key path.
- Encrypted key in the agent works (the original failure case).
- Agent empty → friendly "run ssh-add" error in the card + Logs.
- Existing key-file and password modes still work.

### Effort
~30–40 lines in `ssh.rs`, a few lines of config/UI. Half a day incl. Windows
path. Verify russh 0.61's exact agent-auth method name first (the API has
churned across versions) — check `cargo doc -p russh --open`, module
`keys::agent`.

---

## 2. Cross-platform CI build matrix (GitHub Actions)

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

## Parking lot (smaller ideas, unprioritized)
- **Interactive host-key confirm** — instead of silent TOFU pin on first use,
  show the fingerprint and ask the user to confirm (dialog plugin already added).
- **Multiple server profiles** — config is single-server today; add a profile
  dropdown (the review and original plan both noted this as deferred).
- **App icon** — still the default Tauri icon; run `npm run tauri icon <png>`.
- **Tunnel health indicator** — periodic check that the listener + SSH handle are
  alive, reflected in the Launch-tab dot, rather than status only on action.
- **known_hosts management UI** — view/forget pinned host keys from within the app.
