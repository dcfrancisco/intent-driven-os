# Marina Native Linux Qualification

This procedure qualifies the Marina model runner on a real Linux x86_64 host.
It is distinct from Docker workspace validation and from cross-compilation.
No result may be marked qualified unless the daemon loads a GGUF model and
serves real inference on the Linux host.

## Prerequisites

- Linux x86_64 host or VM with a user account and no root requirement.
- `git`, `cmake`, a C/C++ compiler, Rust stable, Cargo, `curl`, and `python3`.
- A local GGUF model under `$HOME/.marina/models` or another configured model
  directory.
- The pinned llama.cpp revision used by the compatibility matrix.

## Build the native backend

From the repository root:

```sh
./scripts/setup-llama-linux.sh
export LLAMA_CPP_LIB_DIR="$HOME/.marina/lib"
export LLAMA_CPP_REQUIRED=1
cargo build -p oid-console --release --bin marina --bin marinactl
```

The default build disables optional accelerators and conservative x86 SIMD
extensions. It is intended as the portable CPU baseline, not the fastest
possible build. Optimized builds must be qualified separately and must not
replace this baseline without evidence.

Install the binaries and native libraries into the user-scoped Marina prefix:

```sh
./scripts/install-model-runner.sh \
  --prefix "$HOME/.marina" \
  --native-lib-dir "$HOME/.marina/lib"
```

The service uses `$HOME/.marina/config.yaml`, `$HOME/.marina/models`, and the
default Unix socket `$HOME/.marina/marina.sock`. Optional loopback HTTP and
scoped bearer authentication remain governed by the existing configuration.

## Qualification

Run the existing HTTP qualification suite against the native daemon:

```sh
export MARINA_TOKEN='use-a-test-token-without-committing-it'
export MARINA_MODEL='your-local-model-id'
export QUALIFY_PLATFORM="linux-$(uname -m)"
export QUALIFY_REPORT="$PWD/docs/qualification/linux-x86_64-wp-0078.json"
./scripts/qualify-marina-http.sh
```

The native Linux run must additionally record:

- distribution and kernel version;
- CPU flags from `/proc/cpuinfo` and whether AVX/AVX2 are present;
- installed and available memory from `/proc/meminfo`;
- Rust, Cargo, compiler, CMake, and llama.cpp versions;
- native library dependencies from `ldd`;
- Unix-socket permissions and HTTP listener configuration;
- success, cancellation, timeout, recovery, admission, authorization, OID,
  and durable-evidence results.

The report schema is `marina.runtime.qualification/v1`. Compilation,
cross-compilation, or Docker-only results are recorded as unverified rather
than passed.

## Phenom II qualification

The preferred legacy-CPU target is the native Ubuntu x86_64 Phenom II X6 host
at `192.168.1.112`. Transfer the repository or use the host's checkout, run
the build and installation steps there, and execute the qualification wrapper
over SSH. Record the exact kernel, CPU flags, glibc, toolchain, model hash,
and report from the host. The target CPU exposes SSE4a but not AVX or AVX2, so
the conservative build is the required first baseline.

## GitHub Actions baseline

`.github/workflows/marina-linux-qualification.yml` provides a native Ubuntu
x86_64 baseline. Start it manually with a small public GGUF URL, its SHA-256,
and a model ID. It downloads the model transiently, runs real inference, and
uploads only logs and JSON evidence. Passing on GitHub's runner does not
qualify the Phenom host.
