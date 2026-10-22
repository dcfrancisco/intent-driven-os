#!/usr/bin/env bash
set -euo pipefail

# Transport/configuration smoke test. Native inference requires a built
# llama.cpp library and a real GGUF, so this deliberately validates the
# standalone daemon boundary without pretending that a model was loaded.
ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
TMP_DIR=$(mktemp -d "${TMPDIR:-/tmp}/marina-smoke.XXXXXX")
trap 'kill "${TALVIKD_PID:-0}" 2>/dev/null || true; rm -rf "$TMP_DIR"' EXIT

export MARINA_HOME="$TMP_DIR/home"
export MARINA_SOCKET="$TMP_DIR/marina.sock"
mkdir -p "$MARINA_HOME/.marina"

cargo build -p oid-console --bin talvikd --bin talvik >/dev/null
"$ROOT_DIR/target/debug/talvikd" >"$TMP_DIR/talvikd.log" 2>&1 &
TALVIKD_PID=$!

for _ in $(seq 1 50); do
  if "$ROOT_DIR/target/debug/talvik" status >/dev/null 2>&1; then
    break
  fi
  sleep 0.1
done

"$ROOT_DIR/target/debug/talvik" status
"$ROOT_DIR/target/debug/talvik" models
echo "Talvik transport smoke test passed"
