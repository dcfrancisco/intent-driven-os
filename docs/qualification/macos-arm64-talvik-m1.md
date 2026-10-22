# Talvik M1 macOS arm64 qualification

Run on a native Apple Silicon host; do not infer this gate from x86_64.

```sh
uname -a
uname -m                         # expected: arm64
shasum -a 256 "$HOME/.marina/models/qwen2.5-0.5b-instruct-q4_k_m.gguf"
shasum -a 256 "$HOME/.marina/lib/libllama.0.3.0.dylib"
LLAMA_CPP_LIB_DIR="$HOME/.marina/lib" LLAMA_CPP_REQUIRED=1 cargo build --release -p oid-console --bin talvik --bin talvikd
shasum -a 256 target/release/talvik target/release/talvikd
```

Start `talvikd` on an isolated socket and loopback port, then capture daemon
output and run:

```sh
TALVIK_MODEL=qwen2.5-0.5b-instruct-q4_k_m \
TALVIK_OPENAI_BASE_URL=http://127.0.0.1:18080/v1 \
/path/to/isolated-python/bin/python scripts/qualify-openai-client.py 2>&1 | tee qualification/macos-arm64-openai-client.log
python3 scripts/accept-marina-lifecycle.py --socket /tmp/talvik-m1/talvik.sock --model qwen2.5-0.5b-instruct-q4_k_m --timeout 240 2>&1 | tee qualification/macos-arm64-lifecycle.log
```

Expected markers are `OPENAI_CLIENT_ACCEPTED`, `ACCEPTED`, independent
responses, `DONE completed`, `DONE cancelled`, and successful post-cancel and
post-reload inference. Record host/OS, binary hashes, model/native-library
hashes, command logs, daemon logs, and exit codes. A pass requires real GGUF
chat, streaming, embeddings, tool-call/tool-result round trip, cancellation,
recovery, unload/reload, and clean shutdown.
