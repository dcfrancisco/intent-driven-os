# WP-0115: MarinaOS local IPC

## Outcome

Run one marinaosd service as the marinaos user and expose read-only versioned
status, doctor, hardware, and version requests over
/run/marinaos/marinaos.sock. marinaosctl is the client and reports daemon
absence as an actionable failure.

## Evidence required

Test socket permissions, malformed and unsupported requests, restart recovery,
offline operation, and state durability on Ubuntu 24.04. The
scripts/smoke-marinaos-ipc.sh test now proves daemon/client exchange,
malformed request rejection, and invalid configuration diagnostics. Current
status is SOURCE_PRESENT / UNIT_TESTED; Ubuntu VM execution remains NOT TESTED.
