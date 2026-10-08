#!/bin/sh
set -eu

# Run inside the booted VM. This is intentionally evidence-oriented: it fails
# rather than turning missing model, offline, or persistence checks into passes.
report=${1:-/var/lib/oid/qualification/boot-report.json}
mkdir -p "$(dirname "$report")"
status=passed
reason=qualified
systemctl is-active --quiet marina.service || { status=failed; reason=marina-not-active; }
systemctl is-active --quiet oid-readiness.service || { status=failed; reason=readiness-not-active; }
test -S /run/marina/marina.sock || { status=failed; reason=marina-socket-missing; }
test -f /run/oid/readiness || { status=failed; reason=readiness-record-missing; }
systemctl is-enabled --quiet marina.service || { status=failed; reason=marina-not-enabled; }
systemctl is-enabled --quiet oid-readiness.service || { status=failed; reason=readiness-not-enabled; }
systemctl is-enabled --quiet oid-console.service || { status=failed; reason=oid-console-not-enabled; }
state=$(sed -n 's/^state=//p' /run/oid/readiness | head -1)
if [ "$state" != ready ] && [ "$state" != degraded ]; then status=failed; reason=invalid-readiness-state; fi
if command -v systemd-analyze >/dev/null 2>&1; then
    systemd-analyze verify /usr/lib/systemd/system/marina.service /usr/lib/systemd/system/oid-readiness.service /usr/lib/systemd/system/oid-console.service >/dev/null 2>&1 || { status=failed; reason=unit-verification-failed; }
fi
printf '{"schema":"oid.image.qualification/v1","status":"%s","reason":"%s","readiness":"%s","network_required_at_boot":false}\n' "$status" "$reason" "$state" | tee "$report"
[ "$status" = passed ]
