#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="${VERSION:-0.1.0}"
OUTPUT_DIR="${OUTPUT_DIR:-$ROOT/artifacts}"
MARINAOSCTL="${MARINAOSCTL:-$ROOT/target/release/marinaosctl}"
MARINAOSD="${MARINAOSD:-$ROOT/target/release/marinaosd}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version) VERSION="$2"; shift 2 ;;
    --output-dir) OUTPUT_DIR="$2"; shift 2 ;;
    *) echo "usage: $0 [--version VERSION] [--output-dir DIR]" >&2; exit 2 ;;
  esac
done

command -v dpkg-deb >/dev/null || { echo "dpkg-deb is required" >&2; exit 1; }
[[ -x "$MARINAOSCTL" ]] || { echo "marinaosctl release binary is required: $MARINAOSCTL" >&2; exit 1; }
[[ -x "$MARINAOSD" ]] || { echo "marinaosd release binary is required: $MARINAOSD" >&2; exit 1; }
[[ "$VERSION" =~ ^[0-9][0-9A-Za-z.+~:-]*$ ]] || { echo "invalid Debian version: $VERSION" >&2; exit 2; }

for asset in packaging/marinaos/marinaos.conf packaging/marinaos/marinaosd.service \
  packaging/sysusers/marinaos.conf packaging/tmpfiles/marinaos.conf; do
  [[ -f "$ROOT/$asset" ]] || { echo "missing packaging asset: $asset" >&2; exit 1; }
done

build_package() {
  local name="$1" binary="$2" description="$3"
  local work pkg output
  work="$(mktemp -d)"
  pkg="$work/pkg"
  output="$OUTPUT_DIR/${name}_${VERSION}_amd64.deb"
  mkdir -p "$pkg/DEBIAN" "$pkg/usr/bin" "$pkg/usr/libexec" "$pkg/usr/lib/systemd/system" \
    "$pkg/usr/lib/sysusers.d" "$pkg/usr/lib/tmpfiles.d" "$pkg/etc/marinaos"
  if [[ "$name" = marinaos-core ]]; then
    install -m 0755 "$MARINAOSD" "$pkg/usr/libexec/marinaosd"
    install -m 0644 "$ROOT/packaging/marinaos/marinaosd.service" "$pkg/usr/lib/systemd/system/"
    install -m 0644 "$ROOT/packaging/marinaos/marinaos.conf" "$pkg/etc/marinaos/marinaos.conf"
    install -m 0644 "$ROOT/packaging/sysusers/marinaos.conf" "$pkg/usr/lib/sysusers.d/"
    install -m 0644 "$ROOT/packaging/tmpfiles/marinaos.conf" "$pkg/usr/lib/tmpfiles.d/"
    depends='Depends: libc6 (>= 2.35), systemd | systemd-sysv'
  else
    install -m 0755 "$MARINAOSCTL" "$pkg/usr/bin/marinaosctl"
    depends='Depends: marinaos-core (= '"$VERSION"')'
  fi
  cat >"$pkg/DEBIAN/control" <<EOF
Package: $name
Version: $VERSION
Section: admin
Priority: optional
Architecture: amd64
Maintainer: MarinaOS contributors
$depends
Description: $description
Talvik is optional and is not bundled or required.
EOF
  if [[ "$name" = marinaos-core ]]; then
    printf '/etc/marinaos/marinaos.conf\n' >"$pkg/DEBIAN/conffiles"
  fi
  cat >"$pkg/DEBIAN/postinst" <<'EOF'
#!/bin/sh
set -eu
command -v systemd-sysusers >/dev/null 2>&1 && systemd-sysusers || true
command -v systemd-tmpfiles >/dev/null 2>&1 && systemd-tmpfiles --create || true
command -v systemctl >/dev/null 2>&1 && systemctl daemon-reload || true
exit 0
EOF
  cat >"$pkg/DEBIAN/postrm" <<'EOF'
#!/bin/sh
set -eu
command -v systemctl >/dev/null 2>&1 && systemctl daemon-reload || true
exit 0
EOF
  cat >"$pkg/DEBIAN/prerm" <<'EOF'
#!/bin/sh
set -eu
if command -v systemctl >/dev/null 2>&1 && [ "$1" = remove ]; then
  systemctl disable --now marinaosd.service >/dev/null 2>&1 || true
fi
exit 0
EOF
  chmod 0755 "$pkg/DEBIAN/postinst" "$pkg/DEBIAN/postrm" "$pkg/DEBIAN/prerm"
  if [[ -n "${SOURCE_DATE_EPOCH:-}" ]]; then
    find "$pkg" -exec touch --date "${SOURCE_DATE_EPOCH}" {} +
  fi
  mkdir -p "$OUTPUT_DIR"
  dpkg-deb --build --root-owner-group "$pkg" "$output" >/dev/null
  sha256sum "$output" >"$output.sha256"
  rm -rf "$work"
  printf 'created %s\n' "$output"
  cat "$output.sha256"
}

build_package marinaos-core "$MARINAOSD" "MarinaOS Linux foundation daemon"
build_package marinaos-cli "$MARINAOSCTL" "MarinaOS Linux foundation command-line client"
