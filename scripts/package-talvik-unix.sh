#!/usr/bin/env bash
# Package an already-built Unix runner. This does not qualify a release.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN_DIR="${1:?usage: $0 BIN_DIR NATIVE_LIB_DIR OUTPUT.tar.gz}"
LIB_DIR="${2:?native library directory required}"
OUTPUT="${3:?output .tar.gz required}"
[[ "$OUTPUT" == *.tar.gz ]] || { echo "output must end in .tar.gz" >&2; exit 2; }
for binary in talvik talvikd; do
  [[ -x "$BIN_DIR/$binary" ]] || { echo "missing executable: $BIN_DIR/$binary" >&2; exit 1; }
done
for component in llama ggml ggml-base ggml-cpu mtmd oid-chat-template; do
  [[ -n "$(find "$LIB_DIR" -maxdepth 1 -type f -name "lib${component}.*" -print -quit)" ]] || {
    echo "incomplete native payload: lib${component}" >&2; exit 1;
  }
done
WORK="$(mktemp -d "${TMPDIR:-/tmp}/talvik-package.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$WORK/talvik/bin" "$WORK/talvik/lib" "$(dirname "$OUTPUT")"
cp "$BIN_DIR/talvik" "$BIN_DIR/talvikd" "$WORK/talvik/bin/"
find "$LIB_DIR" -maxdepth 1 \( -type f -o -type l \) \
  \( -name 'libllama.*' -o -name 'libggml*.*' -o -name 'libmtmd.*' -o -name 'liboid-chat-template.*' \) \
  -exec cp -a {} "$WORK/talvik/lib/" \;
cp "$ROOT/scripts/install-model-runner.sh" "$ROOT/scripts/install-talvik-macos.sh" "$WORK/talvik/"
cp "$ROOT/packaging/talvik/talvikd.plist.in" "$ROOT/LICENSE" "$WORK/talvik/"
env -u DYLD_LIBRARY_PATH -u LD_LIBRARY_PATH "$WORK/talvik/bin/talvikd" version --json > "$WORK/talvik/build-info.json"
tar -C "$WORK" -czf "$OUTPUT" talvik
(cd "$(dirname "$OUTPUT")" && shasum -a 256 "$(basename "$OUTPUT")" > "$(basename "$OUTPUT").sha256")
echo "Packaged $OUTPUT (qualification required before release)"
