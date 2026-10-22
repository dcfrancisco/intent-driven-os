#!/bin/sh
set -eu

# Run inside the booted VM. This is intentionally evidence-oriented: it fails
# rather than turning missing model, offline, or persistence checks into passes.
report=${1:-/var/lib/oid/qualification/boot-report.json}
mkdir -p "$(dirname "$report")"
status=passed
reason=qualified
booted_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)
systemctl is-active --quiet talvikd.service || { status=failed; reason=talvikd-not-active; }
systemctl is-active --quiet oid-readiness.service || { status=failed; reason=readiness-not-active; }
test -S /run/marina/marina.sock || { status=failed; reason=marina-socket-missing; }
test -f /run/oid/readiness || { status=failed; reason=readiness-record-missing; }
systemctl is-enabled --quiet talvikd.service || { status=failed; reason=talvikd-not-enabled; }
systemctl is-enabled --quiet oid-readiness.service || { status=failed; reason=readiness-not-enabled; }
systemctl is-enabled --quiet oid-console.service || { status=failed; reason=oid-console-not-enabled; }
id marina >/dev/null 2>&1 || { status=failed; reason=marina-user-missing; }
id oid >/dev/null 2>&1 || { status=failed; reason=oid-user-missing; }
test -d /var/lib/marina/models || { status=failed; reason=marina-model-directory-missing; }
test -d /var/lib/oid/evidence || { status=failed; reason=oid-evidence-directory-missing; }
state=$(sed -n 's/^state=//p' /run/oid/readiness | head -1)
if [ "$state" != ready ] && [ "$state" != degraded ]; then status=failed; reason=invalid-readiness-state; fi
if command -v systemd-analyze >/dev/null 2>&1; then
    systemd-analyze verify /usr/lib/systemd/system/talvikd.service /usr/lib/systemd/system/oid-readiness.service /usr/lib/systemd/system/oid-console.service >/dev/null 2>&1 || { status=failed; reason=unit-verification-failed; }
fi
printf '{"schema":"oid.image.qualification/v1","status":"%s","reason":"%s","booted_at":"%s","readiness":"%s","network_required_at_boot":false,"model_inference":"unverified","governed_operation":"unverified","evidence_reboot_persistence":"unverified"}\n' "$status" "$reason" "$booted_at" "$state" | tee "$report"
[ "$status" = passed ]
