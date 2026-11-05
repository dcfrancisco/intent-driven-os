# marina-desktop

The Linux user-session MarinaOS desktop shell. Build on a supported Linux host
with GTK 4 and libadwaita development packages:

```sh
cargo run --manifest-path apps/marina-desktop/Cargo.toml
```

Set `MARINAOS_SOCKET` to point at a development `marinaosd` socket. The app
uses only the versioned read-only status and hardware methods currently exposed
by that daemon. It deliberately reports intent submission as unavailable until
the authenticated governed API is implemented.
