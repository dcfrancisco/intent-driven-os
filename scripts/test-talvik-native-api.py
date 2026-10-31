#!/usr/bin/env python3
"""Exercise Talvik using Copilot-style streaming requests; no external services."""
import base64
import json
import os
import pathlib
import struct
import time
import urllib.error
import urllib.request
import zlib

BASE = os.environ.get("TALVIK_TEST_BASE_URL", "http://127.0.0.1:11435").rstrip("/")
TOKEN = pathlib.Path(os.environ.get("TALVIK_TEST_TOKEN_FILE", str(pathlib.Path.home() / ".marina/sse-test.token"))).read_text().strip()
QWEN = os.environ.get("TALVIK_TEST_VISION_MODEL", "qwen3.5-4b-Q4_K_M")
CODER = os.environ.get("TALVIK_TEST_CODER_MODEL", "qwen2.5-coder-3b-instruct-q4_k_m")
RESULTS = []


def request(body):
    req = urllib.request.Request(
        BASE + "/v1/chat/completions",
        data=json.dumps(body).encode(),
        headers={"Authorization": "Bearer " + TOKEN, "Content-Type": "application/json"},
    )
    try:
        with urllib.request.urlopen(req, timeout=300) as response:
            wire = response.read().decode()
    except urllib.error.HTTPError as error:
        raise AssertionError(f"HTTP {error.code}: {error.read().decode()}") from error
    text, calls, finish, usage = "", [], None, None
    assert "data: [DONE]" in wire, "Stream did not finish successfully"
    for line in wire.splitlines():
        if not line.startswith("data: ") or line == "data: [DONE]":
            continue
        chunk = json.loads(line[6:])
        assert "error" not in chunk, chunk
        if chunk.get("usage"):
            usage = chunk["usage"]
        for choice in chunk.get("choices", []):
            delta = choice.get("delta", {})
            text += delta.get("content") or ""
            calls.extend(delta.get("tool_calls") or [])
            finish = choice.get("finish_reason") or finish
    assert finish in {"stop", "length", "tool_calls"}, f"Missing finish reason: {finish}"
    assert usage, "Missing requested usage metrics"
    return text, calls, finish, usage


def check(name, body, validate):
    started = time.monotonic()
    text, calls, finish, usage = "", [], None, None
    try:
        text, calls, finish, usage = request({
            "model": QWEN,
            "stream": True,
            "stream_options": {"include_usage": True},
            "max_completion_tokens": 160,
            "temperature": 0.7,
            "top_p": 0.8,
            **body,
        })
        validate(text, calls, finish)
        result = {"test": name, "passed": True, "content": text, "tool_calls": calls,
                  "finish_reason": finish, "usage": usage}
    except Exception as error:
        result = {"test": name, "passed": False, "error": str(error), "content": text,
                  "tool_calls": calls, "finish_reason": finish, "usage": usage}
    result["seconds"] = round(time.monotonic() - started, 2)
    RESULTS.append(result)
    print(json.dumps(result), flush=True)
    return result


def clean(text, calls, finish):
    assert text.strip(), "Empty answer"
    assert "<think>" not in text and "</think>" not in text, "Thinking wrapper leaked"
    assert not text.rstrip().endswith("<"), "Trailing terminal prefix leaked"


check("chat", {"messages": [{"role": "user", "content": "What is 2 + 2? Answer with the number only."}]}, clean)
check("coder", {"model": CODER, "temperature": 0,
                "messages": [{"role": "user", "content": "Write a Python function clamp(x, low, high) that bounds x inclusively. Output code only."}]}, clean)


def chunk(kind, data):
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)


png = (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">2I5B", 16, 16, 8, 6, 0, 0, 0))
       + chunk(b"IDAT", zlib.compress((b"\0" + b"\xff\0\0\xff" * 16) * 16)) + chunk(b"IEND", b""))
image = "data:image/png;base64," + base64.b64encode(png).decode()


def red(text, calls, finish):
    clean(text, calls, finish)
    assert "red" in text.lower(), "Incorrect image color"


check("vision", {"messages": [{"role": "user", "content": [
    {"type": "text", "text": "Name the dominant color in one word."},
    {"type": "image_url", "image_url": {"url": image}},
]}]}, red)

tools = [{"type": "function", "function": {"name": "read_file", "description": "Read a file by its path.",
          "parameters": {"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]}}}]
messages = [{"role": "system", "content": "Use the supplied tools to inspect files. Once the tool result is available, answer with the requested content; do not call it again."},
            {"role": "user", "content": "Read /tmp/talvik-check.txt with read_file, then report its exact content."}]


def tool(text, calls, finish):
    assert finish == "tool_calls" and calls, "Agent did not return a usable tool call"
    assert calls[0]["function"]["name"] == "read_file", "Incorrect tool selection"
    assert json.loads(calls[0]["function"]["arguments"])["path"] == "/tmp/talvik-check.txt"


agent = check("agent_tool_sse", {"messages": messages, "tools": tools, "tool_choice": "required"}, tool)
if agent["passed"]:
    call = {key: value for key, value in agent["tool_calls"][0].items() if key != "index"}
    history = messages + [{"role": "assistant", "content": None, "tool_calls": [call]},
                          {"role": "tool", "tool_call_id": call["id"], "content": "TALVIK_AGENT_OK"}]

    def continued(text, calls, finish):
        clean(text, calls, finish)
        assert "TALVIK_AGENT_OK" in text and not calls, "Agent did not consume the tool result"

    check("agent_tool_result", {"messages": history, "tools": tools, "tool_choice": "auto"}, continued)

path = pathlib.Path(os.environ.get("TALVIK_TEST_REPORT", "talvik-test-report.json"))
path.write_text(json.dumps(RESULTS, indent=2) + "\n")
print("Report:", path, flush=True)
raise SystemExit(0 if all(r["passed"] for r in RESULTS) else 1)
