# WP-RUNTIME-002 — Talvik Daemon and Client

Status: initial local IPC boundary implemented (2026-09-05)

## Commands

Start the persistent runtime:

```bash
cargo run -p oid-console --bin talvikd
```

Use a separate terminal for the client:

```bash
cargo run -p oid-console --bin talvik -- status
cargo run -p oid-console --bin talvik -- models
cargo run -p oid-console --bin talvik -- load <model-id>
cargo run -p oid-console --bin talvik -- run <model-id> "The capital of France is"
```

The socket defaults to `$HOME/.marina/marina.sock` and can be overridden with
`MARINA_SOCKET`. Talvik creates the socket parent directory and owns one
`Runtime::start`-created runtime instance. Each request is handled in a
separate thread against that shared service. Generation tokens are flushed as
they arrive. A client Ctrl+C sends a separate `cancel` request, allowing the
daemon and loaded model to remain alive after the client exits.

The newline protocol is deliberately small and local. Networking, authentication,
gRPC, remote clients, and daemon supervision are out of scope for this first
boundary proof. The existing interactive `oid-console` remains available as a
legacy in-process client until a later migration.
