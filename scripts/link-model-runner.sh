#!/usr/bin/env bash
set -euo pipefail

# Expose locally built release binaries through a stable, repository-local path.
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RELEASE_DIR="$ROOT/target/release"
LINK_DIR="$ROOT/marina/bin"
BINARIES=(marina marinactl)

for binary in "${BINARIES[@]}"; do
  source_path="$RELEASE_DIR/$binary"
  if [[ ! -x "$source_path" ]]; then
    echo "Missing release binary: $source_path" >&2
    echo "Build it with: cargo build --release -p oid-console --bin marina --bin marinactl" >&2
    exit 1
  fi
done

mkdir -p "$LINK_DIR"

for binary in "${BINARIES[@]}"; do
  link_path="$LINK_DIR/$binary"
  if [[ -e "$link_path" && ! -L "$link_path" ]]; then
    echo "Refusing to replace non-symlink: $link_path" >&2
    exit 1
  fi

  ln -sfn "../../target/release/$binary" "$link_path"
  echo "Linked $link_path -> ../../target/release/$binary"
done
