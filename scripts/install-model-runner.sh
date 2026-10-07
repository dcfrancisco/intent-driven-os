#!/usr/bin/env bash
set -euo pipefail

# Install the Marina model-runner service and client without installing the
# interactive OID console. Native llama.cpp is installed separately under
# $HOME/.marina/lib. Pass --native-lib-dir when installing a locally built
# release payload; GitHub release archives already contain this directory.
PREFIX="${PREFIX:-${HOME}/.marina}"
NATIVE_LIB_DIR="${LLAMA_CPP_LIB_DIR:-}"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --prefix)
      [[ $# -ge 2 ]] || { echo "--prefix requires a directory" >&2; exit 2; }
      PREFIX="$2"
      shift 2
      ;;
    --native-lib-dir)
      [[ $# -ge 2 ]] || { echo "--native-lib-dir requires a directory" >&2; exit 2; }
      NATIVE_LIB_DIR="$2"
      shift 2
      ;;
    --help|-h)
      echo "Usage: $0 [--prefix DIR] [--native-lib-dir DIR]"
      exit 0
      ;;
    *)
      echo "unknown option: $1" >&2
      exit 2
      ;;
  esac
done

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ -n "$NATIVE_LIB_DIR" ]]; then
  export LLAMA_CPP_LIB_DIR="$NATIVE_LIB_DIR"
  export LLAMA_CPP_REQUIRED=1
fi
cargo install --path "$ROOT/console" --root "$PREFIX" --locked --force \
  --bin marina --bin marinactl

if [[ -n "$NATIVE_LIB_DIR" ]]; then
  [[ -d "$NATIVE_LIB_DIR" ]] || { echo "native library directory not found: $NATIVE_LIB_DIR" >&2; exit 1; }
  mkdir -p "$PREFIX/lib"
  find "$NATIVE_LIB_DIR" -maxdepth 1 -type f \( -name 'libllama.*' -o -name 'libggml*.*' -o -name 'llama.dll' -o -name 'ggml*.dll' \) -exec cp -f {} "$PREFIX/lib/" \;
fi

STATE_DIR="${MARINA_STATE_DIR:-${HOME}/.marina}"
MODEL_DIR="${MARINA_MODEL_DIR:-$STATE_DIR/models}"
mkdir -p "$STATE_DIR" "$MODEL_DIR"
echo
echo "Installed model-runner service: $PREFIX/bin/marina"
echo "Installed model-runner client:  $PREFIX/bin/marinactl"
echo "Runtime state directory:        $STATE_DIR"
echo "Model directory:                $MODEL_DIR"
echo
echo "Start the service with: marina"
echo "Check it with: marinactl status"
echo "Add $PREFIX/bin to PATH if needed."
if [[ -d "$PREFIX/lib" ]] && [[ -n "$(find "$PREFIX/lib" -maxdepth 1 -type f -print -quit)" ]]; then
  echo "Native backend libraries:       $PREFIX/lib"
else
  echo "Native backend not installed; provide a release lib/ payload or use --native-lib-dir DIR."
fi
