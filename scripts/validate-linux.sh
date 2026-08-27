#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
IMAGE="${OID_LINUX_VALIDATION_IMAGE:-oid-linux-validation:ubuntu-24.04}"

if ! command -v docker >/dev/null 2>&1; then
    echo "Docker CLI is required for Linux validation." >&2
    exit 1
fi

if ! docker info >/dev/null 2>&1; then
    echo "Docker daemon is unavailable; Linux validation was not run." >&2
    exit 1
fi

echo "Building ${IMAGE}..."
docker build \
    --file "${REPO_ROOT}/docker/validation/ubuntu-24.04/Dockerfile" \
    --tag "${IMAGE}" \
    "${REPO_ROOT}"

echo "Running Linux validation in Ubuntu 24.04..."
docker run --rm --init \
    --volume "${REPO_ROOT}:/workspace" \
    --workdir /workspace \
    --env CARGO_TERM_COLOR=always \
    "${IMAGE}" \
    bash -eu -o pipefail -c '
        cargo fmt --all -- --check
        cargo build --workspace
        cargo clippy --workspace --all-targets --all-features -- -D warnings
        cargo test --workspace
        cargo test -p oid-console --test process_signals -- --test-threads=1
    '

echo "Checking repository diff..."
(cd "${REPO_ROOT}" && git diff --check)

echo "Linux Docker validation passed."
