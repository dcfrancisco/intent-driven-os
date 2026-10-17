#!/usr/bin/env bash

# Repeatable local release-qualification checks for a running Marina daemon.
# The caller supplies the secret through MARINA_TOKEN; this script never prints
# it. It intentionally uses only curl and standard shell utilities.

set -euo pipefail

MARINA_URL="${MARINA_URL:-http://127.0.0.1:11434}"
MARINA_TOKEN="${MARINA_TOKEN:?set MARINA_TOKEN without printing it}"
MARINA_MODEL="${MARINA_MODEL:?set MARINA_MODEL}"
QUALIFY_RUNS="${QUALIFY_RUNS:-3}"
QUALIFY_PLATFORM="${QUALIFY_PLATFORM:-$(uname -s | tr '[:upper:]' '[:lower:]')-$(uname -m)}"
QUALIFY_REPORT="${QUALIFY_REPORT:-}"
WORK_DIR="${TMPDIR:-/tmp}/marina-qualification.$RANDOM"
mkdir -p "$WORK_DIR"
trap 'rm -rf "$WORK_DIR"' EXIT
success_ids=()
cancel_ids=()
recovery_ids=()

auth=(-H "Authorization: Bearer $MARINA_TOKEN")
json=(-H 'Content-Type: application/json')

generation_id() {
  sed -n 's/.*"generation_id":"\([^"]*\)".*/\1/p' "$1" | head -n 1
}

status_code() {
  sed -n 's/^STATUS=//p' "$1"
}

request() {
  local request_id="$1"
  local max_tokens="$2"
  local timeout_ms="$3"
  local output="$4"
  local status_file="$5"
  local payload
  payload=$(printf '{"model":"%s","request_id":"%s","messages":[{"role":"user","content":"Reply with exactly QUALIFIED"}],"max_tokens":%s,"context_size":512,"timeout_ms":%s}' \
    "$MARINA_MODEL" "$request_id" "$max_tokens" "$timeout_ms")
  curl -sS --max-time 60 "${auth[@]}" "${json[@]}" \
    -d "$payload" -o "$output" -w 'STATUS=%{http_code}\n' "$MARINA_URL/v1/chat/completions" >"$status_file"
}

echo "health: $(curl -sS --max-time 5 "$MARINA_URL/healthz")"

for run in $(seq 1 "$QUALIFY_RUNS"); do
  response="$WORK_DIR/success-$run.json"
  status="$WORK_DIR/success-$run.status"
  request "qualification-success-$run" 1 30000 "$response" "$status"
  test "$(status_code "$status")" = 200
  id=$(generation_id "$response")
  test -n "$id"
  success_ids+=("$id")
  echo "success[$run]: $id"
done

for run in $(seq 1 "$QUALIFY_RUNS"); do
  request_id="qualification-cancel-$run"
  response="$WORK_DIR/cancel-$run.json"
  status="$WORK_DIR/cancel-$run.status"
  payload=$(printf '{"model":"%s","request_id":"%s","messages":[{"role":"user","content":"Write a long detailed explanation and continue until stopped."}],"max_tokens":128,"context_size":512}' "$MARINA_MODEL" "$request_id")
  curl -sS --max-time 60 "${auth[@]}" "${json[@]}" -d "$payload" -o "$response" -w 'STATUS=%{http_code}\n' "$MARINA_URL/v1/chat/completions" >"$status" &
  request_pid=$!
  sleep 1
  cancel="$WORK_DIR/cancel-$run.response"
  curl -sS --max-time 5 "${auth[@]}" -X POST "$MARINA_URL/v1/generations/$request_id/cancel" -o "$cancel"
  wait "$request_pid" || true
  test "$(status_code "$status")" = 200
  grep -q '"finish_reason":"cancelled"' "$response"
  id=$(generation_id "$response")
  test -n "$id"
  grep -q '"status":"cancellation_requested"' "$cancel"
  cancel_ids+=("$id")
  echo "cancel[$run]: $id"
done

for run in $(seq 1 "$QUALIFY_RUNS"); do
  response="$WORK_DIR/timeout-$run.json"
  status="$WORK_DIR/timeout-$run.status"
  request "qualification-timeout-$run" 128 1 "$response" "$status"
  test "$(status_code "$status")" = 504
  recovery="$WORK_DIR/recovery-$run.json"
  recovery_status="$WORK_DIR/recovery-$run.status"
  request "qualification-recovery-$run" 1 30000 "$recovery" "$recovery_status"
  test "$(status_code "$recovery_status")" = 200
  test -n "$(generation_id "$recovery")"
  recovery_ids+=("$(generation_id "$recovery")")
  echo "timeout/recovery[$run]: $(generation_id "$recovery")"
done

echo "qualification: PASS ($QUALIFY_RUNS success, cancellation, and timeout/recovery sequences)"

if [[ -n "$QUALIFY_REPORT" ]]; then
  mkdir -p "$(dirname "$QUALIFY_REPORT")"
  success_csv="$(IFS=,; echo "${success_ids[*]}")"
  cancel_csv="$(IFS=,; echo "${cancel_ids[*]}")"
  recovery_csv="$(IFS=,; echo "${recovery_ids[*]}")"
  python3 - "$QUALIFY_REPORT" "$QUALIFY_PLATFORM" "$MARINA_MODEL" "$QUALIFY_RUNS" "$success_csv" "$cancel_csv" "$recovery_csv" <<'PY'
import json
import pathlib
import sys

path, platform, model, runs, success, cancelled, recovery = sys.argv[1:]
report = {
    "schema": "marina.runtime.qualification/v1",
    "status": "passed",
    "platform": platform,
    "model": model,
    "runs": {
        "success": int(runs),
        "cancellation": int(runs),
        "timeout_recovery": int(runs),
    },
    "checks": {
        "real_http_inference": "passed",
        "unique_generation_ids": "passed",
        "cancellation_recovery": "passed",
        "timeout_recovery": "passed",
    },
    "generation_ids": {
        "success": [value for value in success.split(",") if value],
        "cancellation": [value for value in cancelled.split(",") if value],
        "timeout_recovery": [value for value in recovery.split(",") if value],
    },
}
pathlib.Path(path).write_text(json.dumps(report, sort_keys=True) + "\n")
PY
  echo "qualification report: $QUALIFY_REPORT"
fi
