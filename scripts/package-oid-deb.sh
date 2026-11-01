#!/usr/bin/env bash
set -euo pipefail

# Build the first Debian/Ubuntu system-profile artifact. This packages the
# existing Talvik and OID service assets; it does not create a second service
# implementation or bundle model data.
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="${VERSION:-0.1.0}"
OUTPUT="${OUTPUT:-$ROOT/artifacts/oid-marina_${VERSION}_amd64.deb}"
TALVIKD=""; TALVIK=""; OID_CONSOLE=""; NATIVE_LIB_DIR=""

usage() {
  echo "usage: $0 --talvikd PATH --talvik PATH --oid-console PATH --native-lib-dir DIR [--version VERSION] [--output PATH]" >&2
  exit 2
}
while [[ $# -gt 0 ]]; do
  case "$1" in
    --talvikd) TALVIKD="$2"; shift 2;;
    --talvik) TALVIK="$2"; shift 2;;
    --oid-console) OID_CONSOLE="$2"; shift 2;;
    --native-lib-dir) NATIVE_LIB_DIR="$2"; shift 2;;
    --version) VERSION="$2"; shift 2;;
    --output) OUTPUT="$2"; shift 2;;
    --help|-h) usage;;
    *) echo "unknown option: $1" >&2; usage;;
  esac
done
[[ -x "$TALVIKD" && -x "$TALVIK" && -x "$OID_CONSOLE" && -d "$NATIVE_LIB_DIR" ]] || usage
command -v dpkg-deb >/dev/null || { echo "dpkg-deb is required" >&2; exit 1; }
[[ "$VERSION" =~ ^[0-9][0-9A-Za-z.+~:-]*$ ]] || { echo "invalid Debian version: $VERSION" >&2; exit 2; }

for file in "$ROOT/packaging/marina/config.yaml" "$ROOT/packaging/marina/talvikd.service" \
  "$ROOT/packaging/oid/oid-readiness.service" "$ROOT/packaging/oid/oid-console.service" \
  "$ROOT/packaging/oid/oid-readiness" "$ROOT/packaging/sysusers/marina.conf" \
  "$ROOT/packaging/sysusers/oid.conf" "$ROOT/packaging/tmpfiles/marina.conf" \
  "$ROOT/packaging/tmpfiles/oid.conf"; do
  [[ -f "$file" ]] || { echo "missing packaging asset: $file" >&2; exit 1; }
done

WORK="$(mktemp -d)"; trap 'rm -rf "$WORK"' EXIT; PKG="$WORK/pkg"
mkdir -p "$PKG/DEBIAN" "$PKG/usr/bin" "$PKG/usr/lib/marina" \
  "$PKG/usr/lib/systemd/system" "$PKG/usr/lib/sysusers.d" "$PKG/usr/lib/tmpfiles.d" \
  "$PKG/usr/libexec" "$PKG/etc/marina" "$PKG/etc/oid"
install -m 0755 "$TALVIKD" "$PKG/usr/bin/talvikd"
install -m 0755 "$TALVIK" "$PKG/usr/bin/talvik"
install -m 0755 "$OID_CONSOLE" "$PKG/usr/bin/oid-console"
find "$NATIVE_LIB_DIR" -maxdepth 1 \( -type f -o -type l \) \
  \( -name 'libllama.so*' -o -name 'libggml*.so*' -o -name 'libmtmd.so*' -o -name 'liboid-chat-template.so*' \) -exec cp -a {} "$PKG/usr/lib/marina/" \;
[[ -e "$PKG/usr/lib/marina/libllama.so" || -e "$PKG/usr/lib/marina/libllama.so.0" ]] || {
  echo "native payload does not contain libllama.so" >&2; exit 1;
}
for component in ggml ggml-base ggml-cpu mtmd oid-chat-template; do
  [[ -n "$(find "$PKG/usr/lib/marina" -maxdepth 1 -type f -name "lib${component}.so*" -print -quit)" ]] || {
    echo "native payload does not contain lib${component}.so" >&2; exit 1;
  }
done
install -m 0644 "$ROOT/packaging/marina/config.yaml" "$PKG/etc/marina/config.yaml"
install -m 0640 /dev/null "$PKG/etc/oid/marina-client.env"
install -m 0644 "$ROOT/packaging/marina/talvikd.service" "$PKG/usr/lib/systemd/system/"
install -m 0644 "$ROOT/packaging/oid/oid-readiness.service" "$PKG/usr/lib/systemd/system/"
install -m 0644 "$ROOT/packaging/oid/oid-console.service" "$PKG/usr/lib/systemd/system/"
install -m 0644 "$ROOT/packaging/sysusers/marina.conf" "$PKG/usr/lib/sysusers.d/"
install -m 0644 "$ROOT/packaging/sysusers/oid.conf" "$PKG/usr/lib/sysusers.d/"
install -m 0644 "$ROOT/packaging/tmpfiles/marina.conf" "$PKG/usr/lib/tmpfiles.d/"
install -m 0644 "$ROOT/packaging/tmpfiles/oid.conf" "$PKG/usr/lib/tmpfiles.d/"
install -m 0755 "$ROOT/packaging/oid/oid-readiness" "$PKG/usr/libexec/oid-readiness"
install -m 0755 "$ROOT/scripts/install-oid-image-model.sh" "$PKG/usr/libexec/install-oid-image-model"
cat >"$PKG/DEBIAN/control" <<EOF
Package: oid-marina
Version: $VERSION
Section: misc
Priority: optional
Architecture: amd64
Maintainer: Open Intelligence Desktop contributors
Depends: libc6 (>= 2.35), systemd | systemd-sysv
Description: OID governed runtime and Talvik local model runner
 Versioned native Linux runtime containing Talvik, the OID console, and the
 pinned relocatable llama.cpp backend. Models, evidence, and mutable state are
 intentionally outside the package payload.
EOF
cat >"$PKG/DEBIAN/conffiles" <<'EOF'
/etc/marina/config.yaml
/etc/oid/marina-client.env
EOF
cat >"$PKG/DEBIAN/postinst" <<'EOF'
#!/bin/sh
set -eu
if command -v systemd-sysusers >/dev/null 2>&1; then systemd-sysusers || true; fi
if command -v systemd-tmpfiles >/dev/null 2>&1; then systemd-tmpfiles --create || true; fi
if command -v systemctl >/dev/null 2>&1; then systemctl daemon-reload || true; fi
exit 0
EOF
cat >"$PKG/DEBIAN/postrm" <<'EOF'
#!/bin/sh
set -eu
if command -v systemctl >/dev/null 2>&1; then systemctl daemon-reload || true; fi
exit 0
EOF
chmod 0755 "$PKG/DEBIAN/postinst" "$PKG/DEBIAN/postrm"
mkdir -p "$(dirname "$OUTPUT")"
dpkg-deb --build --root-owner-group "$PKG" "$OUTPUT" >/dev/null
sha256sum "$OUTPUT" >"$OUTPUT.sha256"
printf '%s\n' "created $OUTPUT" "checksum $(cat "$OUTPUT.sha256")"
