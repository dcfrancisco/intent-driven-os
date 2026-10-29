# Talvik GPU qualification

Status: **UNVERIFIED**. This procedure has not been executed on CUDA, Vulkan,
or Metal hardware. Source implementation and deterministic tests are present;
hardware evidence is pending.

## Configuration contract

Talvik accepts `TALVIK_BACKEND=auto|cuda|vulkan|metal|cpu`,
`TALVIK_PLACEMENT=automatic|cpu_only|gpu_preferred|gpu_required`, and an
optional `TALVIK_GPU_LAYERS=<count>`. Automatic placement uses only an
initialized backend named in `TALVIK_QUALIFIED_GPU_BACKENDS`; an explicit
GPU-required request may select an initialized backend without prior
qualification but must fail rather than use CPU.

Memory controls are `TALVIK_MEMORY_BUDGET_BYTES`,
`TALVIK_SAFETY_MARGIN_BYTES`, `TALVIK_GPU_MEMORY_BUDGET_BYTES`,
`TALVIK_VRAM_RESERVE_BYTES`, and `TALVIK_LOAD_PEAK_PERCENT` (default 125).
Shared/unified GPU capacity is protected through host admission and is not
added again as dedicated VRAM.

## Build matrix

| Target | Native build command | Runtime dependency |
| --- | --- | --- |
| macOS Intel CPU | `OID_LLAMA_ACCELERATOR=cpu scripts/setup-llama-macos.sh` | packaged llama/ggml dylibs |
| macOS Apple Silicon Metal | `OID_LLAMA_ACCELERATOR=metal scripts/setup-llama-macos.sh` | Metal framework and packaged dylibs |
| Windows x64 CPU | `scripts/setup-llama-windows.ps1` | MSVC runtime and packaged DLLs |
| Windows x64 CUDA | `scripts/setup-llama-windows.ps1 -Cuda` | compatible NVIDIA driver, CUDA runtime DLLs, and packaged llama/ggml DLLs |
| Windows x64 Vulkan | `scripts/setup-llama-windows.ps1 -Vulkan` | compatible Vulkan loader/driver and packaged Vulkan/ggml DLLs |
| Linux x64 CPU | `TALVIK_LLAMA_BACKEND=cpu scripts/setup-llama-linux.sh` | packaged llama/ggml shared objects |
| Linux x64 CUDA | `TALVIK_LLAMA_BACKEND=cuda LLAMA_CPP_INSTALL_DIR="$HOME/.marina/lib-cuda" scripts/setup-llama-linux.sh` | compatible NVIDIA driver, CUDA runtime libraries, and packaged llama/ggml shared objects |
| Linux x64 Vulkan | `TALVIK_LLAMA_BACKEND=vulkan LLAMA_CPP_INSTALL_DIR="$HOME/.marina/lib-vulkan" scripts/setup-llama-linux.sh` | compatible Vulkan loader/driver and packaged Vulkan/ggml shared objects |

CUDA/Vulkan SDKs are build-time dependencies only for their explicit native
variants. CPU installations do not require them. A build option is not proof
that the runtime driver can initialize the backend.

## Exact Linux qualification commands

Run on the target Linux GPU host, replacing the two registered model IDs:

```bash
export LLAMA_CPP_LIB_DIR="$HOME/.marina/lib-cuda" # or lib-vulkan
export LD_LIBRARY_PATH="$LLAMA_CPP_LIB_DIR"
export TALVIK_BACKEND=cuda                        # or vulkan
export TALVIK_PLACEMENT=gpu_required
export TALVIK_GPU_LAYERS=999
export TALVIK_VRAM_RESERVE_BYTES=536870912
export TALVIK_QUAL_MODEL=qwen2.5-0.5b-instruct-q4_k_m
export TALVIK_QUAL_SECOND_MODEL=gemma-4-e4b-it-q4_0
export CONFIRM_TALVIK_GPU_QUALIFICATION=YES
talvikd
# In a second terminal with the same transport configuration:
scripts/qualify-talvik-gpu.sh
```

Repeat with `TALVIK_GPU_LAYERS=8` for partial offload, then with
`TALVIK_BACKEND=cpu TALVIK_PLACEMENT=cpu_only` for the matched CPU comparison.
To verify required failure, request a backend absent from the build, for
example `TALVIK_BACKEND=cuda TALVIK_PLACEMENT=gpu_required` with the Vulkan
payload; `talvik load <model>` must fail and `/api/ps` must not show it resident.

For bounded concurrency and cancellation, start an isolated loopback daemon
with `TALVIK_HTTP_AUTH=false`, then run:

```bash
curl -sS -N http://127.0.0.1:11434/v1/chat/completions -H 'Content-Type: application/json' -d '{"model":"qwen2.5-0.5b-instruct-q4_k_m","messages":[{"role":"user","content":"Count slowly to 1000"}],"stream":true,"request_id":"gpu-cancel-1"}' > /tmp/talvik-gpu-cancel.log &
curl_pid=$!
talvik cancel gpu-cancel-1
wait "$curl_pid" || true
for request_id in gpu-concurrent-1 gpu-concurrent-2; do curl -sS http://127.0.0.1:11434/v1/chat/completions -H 'Content-Type: application/json' -d "{\"model\":\"qwen2.5-0.5b-instruct-q4_k_m\",\"messages\":[{\"role\":\"user\",\"content\":\"Reply with $request_id\"}],\"request_id\":\"$request_id\"}" > "/tmp/$request_id.json" & done
wait
talvik --json status
```

