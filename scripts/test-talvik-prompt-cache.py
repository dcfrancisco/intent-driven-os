#!/usr/bin/env python3
"""Bounded Qwen3.5 API prompt-cache and phase-timing acceptance.

Requires an idle loopback daemon and its registered Qwen3.5 model. No model,
configuration, credential or service restart is required. Context-size changes
clear only this benchmark model's prompt state. Never run during IDE work.
"""
import json
import os
import pathlib
import time
import urllib.parse
import urllib.request
import uuid

BASE = os.environ.get("TALVIK_TEST_BASE_URL", "http://127.0.0.1:11435").rstrip("/")
MODEL = os.environ.get("TALVIK_TEST_VISION_MODEL", "qwen3.5-4b-Q4_K_M")
REPORT = pathlib.Path(os.environ.get("TALVIK_TEST_REPORT", "talvik-prompt-cache-report.json"))
TOKEN = pathlib.Path(os.environ.get("TALVIK_TEST_TOKEN_FILE", str(pathlib.Path.home()/".marina/sse-test.token"))).read_text().strip()
PREFIX = "cache-check-" + uuid.uuid4().hex[:12]
RESULTS = []


def request(path, body=None):
    req = urllib.request.Request(BASE+path, data=json.dumps(body).encode() if body is not None else None,
                                 headers={"Authorization": "Bearer "+TOKEN, "Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=150) as response:
        return json.load(response)


def clear_prompt_state():
    # Both calls use the same resident model. Changing context size invalidates
    # its native context, giving a cold prompt without unloading model weights.
    request("/v1/chat/completions", {"model": MODEL, "context_size": 4096, "max_tokens": 4,
            "temperature": 0, "timeout_ms": 120000,
            "request_id": PREFIX+"-reset-"+uuid.uuid4().hex[:6],
            "messages": [{"role": "user", "content": "Reply OK."}]})


def check(name, messages):
    started = time.monotonic()
    response = request("/v1/chat/completions", {"model": MODEL, "messages": messages,
                       "context_size": 8192, "max_tokens": 12, "temperature": 0,
                       "timeout_ms": 120000, "request_id": PREFIX+"-"+name})
    text = response["choices"][0]["message"]["content"]
    assert text.strip() == "PERF_OK", (name, text)
    metrics = response["metrics"]
    assert abs(metrics["inference_time_ms"]-metrics["prompt_eval_time_ms"]-metrics["generation_time_ms"]) <= 2, metrics
    assert metrics["generation_time_ms"] > 0, metrics
    expected = response["usage"]["completion_tokens"]*1000/metrics["generation_time_ms"]
    assert abs(metrics["tokens_per_second"]-expected)/expected < .02, metrics
    result = {"phase": name, "wall_seconds": round(time.monotonic()-started, 3),
              "content": text, "usage": response["usage"], "metrics": metrics}
    RESULTS.append(result)
    REPORT.write_text(json.dumps(RESULTS, indent=2)+"\n")
    print(json.dumps(result), flush=True)
    return result


def main():
    assert urllib.parse.urlparse(BASE).hostname in {"127.0.0.1", "localhost", "::1"}, "Only loopback qualification is supported"
    state = request("/api/ps")
    assert state["active_requests"] == state["pending_requests"] == 0, "Daemon busy; no benchmark run"
    records = "".join(f"Task {index:03d}: complete.\n" for index in range(80))
    messages = [{"role": "system", "content": "Read the records below. Reply PERF_OK only.\n"+records},
                {"role": "user", "content": "Report status."}]
    clear_prompt_state()
    cold = check("cold", messages)
    assert not cold["metrics"]["cache_hit"], cold
    repeated = check("repeat", messages)
    assert repeated["metrics"]["cache_hit"], repeated
    assert repeated["metrics"]["reused_prompt_tokens"] >= repeated["usage"]["prompt_tokens"]-2, repeated
    changed_messages = [messages[0], {"role": "user", "content": "Report the final status."}]
    changed = check("changed_user", changed_messages)
    assert changed["metrics"]["cache_hit"], changed
    assert changed["metrics"]["reused_prompt_tokens"] > changed["usage"]["prompt_tokens"]*.75, changed
    clear_prompt_state()
    reference = check("changed_user_cold_reference", changed_messages)
    assert not reference["metrics"]["cache_hit"], reference
    assert reference["content"] == changed["content"], (reference, changed)
    print("PROMPT_CACHE_ACCEPTED", flush=True)


if __name__ == "__main__":
    main()
