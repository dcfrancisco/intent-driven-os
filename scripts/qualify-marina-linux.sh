#!/usr/bin/env bash
set -euo pipefail

# Linux-native qualification wrapper. The daemon must already be running and
# the caller must provide a non-printing MARINA_TOKEN and MARINA_MODEL.
[[ "$(uname -s)" == "Linux" ]] || { echo "Linux is required" >&2; exit 1; }
[[ "$(uname -m)" == "x86_64" ]] || { echo "Linux x86_64 is required" >&2; exit 1; }

MARINA_REPORT="${QUALIFY_REPORT:?set QUALIFY_REPORT}"
MARINACTL="${MARINACTL:-marinactl}"
QUALIFY_RUNS="${QUALIFY_RUNS:-3}"
WORK_DIR="${TMPDIR:-/tmp}/marina-linux-qualification.$RANDOM"
mkdir -p "$WORK_DIR"
trap 'rm -rf "$WORK_DIR"' EXIT

"$MARINACTL" status >"$WORK_DIR/marinactl-status.txt"
test -S "${MARINA_SOCKET:-${HOME}/.marina/marina.sock}"

invalid_status="$(curl -sS --max-time 5 -o /dev/null -w '%{http_code}' "$MARINA_URL/v1/models")"
test "$invalid_status" = 401
admin_status="$(curl -sS --max-time 5 -o /dev/null -w '%{http_code}' \
  -H "Authorization: Bearer $MARINA_TOKEN" -X POST \
  "$MARINA_URL/v1/models/$MARINA_MODEL/load")"
test "$admin_status" = 403
context_status="$(curl -sS --max-time 5 -o /dev/null -w '%{http_code}' \
  -H "Authorization: Bearer $MARINA_TOKEN" -H 'Content-Type: application/json' \
  -d "{\"model\":\"$MARINA_MODEL\",\"request_id\":\"linux-context-rejection\",\"prompt\":\"test\",\"context_size\":32769}" \
  "$MARINA_URL/v1/chat/completions")"
test "$context_status" = 429
printf '%s\n' "invalid_auth=$invalid_status" "admin_scope=$admin_status" "context_admission=$context_status" >"$WORK_DIR/security-status.txt"

export QUALIFY_PLATFORM="linux-$(uname -m)"
export QUALIFY_REPORT="$WORK_DIR/http-report.json"
./scripts/qualify-marina-http.sh

python3 - "$QUALIFY_REPORT" "$MARINA_REPORT" "$WORK_DIR/marinactl-status.txt" <<'PY'
import json
import os
import pathlib
import platform
import shutil
import subprocess
import sys

source, target, status_path = sys.argv[1:]
report = json.loads(pathlib.Path(source).read_text())
report["work_package"] = "WP-0078"
report["native_execution"] = True
report["host"] = {
    "os": platform.platform(),
    "kernel": platform.release(),
    "architecture": platform.machine(),
    "cpu": next(
        (line.split(":", 1)[1].strip() for line in pathlib.Path("/proc/cpuinfo").read_text().splitlines()
         if line.lower().startswith("model name") and ":" in line),
        platform.processor(),
    ),
    "python": platform.python_version(),
}
report["cpu_flags"] = next(
    (line.split(":", 1)[1].split() for line in pathlib.Path("/proc/cpuinfo").read_text().splitlines()
     if line.lower().startswith(("flags", "features")) and ":" in line),
    [],
)
report["memory"] = {
    line.split(":", 1)[0]: line.split(":", 1)[1].strip()
    for line in pathlib.Path("/proc/meminfo").read_text().splitlines()
    if line.startswith(("MemTotal:", "MemAvailable:"))
}
report["tooling"] = {}
for command in (("rustc", "-Vv"), ("cargo", "-V"), ("cmake", "--version"), ("gcc", "--version")):
    try:
        executable = shutil.which(command[0]) or f"/usr/bin/{command[0]}"
        report["tooling"][command[0]] = subprocess.check_output((executable, *command[1:]), text=True, stderr=subprocess.STDOUT).splitlines()[0]
    except (OSError, subprocess.CalledProcessError):
        report["tooling"][command[0]] = "unavailable"
report["llama_cpp_commit"] = os.environ.get("LLAMA_CPP_COMMIT", "unknown")
report["model_sha256"] = os.environ.get("MARINA_MODEL_SHA256", "unknown")
binary = os.environ.get("MARINA_BINARY")
if binary:
    try:
        report["native_dependencies"] = subprocess.check_output(
            ("ldd", binary), text=True, stderr=subprocess.STDOUT
        ).splitlines()
    except (OSError, subprocess.CalledProcessError):
        report["native_dependencies"] = ["ldd unavailable or unresolved dependency"]
else:
    report["native_dependencies"] = ["MARINA_BINARY not supplied"]
report["unix_socket"] = {"status": "passed", "status_output": pathlib.Path(status_path).read_text().strip()}
report["security_checks"] = {
    key: value for key, value in (
        line.split("=", 1)
        for line in pathlib.Path(status_path).with_name("security-status.txt").read_text().splitlines()
    )
}
report["credentials_logged"] = False
pathlib.Path(target).write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
PY
mkdir -p "$(dirname "$MARINA_REPORT")"
echo "qualification report: $MARINA_REPORT"
