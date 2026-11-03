#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
VERSION=0.1.0
RELEASE=1
TALVIKD="$ROOT/target/release/talvikd"
TALVIK="$ROOT/target/release/talvik"
NATIVE_LIB_DIR=""
OUTPUT_DIR="$ROOT/artifacts"

while (($#)); do
  case "$1" in
    --version) VERSION=$2; shift 2 ;;
    --release) RELEASE=$2; shift 2 ;;
    --talvikd) TALVIKD=$2; shift 2 ;;
    --talvik) TALVIK=$2; shift 2 ;;
    --native-lib-dir) NATIVE_LIB_DIR=$2; shift 2 ;;
    --output-dir) OUTPUT_DIR=$2; shift 2 ;;
    --help|-h) echo "Usage: package-talvik-rpm.sh --native-lib-dir PATH [--version VERSION]"; exit 0 ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
done

command -v rpmbuild >/dev/null || { echo "rpmbuild is required" >&2; exit 1; }
[[ -x "$TALVIKD" && -x "$TALVIK" ]] || { echo "release talvik and talvikd binaries are required" >&2; exit 1; }
[[ -n "$NATIVE_LIB_DIR" && -d "$NATIVE_LIB_DIR" ]] || { echo "--native-lib-dir is required" >&2; exit 1; }
for component in llama ggml ggml-base ggml-cpu mtmd oid-chat-template; do
  [[ -n "$(find "$NATIVE_LIB_DIR" -maxdepth 1 -type f -name "lib${component}.so*" -print -quit)" ]] || {
    echo "incomplete native payload: lib${component}.so is missing from $NATIVE_LIB_DIR" >&2; exit 1;
  }
done

WORK=$(mktemp -d "${TMPDIR:-/tmp}/talvik-rpm.XXXXXX")
trap 'rm -rf "$WORK"' EXIT
TOP="$WORK/rpmbuild"
install -d "$TOP/BUILD" "$TOP/BUILDROOT" "$TOP/RPMS" "$TOP/SOURCES" "$TOP/SPECS" "$TOP/SRPMS"
install -m 0755 "$TALVIKD" "$TOP/SOURCES/talvikd"
install -m 0755 "$TALVIK" "$TOP/SOURCES/talvik"
tar -C "$NATIVE_LIB_DIR" -czf "$TOP/SOURCES/talvik-libs.tar.gz" .
cp "$ROOT/packaging/talvik/talvikd.service" "$TOP/SOURCES/talvikd.service"
cp "$ROOT/packaging/talvik/config.yaml" "$TOP/SOURCES/config.yaml"
cp "$ROOT/packaging/sysusers/talvik.conf" "$TOP/SOURCES/talvik.conf"
cp "$ROOT/packaging/tmpfiles/talvik.conf" "$TOP/SOURCES/talvik-tmpfiles.conf"
cp "$ROOT/packaging/talvik/default-model.yaml" "$TOP/SOURCES/default-model.yaml"
cat >"$TOP/SPECS/talvik.spec" <<EOF
Name: talvik
Version: $VERSION
Release: $RELEASE
Summary: Talvik local GGUF model runner
License: Apache-2.0
BuildArch: x86_64
Requires: systemd
Requires: glibc >= 2.35
Requires: libstdc++.so.6()(64bit)

%description
Standalone CPU-first local GGUF model runner and daemon.

%prep

%build

%install
install -D -m 0755 %{_sourcedir}/talvik %{buildroot}/usr/lib/talvik/bin/talvik
install -D -m 0755 %{_sourcedir}/talvikd %{buildroot}/usr/lib/talvik/bin/talvikd
install -D -m 0644 %{_sourcedir}/talvikd.service %{buildroot}/usr/lib/systemd/system/talvikd.service
install -D -m 0644 %{_sourcedir}/config.yaml %{buildroot}/etc/talvik/config.yaml
install -D -m 0644 %{_sourcedir}/talvik.conf %{buildroot}/usr/lib/sysusers.d/talvik.conf
install -D -m 0644 %{_sourcedir}/talvik-tmpfiles.conf %{buildroot}/usr/lib/tmpfiles.d/talvik.conf
install -D -m 0644 %{_sourcedir}/default-model.yaml %{buildroot}/usr/share/talvik/default-model.yaml
mkdir -p %{buildroot}/usr/lib/talvik/lib %{buildroot}/usr/bin
tar -xzf %{_sourcedir}/talvik-libs.tar.gz -C %{buildroot}/usr/lib/talvik/lib
ln -s /usr/lib/talvik/bin/talvik %{buildroot}/usr/bin/talvik
ln -s /usr/lib/talvik/bin/talvikd %{buildroot}/usr/bin/talvikd

%files
/usr/bin/talvik
/usr/bin/talvikd
/usr/lib/talvik
/usr/lib/systemd/system/talvikd.service
/usr/lib/sysusers.d/talvik.conf
/usr/lib/tmpfiles.d/talvik.conf
%config(noreplace) /etc/talvik/config.yaml
/usr/share/talvik/default-model.yaml

%post
if command -v systemd-sysusers >/dev/null 2>&1; then systemd-sysusers || :; fi
if command -v systemd-tmpfiles >/dev/null 2>&1; then systemd-tmpfiles --create || :; fi
if command -v systemctl >/dev/null 2>&1; then systemctl daemon-reload || :; fi

%preun
if [ "$1" -eq 0 ] && command -v systemctl >/dev/null 2>&1; then
  systemctl disable --now talvikd.service >/dev/null 2>&1 || :
fi

%postun
if command -v systemctl >/dev/null 2>&1; then systemctl daemon-reload || :; fi
EOF
rpmbuild --define "_topdir $TOP" -bb "$TOP/SPECS/talvik.spec" >/dev/null
install -d "$OUTPUT_DIR"
find "$TOP/RPMS" -type f -name '*.rpm' -exec cp {} "$OUTPUT_DIR/" \;
sha256sum "$OUTPUT_DIR"/talvik-*.rpm
