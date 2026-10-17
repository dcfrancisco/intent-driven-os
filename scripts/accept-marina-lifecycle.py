#!/usr/bin/env python3
"""Exercise Marina's local streaming and cancellation protocol."""

from __future__ import annotations

import argparse
import socket
import time


def frame(*fields: str) -> bytes:
    return ("\t".join(fields) + "\n").encode()


def read_generation(
    socket_path: str,
    model: str,
    request_id: str,
    prompt: str,
    max_tokens: int,
    timeout: float,
    cancel_after_first: bool = False,
) -> tuple[str, str, float]:
    stream = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    stream.settimeout(timeout)
    stream.connect(socket_path)
    stream.sendall(frame("generate", model, str(max_tokens), request_id, prompt))
    reader = stream.makefile("r", encoding="utf-8")
    text: list[str] = []
    first_token_at = 0.0
    cancel_sent = False
    started = time.monotonic()

    while True:
        line = reader.readline()
        if not line:
            raise RuntimeError(f"{request_id}: socket closed before terminal response")
        print(f"{request_id}: {line.rstrip()!r}", flush=True)
        if line.startswith("TOKEN\t"):
            if not first_token_at:
                first_token_at = time.monotonic()
                if cancel_after_first and not cancel_sent:
                    cancel_socket = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
                    cancel_socket.settimeout(10)
                    cancel_socket.connect(socket_path)
                    cancel_socket.sendall(frame("cancel", request_id))
                    cancel_reply = cancel_socket.makefile("r", encoding="utf-8").readline()
                    print(f"cancel {request_id}: {cancel_reply.rstrip()!r}", flush=True)
                    cancel_socket.close()
                    cancel_sent = True
            text.append(line.split("\t", 1)[1].rstrip("\n"))
        if line.startswith("DONE\t") or line.startswith("ERROR\t"):
            stream.close()
            return "".join(text), line.rstrip("\n"), first_token_at - started


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--socket", required=True)
    parser.add_argument("--model", required=True)
    parser.add_argument("--timeout", type=float, default=240.0)
    args = parser.parse_args()

    france, france_done, france_first = read_generation(
        args.socket,
        args.model,
        "accept-independent-france",
        "What is the capital of France?",
        4,
        args.timeout,
    )
    math, math_done, math_first = read_generation(
        args.socket,
        args.model,
        "accept-independent-math",
        "What is 2 + 2? Answer with the number only.",
        4,
        args.timeout,
    )
    cancelled, cancelled_done, cancelled_first = read_generation(
        args.socket,
        args.model,
        "accept-cancel",
        "Explain why the sky appears blue in one paragraph.",
        128,
        args.timeout,
        cancel_after_first=True,
    )
    recovered, recovered_done, recovered_first = read_generation(
        args.socket,
        args.model,
        "accept-recovery",
        "What is the largest planet in our solar system?",
        2,
        args.timeout,
    )

    if not france or "completed" not in france_done:
        raise RuntimeError(f"France request failed: {france_done!r}, {france!r}")
    if not math or "completed" not in math_done:
        raise RuntimeError(f"math request failed: {math_done!r}, {math!r}")
    if "cancelled" not in cancelled_done:
        raise RuntimeError(f"cancellation request did not cancel: {cancelled_done!r}")
    if not recovered or "completed" not in recovered_done:
        raise RuntimeError(f"recovery request failed: {recovered_done!r}, {recovered!r}")

    print(
        "ACCEPTED",
        f"france={france!r}",
        f"math={math!r}",
        f"cancelled={cancelled_done!r}",
        f"recovery={recovered!r}",
        f"first_token_seconds={[france_first, math_first, cancelled_first, recovered_first]}",
        sep="\n",
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
