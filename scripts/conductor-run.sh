#!/usr/bin/env zsh
set -euo pipefail

# Conductor runs scripts in a non-interactive shell; ensure rustup's toolchain is on PATH.
[ -f "$HOME/.cargo/env" ] && source "$HOME/.cargo/env"

npm run tauri dev
