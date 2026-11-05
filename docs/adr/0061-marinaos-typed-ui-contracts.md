# ADR-0061: MarinaOS typed UI contracts and asynchronous events

- Status: Accepted for initial implementation
- Related: ADR-0039, ADR-0041, ADR-0057, WP-0115, WP-0116

UI adapters consume versioned typed contracts for health, hardware, intent,
lifecycle, approval, execution, verification, evidence, and Talvik availability.
Local IPC is asynchronous from GTK. The current foundation protocol provides
only status, doctor, hardware, and version; all other adapters report
unavailable rather than fabricate data.
