#!/bin/sh
set -eu

usage() { echo "usage: $0 --file MODEL.gguf --sha256 HEX --model-id ID [--dest DIR]" >&2; exit 2; }
file=; sha256=; model_id=; dest=${MARINA_MODEL_DIR:-/var/lib/marina/models}
while [ "$#" -gt 0 ]; do
    case "$1" in
        --file) file=${2:?missing value for --file}; shift 2 ;;
        --sha256) sha256=${2:?missing value for --sha256}; shift 2 ;;
        --model-id) model_id=${2:?missing value for --model-id}; shift 2 ;;
        --dest) dest=${2:?missing value for --dest}; shift 2 ;;
        *) usage ;;
    esac
done
[ -n "$file" ] && [ -n "$sha256" ] && [ -n "$model_id" ] || usage
[ -f "$file" ] || { echo "model file not found: $file" >&2; exit 1; }
printf '%s' "$sha256" | grep -Eq '^[0-9a-fA-F]{64}$' || { echo "invalid sha256" >&2; exit 1; }
printf '%s' "$model_id" | grep -Eq '^[A-Za-z0-9._-]+$' || { echo "invalid model id" >&2; exit 1; }
actual=$(sha256sum "$file" | awk '{print $1}')
[ "$(printf '%s' "$actual" | tr '[:upper:]' '[:lower:]')" = "$(printf '%s' "$sha256" | tr '[:upper:]' '[:lower:]')" ] || { echo "model checksum mismatch" >&2; exit 1; }
mkdir -p "$dest"
staged="$dest/.${model_id}.gguf.$$"; target="$dest/${model_id}.gguf"
trap 'rm -f "$staged"' EXIT INT TERM
cp "$file" "$staged"; chmod 0640 "$staged"; chown marina:marina "$staged" 2>/dev/null || true; mv "$staged" "$target"
printf '%s\n' "installed model_id=$model_id sha256=$actual path=$target"

