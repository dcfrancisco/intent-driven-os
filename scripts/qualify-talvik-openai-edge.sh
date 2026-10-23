#!/usr/bin/env bash
set -euo pipefail

# Exercise the same OpenAI-compatible edge used by Continue. This script does
# not print or persist the bearer token. Tool-call support is model/template
# dependent; a 501 response is reported as unsupported rather than hidden.
BASE_URL="${TALVIK_OPENAI_BASE_URL:-http://127.0.0.1:11434/v1}"
MODEL="${TALVIK_MODEL_ID:-deepseek-coder-1.3b}"
TOKEN="${TALVIK_CONTINUE_TOKEN:-}"
if [[ -z "$TOKEN" ]]; then
  echo "Set TALVIK_CONTINUE_TOKEN to a local Talvik token." >&2
  exit 2
fi

auth=(-H "Authorization: Bearer $TOKEN")
echo "endpoint=$BASE_URL model=$MODEL token=present"
echo "models:"
curl --fail-with-body -sS "${auth[@]}" "$BASE_URL/models"
echo
echo "chat:"
curl --fail-with-body -sS "${auth[@]}" \
  -H 'Content-Type: application/json' \
  -d "{\"model\":\"$MODEL\",\"messages\":[{\"role\":\"user\",\"content\":\"Write a Python function named add that returns the sum of two integers.\"}],\"stream\":false,\"max_tokens\":96}" \
  "$BASE_URL/chat/completions"
echo
echo "stream:"
curl --fail-with-body -sS -N "${auth[@]}" \
  -H 'Content-Type: application/json' \
  -d "{\"model\":\"$MODEL\",\"messages\":[{\"role\":\"user\",\"content\":\"Reply with one short Python statement that prints hello.\"}],\"stream\":true,\"max_tokens\":48}" \
  "$BASE_URL/chat/completions"
echo
echo "tool-capability:"
set +e
curl -sS -w '\nhttp_status=%{http_code}\n' "${auth[@]}" \
  -H 'Content-Type: application/json' \
  -d "{\"model\":\"$MODEL\",\"messages\":[{\"role\":\"user\",\"content\":\"Call the weather function for Manila.\"}],\"tools\":[{\"type\":\"function\",\"function\":{\"name\":\"weather\",\"description\":\"Get weather\",\"parameters\":{\"type\":\"object\",\"properties\":{\"city\":{\"type\":\"string\"}},\"required\":[\"city\"]}}}],\"stream\":false,\"max_tokens\":96}" \
  "$BASE_URL/chat/completions"
tool_status=$?
set -e
if [[ "$tool_status" -ne 0 ]]; then
  echo "tool-call request failed at transport level" >&2
  exit "$tool_status"
fi
echo "preflight complete"
