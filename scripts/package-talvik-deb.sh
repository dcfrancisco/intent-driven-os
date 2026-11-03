#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
VERSION=0.1.0
ARCH=amd64
TALVIKD="$ROOT/target/release/talvikd"
TALVIK="$ROOT/target/release/talvik"
NATIVE_LIB_DIR=""
OUTPUT="$ROOT/artifacts/talvik_${VERSION}_${ARCH}.deb"

while (($#)); do
  case "$1" in
    --version) VERSION=$2; shift 2 ;;
    --talvikd) TALVIKD=$2; shift 2 ;;
    --talvik) TALVIK=$2; shift 2 ;;
    --native-lib-dir) NATIVE_LIB_DIR=$2; shift 2 ;;
    --output) OUTPUT=$2; shift 2 ;;
    --help|-h) echo "Usage: package-talvik-deb.sh --native-lib-dir PATH [--version VERSION]"; exit 0 ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
done

command -v dpkg-deb >/dev/null || { echo "dpkg-deb is required" >&2; exit 1; }
[[ -x "$TALVIKD" && -x "$TALVIK" ]] || { echo "release talvik and talvikd binaries are required" >&2; exit 1; }
[[ -n "$NATIVE_LIB_DIR" && -d "$NATIVE_LIB_DIR" ]] || { echo "--native-lib-dir is required" >&2; exit 1; }
for component in llama ggml ggml-base ggml-cpu mtmd oid-chat-template; do
  [[ -n "$(find "$NATIVE_LIB_DIR" -maxdepth 1 -type f -name "lib${component}.so*" -print -quit)" ]] || {
    echo "incomplete native payload: lib${component}.so is missing from $NATIVE_LIB_DIR" >&2; exit 1;
  }
done

WORK=$(mktemp -d "${TMPDIR:-/tmp}/talvik-deb.XXXXXX")
trap 'rm -rf "$WORK"' EXIT
PKG="$WORK/package"
install -d "$PKG/DEBIAN" "$PKG/usr/lib/talvik/bin" "$PKG/usr/lib/talvik/lib" \
  "$PKG/usr/share/talvik" "$PKG/usr/lib/systemd/system" "$PKG/usr/lib/sysusers.d" \
  "$PKG/usr/lib/tmpfiles.d" "$PKG/etc/talvik" "$PKG/usr/bin"
install -m 0755 "$TALVIKD" "$PKG/usr/lib/talvik/bin/talvikd"
install -m 0755 "$TALVIK" "$PKG/usr/lib/talvik/bin/talvik"
cp -a "$NATIVE_LIB_DIR"/. "$PKG/usr/lib/talvik/lib/"
install -m 0644 "$ROOT/packaging/talvik/talvikd.service" "$PKG/usr/lib/systemd/system/talvikd.service"
install -m 0644 "$ROOT/packaging/talvik/config.yaml" "$PKG/etc/talvik/config.yaml"
install -m 0644 "$ROOT/packaging/talvik/default-model.yaml" "$PKG/usr/share/talvik/default-model.yaml"
install -m 0644 "$ROOT/packaging/sysusers/talvik.conf" "$PKG/usr/lib/sysusers.d/talvik.conf"
install -m 0644 "$ROOT/packaging/tmpfiles/talvik.conf" "$PKG/usr/lib/tmpfiles.d/talvik.conf"
ln -s /usr/lib/talvik/bin/talvik "$PKG/usr/bin/talvik"
ln -s /usr/lib/talvik/bin/talvikd "$PKG/usr/bin/talvikd"
cat >"$PKG/DEBIAN/control" <<EOF
Package: talvik
Version: $VERSION
Section: utils
Priority: optional
Architecture: $ARCH
Maintainer: Talvik Project
Depends: libc6 (>= 2.35), libstdc++6, systemd | systemd-sysv
Description: Talvik local GGUF model runner
 Standalone CPU-first local model serving daemon and CLI.
EOF
cat >"$PKG/DEBIAN/conffiles" <<'EOF'
/etc/talvik/config.yaml
EOF
cat >"$PKG/DEBIAN/postinst" <<'EOF'
#!/bin/sh
set -eu
command -v systemd-sysusers >/dev/null 2>&1 && systemd-sysusers || true
command -v systemd-tmpfiles >/dev/null 2>&1 && systemd-tmpfiles --create || true
command -v systemctl >/dev/null 2>&1 && systemctl daemon-reload || true
exit 0
EOF
cat >"$PKG/DEBIAN/prerm" <<'EOF'
#!/bin/sh
set -eu
if command -v systemctl >/dev/null 2>&1 && [ "$1" = remove ]; then systemctl disable --now talvikd.service >/dev/null 2>&1 || true; fi
exit 0
EOF
cat >"$PKG/DEBIAN/postrm" <<'EOF'
#!/bin/sh
set -eu
command -v systemctl >/dev/null 2>&1 && systemctl daemon-reload || true
exit 0
EOF
chmod 0755 "$PKG/DEBIAN/postinst" "$PKG/DEBIAN/prerm" "$PKG/DEBIAN/postrm"
install -d "$(dirname "$OUTPUT")"
dpkg-deb --build --root-owner-group "$PKG" "$OUTPUT" >/dev/null
sha256sum "$OUTPUT"
echo "created $OUTPUT"
