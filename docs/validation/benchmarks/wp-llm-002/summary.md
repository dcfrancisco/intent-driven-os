# WP-LLM-002 evidence summary

Status: **PARTIAL**.

Ubuntu live inference succeeded on an AMD Phenom II X6 1055T using a portable
llama.cpp build with AVX, AVX2, AVX-512, FMA, F16C, SSE4.2, and BMI2 disabled.
Raw llama.cpp and OID each have five cold-process observations for Prompt A.
Ollama has one cold and two warm structured API observations using the same
Q8_0 GGUF source artifact.

Median Prompt A observations:

| Engine/state | Generation tok/s | Total wall/server time | RSS |
| --- | ---: | ---: | ---: |
| llama.cpp/cold | 3.9 | 19.94 s | 1,505,856 KiB |
| OID/cold | 5.2 | 25.35 s | 1,484,848 KiB |
| Ollama/cold | 1.22 | 298.80 s | ~1.14 GiB server |
| Ollama/warm | 1.22 | 118.86 s (two-run median) | ~1.14 GiB server |

The raw/OID throughput values are not sufficient to claim superiority because
llama-cli and the OID adapter exposed different response/timing semantics.
Live OID streaming, completion, and unload passed. Live Ctrl+C cancellation
did not produce a cancellation event, so the work package remains partial.
