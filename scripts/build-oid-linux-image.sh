#!/bin/sh
set -eu

# Linux-only image builder. It creates a BIOS-bootable Debian bookworm raw
# disk. Run in a disposable Linux build VM as root; it does not alter a host
# installation and does not require Docker at runtime.

usage() { echo "usage: $0 --marina PATH --marinactl PATH --oid-console PATH --output PATH [--llama-lib-dir DIR] [--model FILE --model-sha256 HEX --model-id ID]" >&2; exit 2; }
marina=; marinactl=; oid_console=; output=; llama_lib_dir=; model=; model_sha=; model_id=
while [ "$#" -gt 0 ]; do
    case "$1" in
        --marina) marina=${2:?}; shift 2 ;;
        --marinactl) marinactl=${2:?}; shift 2 ;;
        --oid-console) oid_console=${2:?}; shift 2 ;;
        --output) output=${2:?}; shift 2 ;;
        --llama-lib-dir) llama_lib_dir=${2:?}; shift 2 ;;
        --model) model=${2:?}; shift 2 ;;
        --model-sha256) model_sha=${2:?}; shift 2 ;;
        --model-id) model_id=${2:?}; shift 2 ;;
        *) usage ;;
    esac
done
[ -x "$marina" ] && [ -x "$marinactl" ] && [ -x "$oid_console" ] && [ -n "$output" ] || usage
if [ -n "$model" ] && { [ -z "$model_sha" ] || [ -z "$model_id" ]; }; then usage; fi
if [ -n "$model_id" ] && [ "$model_id" != "$(basename "$model_id")" ]; then
    echo "model ID must be a filename-safe identifier: $model_id" >&2
    exit 2
fi
if [ -n "$llama_lib_dir" ] && [ ! -d "$llama_lib_dir" ]; then
    echo "llama library directory does not exist: $llama_lib_dir" >&2
    exit 1
fi

for command in debootstrap dd losetup mount umount parted mkfs.ext4 mountpoint rsync chroot blkid sha256sum awk; do
    command -v "$command" >/dev/null 2>&1 || { echo "missing Linux image tool: $command" >&2; exit 1; }
done
[ "$(id -u)" -eq 0 ] || { echo "run this builder as root in a disposable Linux build environment" >&2; exit 1; }

work=$(mktemp -d); root="$work/root"; image_root="$work/image-root"; loop=
cleanup() {
    set +e
    for mountpoint in "$image_root/dev/pts" "$image_root/dev" "$image_root/proc" "$image_root/sys" "$image_root"; do
        mountpoint -q "$mountpoint" && umount -l "$mountpoint"
    done
    [ -n "$loop" ] && losetup -d "$loop"
    rm -rf "$work"
}
trap cleanup EXIT INT TERM
mkdir -p "$root" "$image_root"
debootstrap --variant=minbase bookworm "$root" https://deb.debian.org/debian
chroot "$root" apt-get update
chroot "$root" env DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends systemd systemd-sysv linux-image-amd64 grub-pc curl ca-certificates util-linux coreutils procps iproute2

install -D -m 0755 "$marina" "$root/usr/bin/marina"
install -D -m 0755 "$marinactl" "$root/usr/bin/marinactl"
install -D -m 0755 "$oid_console" "$root/usr/bin/oid-console"
if [ -n "$llama_lib_dir" ]; then
    mkdir -p "$root/usr/lib/marina"
    # Preserve SONAME symlinks as well as regular files. Dropping a symlink
    # can leave libllama present but make its ggml dependency unloadable.
    find "$llama_lib_dir" -maxdepth 1 \( -type f -o -type l \) \( -name '*.so' -o -name '*.so.*' \) -exec cp -a {} "$root/usr/lib/marina/" \;
