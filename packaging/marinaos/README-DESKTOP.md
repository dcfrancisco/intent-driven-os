# MarinaOS desktop package

The desktop package installs `marina-desktop`, the desktop entry, and the
MarinaOS icon. It depends on GTK 4, libadwaita, and the supported GNOME/Wayland
runtime. It does not depend on Talvik and does not enable a system service.

The optional user-session unit is enabled explicitly by the user:

```sh
systemctl --user enable --now marina-desktop.service
```

Upgrade replaces only the desktop binary and metadata. Uninstall removes the
desktop process, unit, entry, and icon; it preserves `/var/lib/marinaos`,
`/run/marinaos`, evidence, and Talvik state. Package installation never starts
privileged commands or downloads models.

This package is source-present only until a native Ubuntu builder emits and
qualifies the DEB. Graphical launch must be recorded on a real supported Linux
desktop; macOS compilation is not evidence of graphical support.
