#!/usr/bin/env bash
set -euo pipefail

# Execute only on a supported Linux GPU host after starting talvikd with the
# backend and placement environment documented in the qualification guide.
[[ "${CONFIRM_TALVIK_GPU_QUALIFICATION:-}" == "YES" ]] || {
  echo "Refusing to run: set CONFIRM_TALVIK_GPU_QUALIFICATION=YES on supported GPU hardware." >&2
  exit 2
}
[[ "$(uname -s)" == "Linux" ]] || { echo "Linux is required." >&2; exit 2; }
: "${TALVIK_QUAL_MODEL:?set TALVIK_QUAL_MODEL to a registered model ID}"
: "${TALVIK_QUAL_SECOND_MODEL:?set TALVIK_QUAL_SECOND_MODEL for multi-model residency}"

command -v talvik >/dev/null || { echo "talvik is required on PATH." >&2; exit 1; }

talvik --json status
talvik --json models
talvik --json load "$TALVIK_QUAL_MODEL"
talvik run "$TALVIK_QUAL_MODEL" "Reply with exactly TALVIK_GPU_OK"
talvik --json load "$TALVIK_QUAL_SECOND_MODEL"
talvik --json status
talvik --json unload "$TALVIK_QUAL_SECOND_MODEL"
talvik --json unload "$TALVIK_QUAL_MODEL"

echo "Lifecycle sequence complete. Review daemon diagnostics for selected backend,"
echo "requested/actual offload, memory, fallback, cancellation, and timing evidence."
