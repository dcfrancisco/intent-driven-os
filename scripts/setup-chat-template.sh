#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
LLAMA_CPP_ROOT="${LLAMA_CPP_ROOT:?LLAMA_CPP_ROOT must point to the pinned llama.cpp checkout}"
LLAMA_CPP_ROOT="$(cd "$LLAMA_CPP_ROOT" && pwd)"
LLAMA_CPP_INSTALL_DIR="${LLAMA_CPP_INSTALL_DIR:-${HOME}/.marina/lib}"
mkdir -p "$LLAMA_CPP_INSTALL_DIR"
LLAMA_CPP_INSTALL_DIR="$(cd "$LLAMA_CPP_INSTALL_DIR" && pwd)"
CHAT_BUILD="${TALVIK_CHAT_TEMPLATE_BUILD:-${TMPDIR:-/tmp}/talvik-chat-template-build}"
CMAKE="${CMAKE_COMMAND:-cmake}"
"$CMAKE" -S "$ROOT/runtime/adapters/llama-cpp/sys/native" -B "$CHAT_BUILD" \
  -DLLAMA_CPP_SOURCE_DIR="$LLAMA_CPP_ROOT" -DLLAMA_CPP_LIB_DIR="$LLAMA_CPP_INSTALL_DIR" -DCMAKE_BUILD_TYPE=Release
"$CMAKE" --build "$CHAT_BUILD" --config Release --parallel "${CMAKE_BUILD_PARALLEL_LEVEL:-2}"
"$CMAKE" --install "$CHAT_BUILD" --config Release --prefix "$LLAMA_CPP_INSTALL_DIR"
