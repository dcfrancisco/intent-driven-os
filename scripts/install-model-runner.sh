#!/usr/bin/env bash
set -euo pipefail

# Install the Marina model-runner service and client without installing the
# interactive OID console. The native llama.cpp library remains optional at
# install time and can be configured later with LLAMA_CPP_LIB_DIR.
PREFIX="${PREFIX:-${HOME}/.local}"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --prefix)
      [[ $# -ge 2 ]] || { echo "--prefix requires a directory" >&2; exit 2; }
      PREFIX="$2"
      shift 2
      ;;
    --help|-h)
      echo "Usage: $0 [--prefix DIR]"
      exit 0
      ;;
    *)
      echo "unknown option: $1" >&2
      exit 2
      ;;
  esac
done

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cargo install --path "$ROOT/console" --root "$PREFIX" --locked \
  --bin marina --bin marinactl

mkdir -p "${MARINA_STATE_DIR:-${HOME}/.marina}/models"
echo
echo "Installed model-runner service: $PREFIX/bin/marina"
echo "Installed model-runner client:  $PREFIX/bin/marinactl"
echo "State/models directory:         ${MARINA_STATE_DIR:-${HOME}/.marina}"
echo
echo "Start the service with: marina"
echo "Check it with: marinactl status"
echo "Set LLAMA_CPP_LIB_DIR before rebuilding/configuring a native llama.cpp backend."
