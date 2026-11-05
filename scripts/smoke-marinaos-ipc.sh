#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DAEMON="${MARINAOSD:-$ROOT/target/debug/marinaosd}"
CLI="${MARINAOSCTL:-$ROOT/target/debug/marinaosctl}"
[[ -x "$DAEMON" && -x "$CLI" ]] || { echo "build marinaosd and marinaosctl first" >&2; exit 1; }

WORK="$(mktemp -d)"
cleanup() {
  if [[ -n "${DAEMON_PID:-}" ]]; then kill "$DAEMON_PID" >/dev/null 2>&1 || true; wait "$DAEMON_PID" >/dev/null 2>&1 || true; fi
  rm -rf "$WORK"
}
trap cleanup EXIT
cat >"$WORK/marinaos.conf" <<EOF
state_directory=$WORK/state
readiness_file=$WORK/run/ready.json
socket_path=$WORK/run/marinaos.sock
talvik_optional=true
EOF

"$DAEMON" --config="$WORK/marinaos.conf" >"$WORK/daemon.log" 2>&1 &
DAEMON_PID=$!
for _ in {1..50}; do
  [[ -S "$WORK/run/marinaos.sock" ]] && break
  sleep 0.1
done
[[ -S "$WORK/run/marinaos.sock" ]] || { cat "$WORK/daemon.log" >&2; exit 1; }

"$CLI" --config "$WORK/marinaos.conf" --json status | grep -F '"service":"ready"' >/dev/null
"$CLI" --config "$WORK/marinaos.conf" --json doctor | grep -F '"offline_mode":"available"' >/dev/null
"$CLI" --config "$WORK/marinaos.conf" --json hardware | grep -F '"version":1' >/dev/null

python3 - "$WORK/run/marinaos.sock" <<'PY'
import socket
import sys

with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as client:
    client.connect(sys.argv[1])
    client.sendall(b'{"version":1,"method":"status"\n')
    response = client.recv(4096).decode()
    if '"error":"unsupported_request"' not in response:
        raise SystemExit("malformed request was not rejected")
PY

cat >"$WORK/invalid.conf" <<EOF
unknown_key=true
EOF
if "$DAEMON" --config="$WORK/invalid.conf" >"$WORK/invalid.log" 2>&1; then
  echo "invalid configuration unexpectedly started daemon" >&2
  exit 1
fi
grep -F "unknown key unknown_key" "$WORK/invalid.log" >/dev/null
echo "MarinaOS IPC smoke test passed"
