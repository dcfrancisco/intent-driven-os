#!/usr/bin/env python3
"""Qualify the Ollama and OpenAI embedding endpoints with official Python SDKs."""

import os


def main() -> None:
    try:
        import ollama
        from openai import OpenAI
    except ImportError as error:
        raise SystemExit(
            "ollama and openai Python clients are required; install them with "
            "`python -m pip install ollama openai`"
        ) from error

    model = os.environ.get("TALVIK_EMBED_MODEL")
    token = os.environ.get("TALVIK_API_KEY")
    if not model or not token:
        raise SystemExit("TALVIK_EMBED_MODEL and TALVIK_API_KEY are required")

    ollama_url = os.environ.get(
        "TALVIK_OLLAMA_BASE_URL", "http://127.0.0.1:18080"
    )
    ollama_client = ollama.Client(
        host=ollama_url,
        headers={"Authorization": f"Bearer {token}"},
    )
    embedded = ollama_client.embed(
        model=model,
        input=["embedding qualification one", "embedding qualification two"],
    )
    if len(embedded.embeddings) != 2 or not all(embedded.embeddings):
        raise SystemExit("/api/embed did not return two nonempty vectors")

    legacy = ollama_client.embeddings(
        model=model,
        prompt="legacy embedding qualification",
    )
    if not legacy.embedding:
        raise SystemExit("/api/embeddings returned an empty vector")
    print("PASS Ollama batch and legacy embeddings", flush=True)

    openai_client = OpenAI(
        base_url=os.environ.get(
            "TALVIK_OPENAI_BASE_URL", "http://127.0.0.1:18080/v1"
        ),
        api_key=token,
    )
    openai_result = openai_client.embeddings.create(
        model=model,
        input=["embedding qualification one", "embedding qualification two"],
    )
    if len(openai_result.data) != 2 or not all(
        item.embedding for item in openai_result.data
    ):
        raise SystemExit("/v1/embeddings did not return two nonempty vectors")
    print("PASS OpenAI batch embeddings", flush=True)
    print("EMBEDDING_PYTHON_CLIENTS_ACCEPTED")


if __name__ == "__main__":
    main()
