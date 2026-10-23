#!/usr/bin/env bash
set -euo pipefail

# Expose locally built release binaries through the canonical per-user Marina
# installation path. The repository is only the build source; it does not own
# a second Talvik installation directory.
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RELEASE_DIR="$ROOT/target/release"
LINK_DIR="${TALVIK_BIN_DIR:-${HOME}/.marina/bin}"
LINK_LIB_DIR="${TALVIK_LIB_DIR:-${HOME}/.marina/lib}"
# Keep this helper independent from LLAMA_CPP_LIB_DIR, which may point at a
# build-only or model directory in a user's shell. Override explicitly when
# linking a non-canonical native payload.
NATIVE_LIB_DIR="${TALVIK_NATIVE_LIB_DIR:-$LINK_LIB_DIR}"
BINARIES=(talvik talvikd)

for binary in "${BINARIES[@]}"; do
  source_path="$RELEASE_DIR/$binary"
  if [[ ! -x "$source_path" ]]; then
    echo "Missing release binary: $source_path" >&2
    echo "Build it with: cargo build --release -p oid-console --bin talvik --bin talvikd" >&2
    exit 1
  fi
done

mkdir -p "$LINK_DIR" "$LINK_LIB_DIR"

for binary in "${BINARIES[@]}"; do
  source_path="$RELEASE_DIR/$binary"
  link_path="$LINK_DIR/$binary"
  if [[ -e "$link_path" && ! -L "$link_path" ]]; then
    echo "Refusing to replace non-symlink: $link_path" >&2
    exit 1
  fi

  # macOS ln -f may follow an existing symlink. Remove only the symlink so
  # talvik and talvikd cannot accidentally collapse onto the same binary.
  if [[ -L "$link_path" ]]; then
    unlink "$link_path"
  fi
  ln -s "$source_path" "$link_path"
  echo "Linked $link_path -> $source_path"
done

if [[ -d "$NATIVE_LIB_DIR" ]]; then
  native_count=0
  while IFS= read -r -d '' native_library; do
    library_name="$(basename "$native_library")"
    source_dir="$(cd "$(dirname "$native_library")" && pwd)"
    target_dir="$(cd "$LINK_LIB_DIR" && pwd)"
    if [[ "$source_dir" != "$target_dir" ]]; then
      ln -sfn "$source_dir/$library_name" "$LINK_LIB_DIR/$library_name"
    fi
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
