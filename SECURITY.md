# Security Policy

Security is a core design constraint because OID can eventually coordinate privileged Linux operations.

## Reporting a vulnerability

Please do not disclose exploitable vulnerabilities in a public issue. Use the repository's private security reporting channel when one is configured, or contact the maintainers privately with a minimal reproduction, affected versions/commits, impact, and suggested mitigation.

Do not include secrets or personal data in a report. We will acknowledge reports, investigate scope, coordinate a fix, and publish an advisory when appropriate.

## Design expectations

Changes that execute system operations, cross privilege boundaries, load plugins, persist evidence, or integrate with D-Bus/systemd must document threat assumptions and failure behavior. OID must preserve explicit authorization, least privilege, user-visible plans, and verifiable rollback paths.

