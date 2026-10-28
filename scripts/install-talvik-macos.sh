#!/usr/bin/env bash
set -euo pipefail

PREFIX="${PREFIX:-${HOME}/.marina}"
SERVICE=0
UNINSTALL=0
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

while (($#)); do
  case "$1" in
    --prefix) PREFIX=$2; shift 2 ;;
    --service) SERVICE=1; shift ;;
    --uninstall) UNINSTALL=1; shift ;;
    --help|-h)
      echo "Usage: $0 [--prefix DIR] [--service] [--uninstall]"
      exit 0
      ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
done

PREFIX="$(cd "$PREFIX" 2>/dev/null && pwd -P || { mkdir -p "$PREFIX"; cd "$PREFIX" && pwd -P; })"
PLIST="$HOME/Library/LaunchAgents/com.talvik.talvikd.plist"

if [[ "$UNINSTALL" == 1 ]]; then
  if [[ "$SERVICE" == 1 && -f "$PLIST" ]]; then
    launchctl bootout "gui/$(id -u)" "$PLIST" 2>/dev/null || true
    rm -f "$PLIST"
  fi
  "$SCRIPT_DIR/install-model-runner.sh" --prefix "$PREFIX" --uninstall
  echo "Removed Talvik binaries and native libraries; models and state were preserved."
  exit 0
fi

"$SCRIPT_DIR/install-model-runner.sh" --prefix "$PREFIX"
if [[ "$SERVICE" == 1 ]]; then
  command -v launchctl >/dev/null || {
    echo "launchctl is required for --service on macOS" >&2
    exit 1
  }
  mkdir -p "$HOME/Library/LaunchAgents" "$PREFIX/logs"
  sed \
    -e "s|__TALVIK_HOME__|$PREFIX|g" \
    "$SCRIPT_DIR/talvikd.plist.in" >"$PLIST"
  launchctl bootout "gui/$(id -u)" "$PLIST" 2>/dev/null || true
  launchctl bootstrap "gui/$(id -u)" "$PLIST"
  echo "Installed the per-user launchd definition (not started automatically)."
  echo "Start it with: launchctl kickstart -k gui/$(id -u)/com.talvik.talvikd"
fi