Restart with a deliberately small `TALVIK_GPU_MEMORY_BUDGET_BYTES` and repeat
`talvik load`; admission must reject the candidate, retain existing residents,
and leave the candidate registered. Increase the budget only after recording
the rejection and retry the same load.

## Exact Windows qualification commands

Run in Developer PowerShell on the target GPU host:

```powershell
$env:LLAMA_CPP_LIB_DIR = "$env:USERPROFILE\.marina\lib-cuda" # or lib-vulkan
$env:PATH = "$env:LLAMA_CPP_LIB_DIR;$env:PATH"
$env:TALVIK_BACKEND = "cuda"                                 # or vulkan
$env:TALVIK_PLACEMENT = "gpu_required"
$env:TALVIK_GPU_LAYERS = "999"
$env:TALVIK_VRAM_RESERVE_BYTES = "536870912"
$env:CONFIRM_TALVIK_GPU_QUALIFICATION = "YES"
talvikd.exe
# In a second Developer PowerShell with the same transport configuration:
scripts\qualify-talvik-gpu.ps1 -Model qwen2.5-0.5b-instruct-q4_k_m -SecondModel gemma-4-e4b-it-q4_0
```

Repeat with eight GPU layers and with the CPU payload as described for Linux.
For cancellation and concurrency in an isolated loopback qualification daemon
with HTTP authentication disabled:

```powershell
$cancel = Start-Job { curl.exe -sS -N http://127.0.0.1:11434/v1/chat/completions -H "Content-Type: application/json" -d '{"model":"qwen2.5-0.5b-instruct-q4_k_m","messages":[{"role":"user","content":"Count slowly to 1000"}],"stream":true,"request_id":"gpu-cancel-1"}' }
& talvik cancel gpu-cancel-1
Wait-Job $cancel | Receive-Job
$one = Start-Job { curl.exe -sS http://127.0.0.1:11434/v1/chat/completions -H "Content-Type: application/json" -d '{"model":"qwen2.5-0.5b-instruct-q4_k_m","messages":[{"role":"user","content":"Reply with one"}],"request_id":"gpu-concurrent-1"}' }
$two = Start-Job { curl.exe -sS http://127.0.0.1:11434/v1/chat/completions -H "Content-Type: application/json" -d '{"model":"qwen2.5-0.5b-instruct-q4_k_m","messages":[{"role":"user","content":"Reply with two"}],"request_id":"gpu-concurrent-2"}' }
Wait-Job $one,$two | Receive-Job
& talvik --json status
```

Restart with a deliberately small `$env:TALVIK_GPU_MEMORY_BUDGET_BYTES`,
record the rejected load and unchanged residents, then raise the budget and
retry.

## Model unload lifecycle qualification

### Current development-host investigation

The available local registry contained this persisted entry before the
investigation:

```text
qwen2.5-0.5b-instruct-q4_k_m|...|loaded|...|/Users/dannyfrancisco/.marina/models/qwen2.5-0.5b-instruct-q4_k_m.gguf
```

The checked-in release CLI could not be used against the local daemon from the
restricted Intel macOS environment. The exact responses were:

```text
./target/release/talvik models
talvik: Operation not permitted (os error 1)

./target/release/talvik models   # with local socket access
talvik: Connection refused (os error 61)

talvikd (isolated CPU-only startup)
talvikd: Operation not permitted (os error 1)
```

Therefore no claim is made that the Windows Qwen sequence was reproduced here,
and no GPU execution was attempted. The persisted `Loaded` value is registry
state only; the lifecycle fix and deterministic regression cases require the
resident map and native backend to agree before reporting success.

Unload is a residency operation, not merely a registry transition. The daemon
must resolve the requested ID in its authoritative resident map before calling
the backend. A successful unload must remove the ID from both the resident
snapshot and the native runner; a stale persisted `Loaded` status is repaired
to `Registered` and returns an explicit non-resident error. Pinned models and
models currently generating must return explicit blocking errors.

On a supported Windows or Linux host, capture the daemon log and resident
memory before and after this exact sequence:

```text
talvik models
talvik unload qwen2.5-0.5b-instruct-q4_k_m
talvik models
talvik load qwen2.5-0.5b-instruct-q4_k_m
talvik models
talvik unload qwen2.5-0.5b-instruct-q4_k_m
talvik models
```

Repeat for every registered model, after a completed generation, after
cancellation, after a failed load, and after daemon restart. Record `/api/ps`
or `talvik --json status`, process RSS, backend/native unload logs, and GPU
memory observations where available. A later reload is valid only when a
generation request or an explicitly documented recovery policy is the cause;
it must be visible as a separate load event. This procedure remains pending
and must not be marked PASS from CPU-only source checks.

## Required observations

Record driver, device, dedicated versus shared memory, llama.cpp commit,
native variant, model checksums, context and sampling settings, and cold/warm
state. Exercise full and partial offload, CPU fallback, GPU-required failure,
memory pressure, two-model residency, bounded concurrent requests,
cancellation/recovery, unload/reload, and matched CPU/GPU measurements.

Evidence must include model load duration, prompt evaluation duration, time to
first token, generation duration, prompt/generated token counts, prompt and
generation throughput, total latency, selected backend, and actual offloaded
layers when the native API can verify them. The pinned API currently does not
expose a reliable offloaded-layer count, so that value remains `null`; native
logs must be retained as supporting evidence and qualification must not turn
unknown into a numeric claim.

Do not mark CUDA, Vulkan, Metal, GPU performance, or IDE integration PASS
until the corresponding supported-hardware procedure has been reviewed.
