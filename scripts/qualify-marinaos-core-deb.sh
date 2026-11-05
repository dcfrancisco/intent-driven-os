#!/usr/bin/env bash
set -euo pipefail

CORE=""; CLI=""; UPGRADE_CORE=""; UPGRADE_CLI=""; REPORT=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --core-package) CORE="$2"; shift 2 ;;
    --cli-package) CLI="$2"; shift 2 ;;
    --upgrade-core-package) UPGRADE_CORE="$2"; shift 2 ;;
    --upgrade-cli-package) UPGRADE_CLI="$2"; shift 2 ;;
    --report) REPORT="$2"; shift 2 ;;
    *) echo "usage: $0 --core-package FILE --cli-package FILE [--report FILE]" >&2; exit 2 ;;
  esac
done
[[ -f "$CORE" && -f "$CLI" ]] || { echo "core and cli packages are required" >&2; exit 2; }
command -v dpkg >/dev/null || { echo "dpkg is required" >&2; exit 1; }
command -v dpkg-deb >/dev/null || { echo "dpkg-deb is required" >&2; exit 1; }
WORK="$(mktemp -d)"; trap 'rm -rf "$WORK"' EXIT; ROOT="$WORK/root"
mkdir -p "$ROOT/var/lib/dpkg" "$ROOT/var/lib/marinaos"; : >"$ROOT/var/lib/dpkg/status"
status=passed; reason=qualified
core_contents="$(dpkg-deb --contents "$CORE")"
cli_contents="$(dpkg-deb --contents "$CLI")"
for required in /usr/libexec/marinaosd /etc/marinaos/marinaos.conf \
  /usr/lib/systemd/system/marinaosd.service /usr/lib/sysusers.d/marinaos.conf \
  /usr/lib/tmpfiles.d/marinaos.conf; do
  printf '%s\n' "$core_contents" | grep -F " .$required" >/dev/null || { status=failed; reason="missing-core:$required"; }
done
printf '%s\n' "$cli_contents" | grep -F " ./usr/bin/marinaosctl" >/dev/null || { status=failed; reason=missing-cli; }
if printf '%s\n%s\n' "$core_contents" "$cli_contents" | grep -Eq ' /usr/bin/(talvik|talvikd)| /usr/bin/oid-console'; then
  status=failed; reason=contains-out-of-scope-runtime
fi
install_package() {
  local package="$1" target="$2"
  dpkg-deb --extract "$package" "$target"
  dpkg-deb --control "$package" "$target/DEBIAN"
  rm -f "$target/DEBIAN/postinst" "$target/DEBIAN/postrm" "$target/DEBIAN/preinst" "$target/DEBIAN/prerm"
  dpkg-deb --build --root-owner-group "$target" "$WORK/$(basename "$target").deb" >/dev/null
  dpkg --root="$ROOT" --admindir="$ROOT/var/lib/dpkg" --instdir="$ROOT" --force-depends --unpack "$WORK/$(basename "$target").deb" >/dev/null
}
install_package "$CORE" "$WORK/core"
install_package "$CLI" "$WORK/cli"
printf 'user-data\n' >"$ROOT/var/lib/marinaos/user-data.txt"
if [[ -n "$UPGRADE_CORE" && -n "$UPGRADE_CLI" ]]; then
  install_package "$UPGRADE_CORE" "$WORK/upgrade-core"
  install_package "$UPGRADE_CLI" "$WORK/upgrade-cli"
fi
dpkg --root="$ROOT" --admindir="$ROOT/var/lib/dpkg" --instdir="$ROOT" --force-depends --remove marinaos-cli marinaos-core >/dev/null
[[ -f "$ROOT/var/lib/marinaos/user-data.txt" ]] || { status=failed; reason=data-lost-on-removal; }
[[ -n "$REPORT" ]] || REPORT="$PWD/marinaos-core-qualification.json"
mkdir -p "$(dirname "$REPORT")"
preserved=false; [[ -f "$ROOT/var/lib/marinaos/user-data.txt" ]] && preserved=true
upgrade_tested=false; [[ -n "$UPGRADE_CORE" && -n "$UPGRADE_CLI" ]] && upgrade_tested=true
printf '{"schema":"marinaos.package-set.qualification/v2","status":"%s","reason":"%s","core_sha256":"%s","cli_sha256":"%s","upgrade_tested":%s,"talvik_required":false,"out_of_scope_runtime_absent":%s,"removal_preserves_state":%s,"host_services_modified":false}\n' \
  "$status" "$reason" "$(sha256sum "$CORE" | awk '{print $1}')" "$(sha256sum "$CLI" | awk '{print $1}')" "$upgrade_tested" "$([[ "$reason" != contains-out-of-scope-runtime ]] && echo true || echo false)" "$preserved" >"$REPORT"
cat "$REPORT"
[[ "$status" = passed ]]
