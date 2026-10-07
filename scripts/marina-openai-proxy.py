#!/usr/bin/env python3
"""Dependency-free OpenAI-compatible edge for a local Marina daemon."""

import argparse
import json
import os
import socket
import sys
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


def token_file():
    home = os.environ.get("MARINA_HOME", os.path.expanduser("~"))
    return os.environ.get("MARINA_TOKEN_FILE", os.path.join(home, ".marina", "tokens"))


def valid_tokens():
    try:
        with open(token_file(), encoding="utf-8") as stream:
            return {line.split("\t", 1)[0] for line in stream if "\t" in line}
    except OSError:
        return set()


def socket_path():
    home = os.environ.get("MARINA_HOME", os.path.expanduser("~"))
    return os.environ.get("MARINA_SOCKET", os.path.join(home, ".marina", "marina.sock"))


def request_daemon(line):
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as connection:
        connection.connect(socket_path())
        connection.sendall((line + "\n").encode())
        chunks = []
        while True:
            chunk = connection.recv(65536)
            if not chunk:
                break
            chunks.append(chunk)
        return b"".join(chunks).decode(errors="replace").splitlines()


def stream_daemon(line):
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as connection:
        connection.connect(socket_path())
        connection.sendall((line + "\n").encode())
        with connection.makefile("r", encoding="utf-8", errors="replace") as reader:
            yield from reader


def escaped(value):
    return value.replace("\\", "\\\\").replace("\n", "\\n").replace("\r", "\\r")


def request_id():
    return f"openai-{os.getpid()}-{time.time_ns()}"


class MarinaHandler(BaseHTTPRequestHandler):
    server_version = "MarinaOpenAIProxy/0.1"

    def send_json(self, status, body):
        payload = json.dumps(body).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)

    def authorized(self):
        value = self.headers.get("Authorization", "")
        return value.startswith("Bearer ") and value[7:] in valid_tokens()

    def do_GET(self):  # noqa: N802
        if not self.authorized():
            self.send_json(401, {"error": {"message": "missing or invalid bearer token", "type": "authentication_error"}})
            return
        if self.path == "/v1/models":
            models = []
            for line in request_daemon("model_list"):
                fields = line.split("\t")
                if len(fields) >= 2 and fields[0] == "MODEL":
                    models.append({"id": fields[1], "object": "model", "owned_by": "marina"})
            self.send_json(200, {"object": "list", "data": models})
            return
        self.send_json(404, {"error": {"message": "not found", "type": "invalid_request_error"}})

    def do_POST(self):  # noqa: N802
        if not self.authorized():
            self.send_json(401, {"error": {"message": "missing or invalid bearer token", "type": "authentication_error"}})
            return
        if self.path not in ("/v1/chat/completions", "/v1/completions"):
            self.send_json(404, {"error": {"message": "not found", "type": "invalid_request_error"}})
            return
        try:
            body = json.loads(self.rfile.read(int(self.headers.get("Content-Length", "0"))))
            model = body["model"]
            messages = body.get("messages", [])
            if messages:
                prompt_parts = []
                for message in messages:
                    role = message.get("role", "user")
                    content = message.get("content", "")
                    if isinstance(content, list):
                        if any(not isinstance(part, dict) or part.get("type") not in (None, "text") for part in content):
                            raise ValueError("only text message content is supported")
                        content = " ".join(part.get("text", "") for part in content)
                    if not isinstance(content, str):
                        raise ValueError("message content must be text")
                    prompt_parts.append(f"<{role}>\n{content}")
                prompt = "\n".join(prompt_parts)
            else:
                prompt = body.get("prompt", "")
            max_tokens = int(body.get("max_tokens", 128))
            request_daemon(f"model_load\t{model}")
            daemon_request = f"generate\t{model}\t{max_tokens}\t{request_id()}\t{escaped(str(prompt))}"
            if body.get("stream", False):
                self.send_response(200)
                self.send_header("Content-Type", "text/event-stream")
                self.send_header("Cache-Control", "no-cache")
                self.send_header("Connection", "close")
                self.end_headers()
                for raw_line in stream_daemon(daemon_request):
                    line = raw_line.rstrip("\n")
                    if line.startswith("TOKEN\t"):
                        token = line[6:].replace("\\n", "\n").replace("\\r", "\r").replace("\\\\", "\\")
                        event = {"id": "marina-local", "object": "chat.completion.chunk", "model": model,
                                 "choices": [{"index": 0, "delta": {"content": token}, "finish_reason": None}]}
                        self.wfile.write(f"data: {json.dumps(event)}\n\n".encode())
                        self.wfile.flush()
                    elif line.startswith("ERROR"):
                        break
                self.wfile.write(b"data: [DONE]\n\n")
                self.wfile.flush()
                return
            lines = request_daemon(daemon_request)
            output = ""
            for line in lines:
                if line.startswith("TOKEN\t"):
                    output += line[6:].replace("\\n", "\n").replace("\\r", "\r").replace("\\\\", "\\")
                if line.startswith("ERROR"):
                    self.send_json(500, {"error": {"message": line, "type": "server_error"}})
                    return
            self.send_json(200, {"id": "marina-local", "object": "chat.completion", "model": model,
                                 "choices": [{"index": 0, "message": {"role": "assistant", "content": output}, "finish_reason": "stop"}]})
        except (ValueError, KeyError, OSError, json.JSONDecodeError) as error:
            self.send_json(400, {"error": {"message": str(error), "type": "invalid_request_error"}})

    def log_message(self, format_string, *args):
        print(format_string % args, file=sys.stderr)


def main():
    parser = argparse.ArgumentParser(description="OpenAI-compatible local edge for Marina")
    parser.add_argument("--host", default=os.environ.get("MARINA_HTTP_HOST", "127.0.0.1"))
    parser.add_argument("--port", type=int, default=int(os.environ.get("MARINA_HTTP_PORT", "11435")))
    args = parser.parse_args()
    if not valid_tokens():
        raise SystemExit(f"No Marina tokens found at {token_file()}; run: marinactl auth token create ide")
    server = ThreadingHTTPServer((args.host, args.port), MarinaHandler)
    print(f"Marina OpenAI edge listening on http://{args.host}:{args.port}/v1", file=sys.stderr)
    server.serve_forever()


if __name__ == "__main__":
    main()
