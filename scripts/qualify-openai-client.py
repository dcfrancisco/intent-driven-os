#!/usr/bin/env python3
"""Qualify Talvik with the standard OpenAI Python client.

Install the client outside the repository when running this procedure:
``python -m pip install openai``. The script intentionally fails clearly when
the dependency or a required environment variable is missing.
"""

import os


def main() -> None:
    try:
        from openai import OpenAI
    except ImportError as error:
        raise SystemExit(
            "openai Python client is required; install it with `python -m pip install openai`"
        ) from error

    base_url = os.environ.get("TALVIK_OPENAI_BASE_URL", "http://127.0.0.1:18080/v1")
    model = os.environ.get("TALVIK_MODEL")
    if not model:
        raise SystemExit("TALVIK_MODEL is required")
    client = OpenAI(
        base_url=base_url,
        api_key=os.environ.get("TALVIK_API_KEY", "local-test-key"),
    )

    listed = {entry.id for entry in client.models.list().data}
    if model not in listed:
        raise SystemExit(f"model {model!r} was not returned by /v1/models")
    plain = client.chat.completions.create(
        model=model,
        messages=[{"role": "user", "content": "Reply with exactly: OPENAI_OK"}],
        max_tokens=16,
    )
    if not plain.choices[0].message.content:
        raise SystemExit("non-stream OpenAI response did not contain assistant content")
    streamed = client.chat.completions.create(
        model=model,
        messages=[{"role": "user", "content": "Reply with exactly: STREAM_OK"}],
        max_tokens=16,
        stream=True,
    )
    streamed_text = "".join(chunk.choices[0].delta.content or "" for chunk in streamed)
    if not streamed_text:
        raise SystemExit("streaming OpenAI response was empty")

    tools = [
        {
            "type": "function",
            "function": {
                "name": "get_weather",
                "description": "Get current weather for a city",
                "parameters": {
                    "type": "object",
                    "properties": {"city": {"type": "string"}},
                    "required": ["city"],
                },
            },
        }
    ]
    tool_request = client.chat.completions.create(
        model=model,
        messages=[
            {
                "role": "user",
                "content": (
                    "Return only a JSON object with exactly these fields: name equal to "
                    "get_weather, arguments equal to an object with city equal to Manila."
                ),
            }
        ],
        tools=tools,
        tool_choice="required",
        max_tokens=64,
        temperature=0,
    )
    call = tool_request.choices[0].message.tool_calls
    if not call or call[0].function.name != "get_weather":
        raise SystemExit("OpenAI tool request did not return get_weather")
    round_trip = client.chat.completions.create(
        model=model,
        messages=[
            {"role": "user", "content": "What is the weather in Manila?"},
            {
                "role": "assistant",
                "content": None,
                "tool_calls": [
                    {
                        "id": call[0].id,
                        "type": "function",
                        "function": {
                            "name": call[0].function.name,
                            "arguments": call[0].function.arguments,
                        },
                    }
                ],
            },
            {
                "role": "tool",
                "tool_call_id": call[0].id,
                "name": "get_weather",
                "content": "Clear, 29 degrees Celsius.",
            },
            {"role": "user", "content": "Summarize the tool result in one sentence."},
        ],
        tools=tools,
        max_tokens=64,
        temperature=0,
    )
    summary = round_trip.choices[0].message.content or ""
    if "29" not in summary or "Manila" not in summary:
        raise SystemExit(f"tool-result round trip was not relevant: {summary!r}")
    print("OPENAI_CLIENT_ACCEPTED")


if __name__ == "__main__":
    main()
