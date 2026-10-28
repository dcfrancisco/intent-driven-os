#!/usr/bin/env python3
"""Smoke-qualify Talvik's native API using the official Ollama Python client.

Install the SDK outside the repository with `python -m pip install ollama`.
Set TALVIK_API_KEY and TALVIK_MODEL to a Talvik bearer token and installed
text-generation model before running this script.
"""

import json
import os
import urllib.error
import urllib.request


def fail(message: str) -> None:
    raise SystemExit(message)


def main() -> None:
    try:
        import ollama
    except ImportError as error:
        raise SystemExit(
            "ollama Python client is required; install it with `python -m pip install ollama`"
        ) from error

    base_url = os.environ.get(
        "TALVIK_OLLAMA_BASE_URL", "http://127.0.0.1:18080"
    ).rstrip("/")
    model = os.environ.get("TALVIK_MODEL")
    token = os.environ.get("TALVIK_API_KEY")
    if not model or not token:
        fail("TALVIK_MODEL and TALVIK_API_KEY are required")

    client = ollama.Client(
        host=base_url,
        headers={"Authorization": f"Bearer {token}"},
    )

    version_request = urllib.request.Request(
        f"{base_url}/api/version",
        headers={"Authorization": f"Bearer {token}"},
    )
    with urllib.request.urlopen(version_request, timeout=15) as response:
        version = json.load(response)
    if not version.get("version"):
        fail("/api/version did not return a version")

    listed = {entry.model for entry in client.list().models}
    if model not in listed:
        fail(f"model {model!r} was not returned by /api/tags")
    client.ps()
    client.show(model)
    print("PASS version, tags, ps, and show", flush=True)

    generated = client.generate(
        model=model,
        prompt="Reply with exactly: GENERATE_OK",
        stream=False,
        options={"num_ctx": 512, "num_predict": 4, "temperature": 0},
    )
    if not generated.done or not generated.response:
        fail("non-streaming /api/generate returned no completed text")
    print("PASS non-streaming generate", flush=True)

    generated_stream = client.generate(
        model=model,
        prompt="Reply with exactly: GENERATE_STREAM_OK",
        stream=True,
        options={"num_ctx": 512, "num_predict": 4, "temperature": 0},
    )
    generate_chunks = list(generated_stream)
    if not generate_chunks or not generate_chunks[-1].done:
        fail("streaming /api/generate did not end with a done record")
    if not "".join(chunk.response for chunk in generate_chunks):
        fail("streaming /api/generate returned no text")
    print("PASS streaming generate", flush=True)

    chat = client.chat(
        model=model,
        messages=[{"role": "user", "content": "Reply with exactly: CHAT_OK"}],
        stream=False,
        options={"num_ctx": 512, "num_predict": 4, "temperature": 0},
    )
    if not chat.done or not chat.message.content:
        fail("non-streaming /api/chat returned no completed assistant message")
    print("PASS non-streaming chat", flush=True)

    chat_stream = client.chat(
        model=model,
        messages=[{"role": "user", "content": "Reply with exactly: CHAT_STREAM_OK"}],
        stream=True,
        options={"num_ctx": 512, "num_predict": 4, "temperature": 0},
    )
    chat_chunks = list(chat_stream)
    if not chat_chunks or not chat_chunks[-1].done:
        fail("streaming /api/chat did not end with a done record")
    if not "".join(chunk.message.content or "" for chunk in chat_chunks):
        fail("streaming /api/chat returned no text")
    print("PASS streaming chat", flush=True)

    try:
        client.generate(
            model=f"{model}-talvik-qualification-missing",
            prompt="This request must fail.",
            stream=False,
        )
    except ollama.ResponseError as error:
        if error.status_code != 404:
            fail(f"missing-model error used unexpected HTTP status {error.status_code}")
    else:
        fail("unknown model unexpectedly produced a successful generation")

    print("OLLAMA_PYTHON_CLIENT_ACCEPTED")


if __name__ == "__main__":
    main()
