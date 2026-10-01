#!/usr/bin/env bash
set -euo pipefail

# Build the native llama.cpp library used by OID's existing adapter.
# The checkout and build stay outside the repository; only the resulting
# library directory needs to be supplied to Cargo.
LLAMA_CPP_ROOT="${LLAMA_CPP_ROOT:-${TMPDIR:-/tmp}/oid-llama.cpp}"
LLAMA_CPP_BUILD="${LLAMA_CPP_BUILD:-${TMPDIR:-/tmp}/oid-llama-cpu-build}"
LLAMA_CPP_REPOSITORY="${LLAMA_CPP_REPOSITORY:-https://github.com/ggml-org/llama.cpp.git}"
LLAMA_CPP_COMMIT="${LLAMA_CPP_COMMIT:-18443257a30c884d5332abb8e7dc43c7ffe42fda}"

OS_NAME="$(uname -s)"
MACHINE_ARCH="$(uname -m)"
ACCELERATOR="${OID_LLAMA_ACCELERATOR:-auto}"
case "$OS_NAME:$MACHINE_ARCH" in
  Darwin:arm64|Darwin:aarch64)
    DEFAULT_ACCELERATOR=metal
    ;;
  Darwin:x86_64)
    DEFAULT_ACCELERATOR=cpu
    ;;
  *)
    echo "This script supports macOS only; detected $OS_NAME/$MACHINE_ARCH." >&2
    echo "Use the same CMake options with a Linux or Windows build toolchain." >&2
    exit 1
    ;;
esac

if [[ "$ACCELERATOR" == auto ]]; then
  ACCELERATOR="$DEFAULT_ACCELERATOR"
fi
case "$ACCELERATOR" in
  cpu|metal) ;;
  *) echo "OID_LLAMA_ACCELERATOR must be auto, cpu, or metal." >&2; exit 2 ;;
esac

GGML_NATIVE=ON
GGML_METAL=OFF
if [[ "$ACCELERATOR" == metal ]]; then
  GGML_METAL=ON
fi

if [[ ! -d "$LLAMA_CPP_ROOT/.git" ]]; then
  git clone --depth 1 "$LLAMA_CPP_REPOSITORY" "$LLAMA_CPP_ROOT"
fi
git -C "$LLAMA_CPP_ROOT" fetch --depth 1 origin "$LLAMA_CPP_COMMIT"
git -C "$LLAMA_CPP_ROOT" checkout --detach "$LLAMA_CPP_COMMIT"

cmake_command="$(command -v cmake || true)"
if [[ -z "$cmake_command" && -x /usr/local/opt/cmake/bin/cmake ]]; then
  cmake_command=/usr/local/opt/cmake/bin/cmake
fi
if [[ -z "$cmake_command" ]]; then
  echo "cmake is required (install it with Homebrew or provide it on PATH)." >&2
  exit 1
fi

"$cmake_command" -S "$LLAMA_CPP_ROOT" -B "$LLAMA_CPP_BUILD" \
  -DGGML_NATIVE="$GGML_NATIVE" \
  -DGGML_OPENMP=OFF \
  -DGGML_METAL="$GGML_METAL" \
  -DGGML_BLAS=OFF \
  -DLLAMA_BUILD_TESTS=OFF \
  -DLLAMA_BUILD_EXAMPLES=OFF \
  -DLLAMA_CURL=OFF \
  -DCMAKE_BUILD_TYPE=Release
"$cmake_command" --build "$LLAMA_CPP_BUILD" --target llama -j2

echo
echo "OID native llama.cpp is ready for $OS_NAME/$MACHINE_ARCH ($ACCELERATOR). Build/run OID with:"
echo "LLAMA_CPP_LIB_DIR=$LLAMA_CPP_BUILD/bin LLAMA_CPP_REQUIRED=1 DYLD_LIBRARY_PATH=$LLAMA_CPP_BUILD/bin cargo run --release"
