#!/usr/bin/env bash
set -euo pipefail

ARCHIVE="${1:?usage: $0 RELEASE_ARCHIVE}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/talvik-release-smoke.XXXXXX")"
PREFIX="$WORK/prefix"
STATE="$WORK/state"
SOCKET="$STATE/marina.sock"
cleanup() {
  if [[ -n "${DAEMON_PID:-}" ]]; then kill "$DAEMON_PID" 2>/dev/null || true; fi
  rm -rf "$WORK"
}
trap cleanup EXIT

mkdir -p "$PREFIX" "$STATE/models"
case "$ARCHIVE" in
  *.tar.gz|*.tgz) tar -xzf "$ARCHIVE" -C "$WORK" ;;
  *) echo "unsupported Unix archive: $ARCHIVE" >&2; exit 2 ;;
esac
PAYLOAD="$WORK/talvik"
[[ -x "$PAYLOAD/bin/talvik" && -x "$PAYLOAD/bin/talvikd" ]] || {
  echo "archive does not contain talvik/bin/talvik and talvikd" >&2; exit 1;
}
(cd "$PAYLOAD" && ./install-model-runner.sh --prefix "$PREFIX")
TALVIK_HOME="$STATE" TALVIK_SOCKET="$SOCKET" "$PREFIX/bin/talvik" --help >/dev/null
TALVIK_HOME="$STATE" TALVIK_SOCKET="$SOCKET" "$PREFIX/bin/talvik" --version >/dev/null
TALVIK_HOME="$STATE" TALVIK_SOCKET="$SOCKET" "$PREFIX/bin/talvikd" --help >/dev/null
TALVIK_HOME="$STATE" TALVIK_SOCKET="$SOCKET" "$PREFIX/bin/talvikd" >"$WORK/daemon.log" 2>&1 &
DAEMON_PID=$!
for _ in $(seq 1 30); do
  if TALVIK_HOME="$STATE" TALVIK_SOCKET="$SOCKET" "$PREFIX/bin/talvik" --json status >/dev/null 2>&1; then break; fi
  sleep 1
done
TALVIK_HOME="$STATE" TALVIK_SOCKET="$SOCKET" "$PREFIX/bin/talvik" --json status | grep -q '"health"'
TALVIK_HOME="$STATE" TALVIK_SOCKET="$SOCKET" "$PREFIX/bin/talvik" --json models >/dev/null
TALVIK_HOME="$STATE" TALVIK_SOCKET="$SOCKET" "$PAYLOAD/install-model-runner.sh" --prefix "$PREFIX" --uninstall
[[ ! -e "$PREFIX/bin/talvik" && -d "$STATE/models" ]] || {
  echo "uninstall did not remove binaries while preserving model state" >&2; exit 1;
}
echo "Talvik release install smoke: PASS"
