# MarinaOS ADR and work-package traceability

The uploaded design package proposed ADR-0053..0062 and WP-0112..0124. The
repository already had WP-0112 and WP-0113 assigned on this branch, so those
identifiers are not silently reused. This table is the authoritative mapping
until a future migration deliberately renames the files.

| Design package item | Repository record | Status |
| --- | --- | --- |
| ADR-0053 product boundary | docs/adr/0053-marinaos-independent-linux-foundation.md | Proposed for review |
| ADR-0054 Ubuntu baseline | Existing ADR-0050 plus WP-0113 | Source present |
| ADR-0055 package composition | docs/adr/0056-marinaos-package-composition.md | Proposed |
| ADR-0056 readiness/degraded behavior | ADR-0053 and packaging/marinaos/marinaosd.service | Source present |
| ADR-0057 local IPC | docs/adr/0057-marinaos-local-ipc.md | Proposed |
| WP-0112 inventory | INVENTORY.md | Source present; identifier retained by existing implementation WP |
| WP-0113 Ubuntu baseline | docs/work-packages/WP-0113-marinaos-ubuntu-vm-qualification.md | Source present; VM not tested |
| WP-0114 package split | docs/work-packages/WP-0114-marinaos-package-split.md | Source present |
| WP-0115 daemon/IPC | docs/work-packages/WP-0115-marinaos-local-ipc.md | Source present |

No record is BUILT, VM_TESTED, HARDWARE_TESTED, or RELEASED without the
artifact hash and execution log required by the design package.
