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
    token = os.environ.get("TALVIK_API_KEY")
    if not model or not token:
        raise SystemExit("TALVIK_MODEL and TALVIK_API_KEY are required")
    client = OpenAI(
        base_url=base_url,
        api_key=token,
    )

    listed = {entry.id for entry in client.models.list().data}
    if model not in listed:
        raise SystemExit(f"model {model!r} was not returned by /v1/models")
    print("PASS model discovery", flush=True)

    plain = client.chat.completions.create(
        model=model,
        messages=[{"role": "user", "content": "Reply with exactly: OPENAI_OK"}],
        max_tokens=4,
    )
    if not plain.choices[0].message.content:
        raise SystemExit("non-stream OpenAI response did not contain assistant content")
    print("PASS non-stream Chat Completions", flush=True)

    streamed = client.chat.completions.create(
        model=model,
        messages=[{"role": "user", "content": "Reply with exactly: STREAM_OK"}],
        max_tokens=4,
        stream=True,
    )
    streamed_text = "".join(chunk.choices[0].delta.content or "" for chunk in streamed)
    if not streamed_text:
        raise SystemExit("streaming OpenAI response was empty")
    print("PASS streaming Chat Completions", flush=True)

    legacy = client.completions.create(
        model=model,
        prompt="Complete with a short greeting:",
        max_tokens=4,
    )
    if legacy.object != "text_completion" or not legacy.choices[0].text:
        raise SystemExit("legacy Completions response did not use the text_completion shape")
    print("PASS non-stream legacy Completions", flush=True)

    legacy_stream = client.completions.create(
        model=model,
        prompt="Complete with one word:",
        max_tokens=4,
        stream=True,
    )
    legacy_stream_text = "".join(chunk.choices[0].text or "" for chunk in legacy_stream)
    if not legacy_stream_text:
        raise SystemExit("legacy Completions stream was empty")
    print("PASS streaming legacy Completions", flush=True)

    response = client.responses.create(
        model=model,
        instructions="Reply briefly.",
        input="Say hello.",
        max_output_tokens=4,
    )
    if response.object != "response" or not response.output_text:
        raise SystemExit("Responses API did not return a text response")
    print("PASS non-stream Responses", flush=True)

    if os.environ.get("TALVIK_QUALIFY_TOOLS", "").lower() in {"1", "true", "yes"}:
        qualify_tool_round_trip(client, model)

    print("OPENAI_CLIENT_ACCEPTED")


def qualify_tool_round_trip(client, model: str) -> None:
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
                    "Call the get_weather function with city set to Manila. Do not answer "
                    "with plain text."
                ),
            }
        ],
        tools=tools,
        tool_choice="auto",
        parallel_tool_calls=False,
        max_tokens=16,
        context_size=512,
        temperature=0,
    )
    calls = tool_request.choices[0].message.tool_calls
    if not calls or len(calls) != 1 or calls[0].function.name != "get_weather":
        raise SystemExit("OpenAI tool request did not return get_weather")
    required = client.chat.completions.create(
        model=model,
        messages=[
            {
                "role": "user",
                "content": "Call get_weather with city set to Manila. Do not answer in prose.",
            }
        ],
        tools=tools,
        tool_choice="required",
        max_tokens=16,
        context_size=512,
        temperature=0,
    )
    required_calls = required.choices[0].message.tool_calls
    if not required_calls:
        raise SystemExit("OpenAI required tool choice returned no tool call")
    print("PASS required tool choice", flush=True)

    selected = client.chat.completions.create(
        model=model,
        messages=[
            {
                "role": "user",
                "content": "Call get_weather with city set to Manila. Do not answer in prose.",
            }
        ],
        tools=tools,
        tool_choice={"type": "function", "function": {"name": "get_weather"}},
        max_tokens=16,
        context_size=512,
        temperature=0,
    )
    selected_calls = selected.choices[0].message.tool_calls
    if not selected_calls or any(
        tool_call.function.name != "get_weather" for tool_call in selected_calls
    ):
        raise SystemExit("OpenAI function-specific tool choice returned the wrong tool")
    print("PASS function-specific tool choice", flush=True)

    call = calls[0]
    round_trip = client.chat.completions.create(
        model=model,
        messages=[
            {"role": "user", "content": "What is the weather in Manila?"},
            {
                "role": "assistant",
                "content": None,
                "tool_calls": [
                    {
                        "id": call.id,
                        "type": "function",
                        "function": {
                            "name": call.function.name,
                            "arguments": call.function.arguments,
                        },
                    }
                ],
            },
            {
                "role": "tool",
                "tool_call_id": call.id,
                "name": "get_weather",
                "content": "Clear, 29 degrees Celsius.",
            },
            {"role": "user", "content": "Summarize the tool result briefly."},
        ],
        tools=tools,
        max_tokens=8,
        context_size=512,
        temperature=0,
    )
    if not round_trip.choices[0].message.content:
        raise SystemExit("OpenAI tool-result round trip returned no assistant content")
    print("PASS tool call and result message transport", flush=True)


if __name__ == "__main__":
    main()
