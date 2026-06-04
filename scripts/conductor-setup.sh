#!/usr/bin/env zsh
set -euo pipefail

# Conductor runs scripts in a non-interactive shell; ensure rustup's toolchain is on PATH.
[ -f "$HOME/.cargo/env" ] && source "$HOME/.cargo/env"

npm install
# Pre-fetch Rust crates so the first `tauri dev` doesn't stall on network downloads.
(cd src-tauri && cargo fetch)
