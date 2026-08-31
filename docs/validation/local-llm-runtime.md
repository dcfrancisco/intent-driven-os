# Local LLM Runtime Validation

## WP-LLM-001 status

The OID-owned `ModelRunner` contract and the dedicated llama.cpp adapter are
implemented. The public runtime boundary contains no llama.cpp types:

```text
OID Console → Intelligent Runtime → ModelRunner → llama.cpp Adapter → GGUF
```

## WP-LLM-002 Status

**PARTIAL** — Ubuntu live inference, raw llama.cpp comparison, Ollama API
measurements, streaming, loading, completion, and unload were exercised. The
live Ctrl+C path was not validated as graceful: a control character reached the
PTY, but generation continued to completion and no cancellation event was
observed. Contract-level cancellation tests pass. No WP-LLM-003 work has begun.

## Hardware

Captured on 2026-08-28 from `danny@192.168.1.112`:

```text
OS:              Ubuntu 26.04 LTS
Kernel:          7.0.0-30-generic x86_64
CPU:             AMD Phenom(tm) II X6 1055T Processor
Physical cores:  6
Logical CPUs:    6
RAM:             15 GiB usable / 16 GB installed
Swap:            4 GiB, initially unused
Rust:            1.96.0
GCC:             15.2.0
CMake:           4.2.3
Ollama:          0.30.10
OID commit:      67c0bbde59833fe7572b9ac7bca14d70a2bf4f9c
llama.cpp:       18443257a30c884d5332abb8e7dc43c7ffe42fda (0.3.0-dev)
```

The CPU has SSE/SSE2/SSE4a, but no AVX, AVX2, AVX-512, FMA, F16C, SSE4.2,
or BMI2. llama.cpp was configured with native and unsupported SIMD features
disabled and OpenMP enabled:

```text
-DGGML_NATIVE=OFF -DGGML_AVX=OFF -DGGML_AVX2=OFF
-DGGML_AVX512=OFF -DGGML_FMA=OFF -DGGML_F16C=OFF
-DGGML_SSE42=OFF -DGGML_BMI2=OFF -DGGML_OPENMP=ON
-DLLAMA_CURL=OFF -DCMAKE_BUILD_TYPE=Release
```

The resulting build completed for `llama-cli` and `llama-bench`.

## Model and benchmark configuration

```text
Model:             Llama 3.2 1B Instruct
GGUF:              /home/danny/Models/llama3.2-1b.gguf
Quantization:      Q8_0
File size:         1.22 GiB
SHA-256:           74701a8c35f6c8d9a4b91f3f3497643001d63e0c7a84e085bed452548fa88d45
Context:           4096
Threads:           6 generation / 6 batch
Temperature:       0.8
Seed:              0
Maximum output:    128 tokens
Primary prompt:    Reply with exactly: OID inference works.
Repetitions:       5 raw, 5 OID, 1 Ollama cold, 2 Ollama warm
```

The Q8_0 artifact was selected because it is the exact local GGUF underlying
the installed Ollama `llama3.2:1b` model. It is not the preferred Q4_K_M
quantization, but it proves artifact identity across the three paths.

## Functional validation

| Check | Result | Evidence |
| --- | --- | --- |
| Raw llama.cpp inference | PASS | `benchmarks/wp-llm-002/raw/` |
| OID model load | PASS | OID run logs |
| OID incremental streaming | PASS | token fragments preceded completion stats |
| OID completion and statistics | PASS | five OID run logs |
| OID cancellation | NOT VALIDATED | live PTY control-C did not stop generation |
| OID unload | PASS | each OID run returned to `model none` |
| Ollama inference | PASS | structured `/api/generate` responses |

## Performance results

The primary values below are observed medians for Prompt A. Raw llama.cpp and
OID report backend generation differently enough that their throughput is an
observed CLI-versus-adapter comparison, not a clean proof of backend
efficiency. Ollama values are server-reported.

