#!/usr/bin/env bash
set -euo pipefail

# Rootless OID console install. Marina remains independently managed by
# scripts/install-model-runner.sh; this script installs only the OID client
# and optionally places a supplied native payload in the canonical Marina lib.
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PREFIX="${OID_PREFIX:-${XDG_BIN_HOME:-${HOME}/.local}}"
MARINA_HOME="${MARINA_HOME:-${HOME}/.marina}"
NATIVE_LIB_DIR="${LLAMA_CPP_LIB_DIR:-}"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --prefix) PREFIX="$2"; shift 2;;
    --marina-home) MARINA_HOME="$2"; shift 2;;
    --native-lib-dir) NATIVE_LIB_DIR="$2"; shift 2;;
    --help|-h)
      echo "Usage: $0 [--prefix DIR] [--marina-home DIR] [--native-lib-dir DIR]"
      exit 0
      ;;
    *) echo "unknown option: $1" >&2; exit 2;;
  esac
done
[[ -d "$ROOT/console" ]] || { echo "console source tree not found: $ROOT/console" >&2; exit 1; }
command -v cargo >/dev/null || { echo "cargo is required" >&2; exit 1; }
cargo install --path "$ROOT/console" --root "$PREFIX" --locked --force --bin oid-console
mkdir -p "$MARINA_HOME/models" "$MARINA_HOME/state" "$MARINA_HOME/logs"
if [[ -n "$NATIVE_LIB_DIR" ]]; then
  [[ -d "$NATIVE_LIB_DIR" ]] || { echo "native library directory not found: $NATIVE_LIB_DIR" >&2; exit 1; }
  mkdir -p "$MARINA_HOME/lib"
  find "$NATIVE_LIB_DIR" -maxdepth 1 \( -type f -o -type l \) \
    \( -name 'libllama.*' -o -name 'libggml*.*' \) -exec cp -a {} "$MARINA_HOME/lib/" \;
fi
printf '%s\n' "installed oid-console: $PREFIX/bin/oid-console" \
  "Marina remains separate: $MARINA_HOME/bin/marina" \
  "models: $MARINA_HOME/models"
