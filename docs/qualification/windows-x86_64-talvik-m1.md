# Talvik M1 Windows x86_64 qualification

Run from PowerShell on a native Windows x86_64 host. Preserve the per-user
Marina data directory and use `talvik.exe` and `talvikd.exe`.

```powershell
Get-ComputerInfo | Select-Object WindowsProductName, OsArchitecture, WindowsVersion
$env:LLAMA_CPP_LIB_DIR = "$HOME\.marina\lib"
$env:LLAMA_CPP_REQUIRED = "1"
cargo build --release -p oid-console --bin talvik --bin talvikd
Get-FileHash target\release\talvik.exe -Algorithm SHA256
Get-FileHash target\release\talvikd.exe -Algorithm SHA256
Get-FileHash "$HOME\.marina\models\qwen2.5-0.5b-instruct-q4_k_m.gguf" -Algorithm SHA256
Get-Command dumpbin -ErrorAction SilentlyContinue
```

Start `talvikd.exe` on an isolated loopback TCP port and capture its logs:

```powershell
$env:TALVIK_MODEL = "qwen2.5-0.5b-instruct-q4_k_m"
$env:TALVIK_OPENAI_BASE_URL = "http://127.0.0.1:18080/v1"
& "$HOME\qualification-venv\Scripts\python.exe" scripts\qualify-openai-client.py 2>&1 | Tee-Object qualification\windows-x86_64-openai-client.log
python scripts\accept-marina-lifecycle.py --tcp 127.0.0.1:18080 --model $env:TALVIK_MODEL --timeout 240 2>&1 | Tee-Object qualification\windows-x86_64-lifecycle.log
```

Expected markers are `OPENAI_CLIENT_ACCEPTED`, `ACCEPTED`, independent
responses, `DONE completed`, `DONE cancelled`, and successful post-cancel and
post-reload inference. Record Windows version, process exit codes, model and
native DLL hashes, and all logs. A pass requires real GGUF chat, streaming,
embeddings, tool-call/tool-result round trip, cancellation, recovery,
unload/reload, and clean shutdown. The repository remains unqualified until a
native Windows host or runner executes this procedure.