fi
install -D -m 0644 packaging/marina/config.yaml "$root/etc/marina/config.yaml"
install -D -m 0644 packaging/marina/marina.service "$root/usr/lib/systemd/system/marina.service"
install -D -m 0644 packaging/oid/oid-readiness.service "$root/usr/lib/systemd/system/oid-readiness.service"
install -D -m 0644 packaging/oid/oid-console.service "$root/usr/lib/systemd/system/oid-console.service"
install -D -m 0644 packaging/sysusers/marina.conf "$root/usr/lib/sysusers.d/marina.conf"
install -D -m 0644 packaging/sysusers/oid.conf "$root/usr/lib/sysusers.d/oid.conf"
install -D -m 0644 packaging/tmpfiles/marina.conf "$root/usr/lib/tmpfiles.d/marina.conf"
install -D -m 0644 packaging/tmpfiles/oid.conf "$root/usr/lib/tmpfiles.d/oid.conf"
install -D -m 0755 packaging/oid/oid-readiness "$root/usr/libexec/oid-readiness"
install -D -m 0755 scripts/install-oid-image-model.sh "$root/usr/libexec/install-oid-image-model"
install -D -m 0755 scripts/qualify-oid-image.sh "$root/usr/local/sbin/qualify-oid-image"
install -D -m 0640 /dev/null "$root/etc/oid/marina-client.env"
printf '%s\n' '# Provision OID_MARINA_TOKEN after boot; never commit a token.' >"$root/etc/oid/marina-client.env"
mkdir -p "$root/var/lib/marina/models" "$root/var/lib/oid/evidence"
chroot "$root" systemd-sysusers; chroot "$root" systemd-tmpfiles --create
chroot "$root" systemctl enable marina.service oid-readiness.service oid-console.service
if [ -n "$model" ]; then
    actual=$(sha256sum "$model" | awk '{print $1}')
    [ "$actual" = "$model_sha" ] || { echo "model checksum mismatch" >&2; exit 1; }
    install -o marina -g marina -m 0640 "$model" "$root/var/lib/marina/models/$model_id.gguf"
fi

mkdir -p "$(dirname "$output")"
truncate -s 4G "$output"
parted -s "$output" mklabel msdos mkpart primary ext4 1MiB 100% set 1 boot on
loop=$(losetup --find --show --partscan "$output")
mkfs.ext4 -F "${loop}p1" >/dev/null
mount "${loop}p1" "$image_root"
root_uuid=$(blkid -s UUID -o value "${loop}p1")
printf 'UUID=%s / ext4 defaults 0 1\n' "$root_uuid" >"$image_root/etc/fstab"
rsync -aHAX "$root/" "$image_root/"
mount --bind /dev "$image_root/dev"; mount --bind /dev/pts "$image_root/dev/pts"
mount -t proc proc "$image_root/proc"; mount -t sysfs sysfs "$image_root/sys"
chroot "$image_root" grub-install --target=i386-pc --recheck "$loop"
chroot "$image_root" grub-mkconfig -o /boot/grub/grub.cfg
sync
sha256sum "$output" >"$output.sha256"
manifest="$output.manifest"
{
    printf '%s\n' 'schema=oid.image.manifest/v1' 'image_format=debian-bookworm-amd64-bios-raw'
    printf 'image=%s\n' "$(basename "$output")"
    printf 'image_sha256=%s\n' "$(awk '{print $1}' "$output.sha256")"
    for input in "$marina" "$marinactl" "$oid_console"; do
        printf '%s_sha256=%s\n' "$(basename "$input")" "$(sha256sum "$input" | awk '{print $1}')"
    done
    if [ -n "$llama_lib_dir" ]; then
        find "$llama_lib_dir" -maxdepth 1 \( -type f -o -type l \) \( -name '*.so' -o -name '*.so.*' \) -printf '%f\n' |
            sort |
            while IFS= read -r library_name; do
                library="$llama_lib_dir/$library_name"
                printf 'native_library=%s sha256=%s\n' "$library_name" "$(sha256sum "$library" | awk '{print $1}')"
            done
    fi
    if [ -n "$model" ]; then
        printf 'model_id=%s\nmodel_sha256=%s\n' "$model_id" "$model_sha"
    else
        printf '%s\n' 'model=not-included'
    fi
} >"$manifest"
printf '%s\n' "created $output" "checksum $(cat "$output.sha256")" "manifest $manifest"
