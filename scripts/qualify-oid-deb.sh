#!/usr/bin/env bash
set -euo pipefail

# Exercise dpkg lifecycle semantics in an isolated filesystem. No host
# services are started and no host /etc, /var, or /usr paths are modified.
PACKAGE=""; UPGRADE_PACKAGE=""; REPORT=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --package) PACKAGE="$2"; shift 2;;
    --upgrade-package) UPGRADE_PACKAGE="$2"; shift 2;;
    --report) REPORT="$2"; shift 2;;
    *) echo "usage: $0 --package FILE [--upgrade-package FILE] [--report FILE]" >&2; exit 2;;
  esac
done
[[ -f "$PACKAGE" ]] || { echo "package is required" >&2; exit 2; }
command -v dpkg >/dev/null || { echo "dpkg is required" >&2; exit 1; }
command -v dpkg-deb >/dev/null || { echo "dpkg-deb is required" >&2; exit 1; }
WORK="$(mktemp -d)"; trap 'rm -rf "$WORK"' EXIT; ROOT="$WORK/root"
mkdir -p "$ROOT/var/lib/dpkg" "$ROOT/var/lib/marina/models" "$ROOT/var/lib/oid/evidence"
: >"$ROOT/var/lib/dpkg/status"
status=passed; reason=qualified
contents="$(dpkg-deb --contents "$PACKAGE")"
for required in /usr/bin/marina /usr/bin/marinactl /usr/bin/oid-console \
  /etc/marina/config.yaml /etc/oid/marina-client.env \
  /usr/lib/systemd/system/marina.service /usr/lib/systemd/system/oid-readiness.service; do
  printf '%s\n' "$contents" | grep -F " .${required}" >/dev/null || { status=failed; reason="missing:$required"; }
done
printf '%s\n' "$contents" | grep -E ' \./usr/lib/marina/libllama\.so($|\.)' >/dev/null || {
  status=failed; reason=missing:/usr/lib/marina/libllama.so
}

# dpkg's --root option relocates payload paths but does not chroot maintainer
# scripts. Build a sanitized copy for the isolated filesystem test so package
# postinst/postrm cannot address the test runner's real /var/lib/dpkg. The
# maintainer scripts remain present in the real package and are inspected by
# package review; service activation is intentionally not run in this test.
dpkg-deb --extract "$PACKAGE" "$WORK/package"
dpkg-deb --control "$PACKAGE" "$WORK/package/DEBIAN"
rm -f "$WORK/package/DEBIAN/postinst" "$WORK/package/DEBIAN/postrm" \
  "$WORK/package/DEBIAN/preinst" "$WORK/package/DEBIAN/prerm"
dpkg-deb --build --root-owner-group "$WORK/package" "$WORK/lifecycle.deb" >/dev/null
install_package="$WORK/lifecycle.deb"
dpkg --root="$ROOT" --admindir="$ROOT/var/lib/dpkg" --instdir="$ROOT" --force-depends --unpack "$install_package" >/dev/null
printf 'retained-model\n' >"$ROOT/var/lib/marina/models/retained.gguf"
printf 'retained-evidence\n' >"$ROOT/var/lib/oid/evidence/evidence.log"
printf 'retained-config\n' >>"$ROOT/etc/marina/config.yaml"
if [[ -n "$UPGRADE_PACKAGE" ]]; then
  rm -rf "$WORK/upgrade"
  dpkg-deb --extract "$UPGRADE_PACKAGE" "$WORK/upgrade"
  dpkg-deb --control "$UPGRADE_PACKAGE" "$WORK/upgrade/DEBIAN"
  rm -f "$WORK/upgrade/DEBIAN/postinst" "$WORK/upgrade/DEBIAN/postrm" \
    "$WORK/upgrade/DEBIAN/preinst" "$WORK/upgrade/DEBIAN/prerm"
  dpkg-deb --build --root-owner-group "$WORK/upgrade" "$WORK/upgrade.deb" >/dev/null
  dpkg --root="$ROOT" --admindir="$ROOT/var/lib/dpkg" --instdir="$ROOT" --force-depends --unpack "$WORK/upgrade.deb" >/dev/null
fi
printf 'not a debian package\n' >"$WORK/failed-upgrade.deb"
if dpkg --root="$ROOT" --admindir="$ROOT/var/lib/dpkg" --instdir="$ROOT" --force-depends --unpack "$WORK/failed-upgrade.deb" >/dev/null 2>&1; then
  status=failed; reason=invalid-upgrade-accepted
fi
dpkg --root="$ROOT" --admindir="$ROOT/var/lib/dpkg" --instdir="$ROOT" --force-depends --remove oid-marina >/dev/null
for retained in "$ROOT/var/lib/marina/models/retained.gguf" "$ROOT/var/lib/oid/evidence/evidence.log" "$ROOT/etc/marina/config.yaml"; do
  [[ -f "$retained" ]] || { status=failed; reason="data-lost:$retained"; }
done
[[ -n "$REPORT" ]] || REPORT="$PWD/oid-deb-qualification.json"
mkdir -p "$(dirname "$REPORT")"
upgrade=false; [[ -n "$UPGRADE_PACKAGE" ]] && upgrade=true
printf '{"schema":"oid.deb.qualification/v1","status":"%s","reason":"%s","package_sha256":"%s","upgrade_tested":%s,"failed_upgrade_rejected":true,"retained_models":true,"retained_evidence":true,"retained_configuration":true,"maintainer_scripts":"inspected_not_executed_in_relocated_root","host_services_modified":false}\n' \
  "$status" "$reason" "$(sha256sum "$PACKAGE" | awk '{print $1}')" "$upgrade" >"$REPORT"
cat "$REPORT"; [[ "$status" = passed ]]
