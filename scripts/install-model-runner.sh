#!/usr/bin/env bash
set -euo pipefail

# Install the Talvik model runner without installing the
# interactive OID console. Native llama.cpp is installed separately under
# $HOME/.marina/lib. Pass --native-lib-dir when installing a locally built
# release payload; GitHub release archives already contain this directory.
PREFIX="${PREFIX:-${HOME}/.marina}"
NATIVE_LIB_DIR="${LLAMA_CPP_LIB_DIR:-}"
NATIVE_LIB_OVERRIDE=0
UNINSTALL=0
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
      NATIVE_LIB_OVERRIDE=1
      shift 2
      ;;
    --uninstall)
      UNINSTALL=1
      shift
      ;;
    --help|-h)
      echo "Usage: $0 [--prefix DIR] [--native-lib-dir DIR] [--uninstall]"
      exit 0
      ;;
    *)
      echo "unknown option: $1" >&2
      exit 2
      ;;
  esac
done

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
if [[ -x "$SCRIPT_DIR/bin/talvik" && -x "$SCRIPT_DIR/bin/talvikd" ]]; then
  ROOT="$SCRIPT_DIR"
fi
if [[ "$UNINSTALL" == 1 ]]; then
  rm -f "$PREFIX/bin/talvik" "$PREFIX/bin/talvikd"
  find "$PREFIX/lib" -maxdepth 1 \( -type f -o -type l \) \( -name 'libllama.*' -o -name 'libggml*.*' -o -name 'libmtmd.*' -o -name 'liboid-chat-template.*' \) -delete 2>/dev/null || true
  rmdir "$PREFIX/bin" "$PREFIX/lib" 2>/dev/null || true
  echo "Removed Talvik executables and native libraries from $PREFIX; models and state were preserved."
  exit 0
fi

# A release install must not silently succeed with an unusable native runner.
if [[ -x "$ROOT/bin/talvik" && -x "$ROOT/bin/talvikd" ]]; then
  native_check_dir="$ROOT/lib"
else
  NATIVE_LIB_DIR="${NATIVE_LIB_DIR:-$HOME/.marina/lib}"
  native_check_dir="$NATIVE_LIB_DIR"
fi
if [[ "$NATIVE_LIB_OVERRIDE" == 1 ]]; then native_check_dir="$NATIVE_LIB_DIR"; fi
for component in llama ggml ggml-base ggml-cpu mtmd oid-chat-template; do
  if [[ ! -d "$native_check_dir" ]] || [[ -z "$(find "$native_check_dir" -maxdepth 1 -type f -name "lib${component}.*" -print -quit)" ]]; then
    echo "Native payload is incomplete: lib${component} is missing from $native_check_dir. Use a complete Talvik release or --native-lib-dir DIR." >&2
    exit 1
  fi
done

if [[ -x "$ROOT/bin/talvik" && -x "$ROOT/bin/talvikd" ]]; then
  mkdir -p "$PREFIX/bin" "$PREFIX/lib"
  cp -f "$ROOT/bin/talvik" "$ROOT/bin/talvikd" "$PREFIX/bin/"
  if [[ -d "$ROOT/lib" ]]; then
    find "$ROOT/lib" -maxdepth 1 \
      \( -type f -o -type l \) \
      \( -name 'libllama.*' -o -name 'libggml*.*' -o -name 'libmtmd.*' -o -name 'liboid-chat-template.*' \) \
      -exec cp -a {} "$PREFIX/lib/" \;
  fi
  chmod 0755 "$PREFIX/bin/talvik" "$PREFIX/bin/talvikd"
  PAYLOAD_INSTALL=1
else
  PAYLOAD_INSTALL=0
fi

if [[ "$PAYLOAD_INSTALL" == 0 ]]; then
if [[ -n "$NATIVE_LIB_DIR" ]]; then
  export LLAMA_CPP_LIB_DIR="$NATIVE_LIB_DIR"
  export LLAMA_CPP_REQUIRED=1
fi
export LLAMA_CPP_REQUIRED=1
export LLAMA_CPP_MTMD_REQUIRED=1
export LLAMA_CPP_CHAT_TEMPLATE_REQUIRED=1
cargo install --path "$ROOT/console" --root "$PREFIX" --locked --force \
  --bin talvik --bin talvikd
fi

if [[ -n "$NATIVE_LIB_DIR" && ( "$PAYLOAD_INSTALL" == 0 || "$NATIVE_LIB_OVERRIDE" == 1 ) ]]; then
  [[ -d "$NATIVE_LIB_DIR" ]] || { echo "native library directory not found: $NATIVE_LIB_DIR" >&2; exit 1; }
  mkdir -p "$PREFIX/lib"
  native_source_dir="$(cd "$NATIVE_LIB_DIR" && pwd)"
  native_target_dir="$(cd "$PREFIX/lib" && pwd)"
  if [[ "$native_source_dir" != "$native_target_dir" ]]; then
    find "$NATIVE_LIB_DIR" -maxdepth 1 \
      \( -type f -o -type l \) \
      \( -name 'libllama.*' -o -name 'libggml*.*' -o -name 'libmtmd.*' -o -name 'liboid-chat-template.*' -o -name 'llama.dll' -o -name 'ggml*.dll' -o -name 'oid-chat-template.dll' \) \
      -exec cp -a {} "$PREFIX/lib/" \;
  fi
fi

STATE_DIR="${TALVIK_STATE_DIR:-${MARINA_STATE_DIR:-${TALVIK_HOME:-$PREFIX}}}"
MODEL_DIR="${TALVIK_MODEL_DIR:-${MARINA_MODEL_DIR:-$STATE_DIR/models}}"
mkdir -p "$STATE_DIR" "$MODEL_DIR"
echo
echo "Installed model runner daemon:  $PREFIX/bin/talvikd"
echo "Installed model runner client:  $PREFIX/bin/talvik"
echo "Runtime state directory:        $STATE_DIR"
echo "Model directory:                $MODEL_DIR"
echo
echo "Start the service with: talvikd"
echo "Check it with: talvik status"
echo "Add $PREFIX/bin to PATH if needed."
if [[ -d "$PREFIX/lib" ]] && [[ -n "$(find "$PREFIX/lib" -maxdepth 1 -type f -print -quit)" ]]; then
  echo "Native backend libraries:       $PREFIX/lib"
else
  echo "Native backend not installed; provide a release lib/ payload or use --native-lib-dir DIR."
fi
