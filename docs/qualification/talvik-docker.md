# Talvik Docker qualification

The image is CPU-first and builds llama.cpp from the pinned commit declared in
`docker/talvik/Dockerfile`. Compose binds the HTTP port to loopback and keeps
authentication enabled. Models and state are separate named volumes.

```sh
docker compose -f docker/talvik/compose.yaml build
docker compose -f docker/talvik/compose.yaml up -d
curl --fail http://127.0.0.1:11434/healthz
docker compose -f docker/talvik/compose.yaml ps
docker compose -f docker/talvik/compose.yaml down
```

The health endpoint is intentionally unauthenticated; `/v1/*` requires a bearer
token. Create a token inside the container with the Talvik client before using
the authenticated API, then run the OpenAI-compatible qualification procedure.
Do not bind the port to `0.0.0.0` unless the deployment supplies an external
authenticated boundary.
