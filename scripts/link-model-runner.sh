#!/usr/bin/env bash
set -euo pipefail

# Expose locally built release binaries through a stable, repository-local path.
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RELEASE_DIR="$ROOT/target/release"
LINK_DIR="$ROOT/talvik/bin"
LINK_LIB_DIR="$ROOT/talvik/lib"
# Keep this helper independent from LLAMA_CPP_LIB_DIR, which may point at a
# build-only or model directory in a user's shell. Override explicitly when
# linking a non-canonical native payload.
NATIVE_LIB_DIR="${TALVIK_NATIVE_LIB_DIR:-${HOME}/.marina/lib}"
BINARIES=(talvik talvikd)

for binary in "${BINARIES[@]}"; do
  source_path="$RELEASE_DIR/$binary"
  if [[ ! -x "$source_path" ]]; then
    echo "Missing release binary: $source_path" >&2
    echo "Build it with: cargo build --release -p oid-console --bin talvik --bin talvikd" >&2
    exit 1
  fi
done

mkdir -p "$LINK_DIR"
mkdir -p "$LINK_LIB_DIR"

for binary in "${BINARIES[@]}"; do
  link_path="$LINK_DIR/$binary"
  if [[ -e "$link_path" && ! -L "$link_path" ]]; then
    echo "Refusing to replace non-symlink: $link_path" >&2
    exit 1
  fi

  ln -sfn "../../target/release/$binary" "$link_path"
  echo "Linked $link_path -> ../../target/release/$binary"
done

if [[ -d "$NATIVE_LIB_DIR" ]]; then
  native_count=0
  while IFS= read -r -d '' native_library; do
    library_name="$(basename "$native_library")"
    ln -sfn "$(cd "$(dirname "$native_library")" && pwd)/$library_name" \
      "$LINK_LIB_DIR/$library_name"
    native_count=$((native_count + 1))
  done < <(
    find "$NATIVE_LIB_DIR" -maxdepth 1 \( -type f -o -type l \) \
      \( -name 'libllama.*' -o -name 'libggml*.*' \) -print0
  )
  if (( native_count > 0 )); then
    echo "Linked $native_count native libraries into $LINK_LIB_DIR"
  else
    echo "No native llama.cpp libraries found in $NATIVE_LIB_DIR" >&2
  fi
else
  echo "Native library directory not found: $NATIVE_LIB_DIR" >&2
  echo "Set TALVIK_NATIVE_LIB_DIR or install llama.cpp under $HOME/.marina/lib" >&2
fi