| Engine | State | Load | TTFT | Prompt eval | Generation | tok/s | Total | RSS |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| llama.cpp | cold process | included | unavailable | unavailable | 128 tokens | 3.9 | 19.94 s external | 1,505,856 KiB |
| OID | cold process | included | unavailable | unavailable | 128 tokens | 5.2 | 25.35 s external | 1,484,848 KiB |
| Ollama | cold API | 0.81 s | unavailable | 0.88 s | 128 tokens | 1.22 | 298.80 s | server observed ~1.14 GiB |
| Ollama | warm API | resident | unavailable | 0.84–16.25 s | 128 tokens | 1.22 | 118.86 s median of 2 | server observed ~1.14 GiB |

Raw observations and the source payloads are retained under
`docs/validation/benchmarks/wp-llm-002/`. The Ollama cold run reported
105.353 s generation and the two warm runs reported 105.001 s and 105.132 s.

## OID overhead and efficiency

Using the recorded primary medians:

```text
Observed OID generation efficiency vs raw CLI:
    5.2 / 3.9 × 100 = 133.3%

Observed cold external latency difference:
    25.35 s - 19.94 s = +5.41 s
    relative to raw external time: +27.1%

Observed RSS difference:
    1,484,848 KiB - 1,505,856 KiB = -21,008 KiB
```

These figures must not be interpreted as OID superiority. The raw CLI emitted
different answer semantics for the same short prompt and does not expose the
same internal phase timings as OID. The evidence supports an observed result,
not a clean attribution of the 5.41-second cold difference to orchestration.
The OID adapter did not show pathological steady-state overhead in its own
generation timing, but direct apples-to-apples attribution remains open.

## OID versus Ollama

For this artifact and machine, Ollama was slower in the measured server-side
generation phase: about 1.22 tok/s versus 3.9 tok/s raw CLI and 5.2 tok/s OID
observed adapter throughput. Ollama also showed materially different cold and
warm lifecycle timings. These are observations under the captured settings,
not a general performance claim. Ollama's model was resident during warm
requests and its default runtime produced different response text despite
`raw` generation mode.

## Findings

- The Phenom II lacks modern SIMD features; an explicit portable llama.cpp
  build succeeded without AVX-family assumptions.
- The same 1.22 GiB Q8_0 GGUF artifact was loaded by raw llama.cpp and OID and
  used as the source for the Ollama benchmark model.
- OID produced incremental output, completion statistics, and clean unloads.
- Ollama's structured API is required for useful phase timings; its CLI client
  RSS was not a valid server memory measurement.
- The current evidence cannot account for every raw/OID timing difference
  because raw llama-cli and the OID adapter expose different prompt/template
  and timing semantics.

## Defects found

- Live Ctrl+C cancellation was not demonstrated. The PTY control character did
  not produce a cancellation event while generation was active. This is a
  validation gap or console signal-wiring defect; it is not being silently
  marked PASS.
- The initial Ollama API capture was malformed and retained as an error
  observation; it was excluded from metrics and replaced with valid JSON
  captures.

## Changes made

- Corrected the native FFI imports required for the target llama.cpp build in
  `runtime/adapters/llama-cpp/sys/src/lib.rs`.
- Added no new runtime architecture or benchmark-only abstractions.

## Regression status

Targeted model-runner, llama.cpp adapter, and runtime tests pass; formatting
passes. Strict workspace Clippy still reports the known unrelated
`console/src/prompt.rs:64` warning.

## Platform status

```text
Linux CPU:      implemented and validated
Linux CUDA:     not implemented
Linux ROCm:     not implemented
Linux Vulkan:   not implemented
Unit OS:        architecturally preserved, not validated
macOS Metal:    architecturally preserved, not validated
```

## Recommended next work package

**WP-LLM-003 — Live cancellation signal wiring and validation.** The smallest
evidence-based follow-up is to make the console's actual Ctrl+C path cancel an
active generation, add a regression test for that path, and rerun the
cancellation portion before making performance or residency decisions.
