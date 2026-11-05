# ADR-0065: MarinaOS independent UI packaging and recovery

- Status: Accepted for packaging design
- Related: ADR-0051, ADR-0052, ADR-0056, ADR-0058, WP-0114, WP-0116

The desktop ships as a separate package with a desktop entry, icon, dependency
declarations, optional user-session startup, and reversible upgrade/uninstall.
It never modifies the default session, downloads models, or enables privileged
execution during installation.
