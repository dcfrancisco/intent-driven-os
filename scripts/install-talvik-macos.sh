#!/usr/bin/env bash
set -euo pipefail

PREFIX="${PREFIX:-${HOME}/.marina}"
SERVICE=0
UNINSTALL=0
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

while (($#)); do
  case "$1" in
    --prefix) [[ $# -ge 2 ]] || { echo "--prefix requires a directory" >&2; exit 2; }; PREFIX=$2; shift 2 ;;
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
LAUNCH_AGENT_DIR="${TALVIK_LAUNCH_AGENT_DIR:-$HOME/Library/LaunchAgents}"
PLIST="$LAUNCH_AGENT_DIR/com.talvik.talvikd.plist"

if [[ "$UNINSTALL" == 1 ]]; then
  if [[ "$SERVICE" == 1 && -f "$PLIST" ]]; then
    launchctl bootout "gui/$(id -u)" "$PLIST" 2>/dev/null || true
    rm -f "$PLIST"
  fi
  "$SCRIPT_DIR/install-model-runner.sh" --prefix "$PREFIX" --uninstall
  echo "Removed Talvik binaries and native libraries; models and state were preserved."
  exit 0
fi

if [[ "$SERVICE" == 1 ]]; then
  TEMPLATE="$SCRIPT_DIR/talvikd.plist.in"
  if [[ ! -f "$TEMPLATE" ]]; then
    TEMPLATE="$SCRIPT_DIR/../packaging/talvik/talvikd.plist.in"
  fi
  [[ -f "$TEMPLATE" ]] || { echo "Talvik launchd template is missing" >&2; exit 1; }
fi

# Stop the managed service before replacing its binaries and native payload.
if [[ "$SERVICE" == 1 ]]; then
  command -v launchctl >/dev/null || { echo "launchctl is required for --service on macOS" >&2; exit 1; }
  launchctl bootout "gui/$(id -u)" "$PLIST" 2>/dev/null || true
fi
"$SCRIPT_DIR/install-model-runner.sh" --prefix "$PREFIX"
if [[ "$SERVICE" == 1 ]]; then
  command -v launchctl >/dev/null || {
    echo "launchctl is required for --service on macOS" >&2
    exit 1
  }
  mkdir -p "$LAUNCH_AGENT_DIR" "$PREFIX/logs"
  # Escape both XML text and sed replacement syntax, including prefixes
  # containing spaces, ampersands, or the substitution delimiter.
  PREFIX_XML="$(printf '%s' "$PREFIX" | sed -e 's/&/\&amp;/g' -e 's/</\&lt;/g' -e 's/>/\&gt;/g' -e 's/"/\&quot;/g')"
  PREFIX_REPLACEMENT="$(printf '%s' "$PREFIX_XML" | sed 's/[\\&|]/\\&/g')"
  sed \
    -e "s|__TALVIK_HOME__|$PREFIX_REPLACEMENT|g" \
    "$TEMPLATE" >"$PLIST"
  launchctl bootstrap "gui/$(id -u)" "$PLIST"
  echo "Installed and started Talvik for the current user."
  echo "OpenAI base URL: http://127.0.0.1:11435/v1"
  echo "Check readiness: curl http://127.0.0.1:11435/readyz"
  echo "Create an IDE token: $PREFIX/bin/talvik auth token create vscode"
  echo "Stop: launchctl bootout gui/$(id -u) \"$PLIST\""
  echo "Start: launchctl bootstrap gui/$(id -u) \"$PLIST\""
fi
