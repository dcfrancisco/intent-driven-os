# Talvik reports

The [2026-11-01 status report](TALVIK-STATUS-2026-11-01.md) is the current assessment of implementation, tests, installation, Agent behavior, performance and RC gates. The [manual qualification runbook](TALVIK-QUALIFICATION-RUNBOOK.md) defines the next checks. Dates use Asia/Manila unless an original log states otherwise.

## Current evidence

- Source implementation: `5d5925b`, branch `feature/qwen-coding-assistants`, pushed to origin.
- [Source versus installation snapshot](evidence/source-install-snapshot-2026-11-01.json).
- [Evidence inventory and SHA-256 hashes](evidence/manifest.json).
- [Native Jinja workspace results](evidence/native-jinja/talvik-jinja-workspace.log).
- [Native Jinja strict Clippy results](evidence/native-jinja/talvik-jinja-clippy.log).

## Historical investigations

These snapshots preserve the earlier findings, including rejected candidates. Their installation, authentication, test counts, and RC statements must not be read as current status.

- [MILESTONE](historical/MILESTONE.md)
- [TALVIK-ACTION-FIX-2026-11-01](historical/TALVIK-ACTION-FIX-2026-11-01.md)
- [TALVIK-API-RC-REPORT-2026-10-31](historical/TALVIK-API-RC-REPORT-2026-10-31.md)
- [TALVIK-LATENCY-REPORT-2026-10-31](historical/TALVIK-LATENCY-REPORT-2026-10-31.md)
- [TALVIK-LOADED-PROFILES-REPORT-2026-10-31](historical/TALVIK-LOADED-PROFILES-REPORT-2026-10-31.md)
- [TALVIK-MODEL-ADAPTATION-STUDY-2026-10-31](historical/TALVIK-MODEL-ADAPTATION-STUDY-2026-10-31.md)
- [TALVIK-TEST-REPORT-2026-10-31](historical/TALVIK-TEST-REPORT-2026-10-31.md)
- [TALVIK-VSCODE-FIX-REPORT-2026-11-01](historical/TALVIK-VSCODE-FIX-REPORT-2026-11-01.md)

Historical JSON and logs are preserved under `evidence/historical`. They are not new runs against `5d5925b`. Native logs are under `evidence/native-jinja`. No model weights, API token files, VS Code secret storage, release archives or screenshots are included. Known bearer credentials are redacted from imported text. The original local artifact directory also contains developer probe scripts and packaged binaries; those are not canonical source or current release artifacts.
