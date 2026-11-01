#!/usr/bin/env bash
set -euo pipefail

# Build the pinned llama.cpp CPU, CUDA, or Vulkan backend for native Linux
# x86_64. GPU variants remain explicit and require their vendor toolchains.
# This deliberately uses a conservative instruction-set baseline: operators
# may create a separate optimized build, but the default does not require
# AVX, AVX2, AVX-512, FMA, F16C, SSE4.2, or BMI2.
LLAMA_CPP_ROOT="${LLAMA_CPP_ROOT:-${TMPDIR:-/tmp}/oid-llama.cpp}"
TALVIK_LLAMA_BACKEND="${TALVIK_LLAMA_BACKEND:-cpu}"
LLAMA_CPP_BUILD="${LLAMA_CPP_BUILD:-${TMPDIR:-/tmp}/oid-llama-linux-${TALVIK_LLAMA_BACKEND}-build}"
if [[ -z "${LLAMA_CPP_INSTALL_DIR:-}" ]]; then
  if [[ "$TALVIK_LLAMA_BACKEND" == cpu ]]; then
    LLAMA_CPP_INSTALL_DIR="${HOME}/.marina/lib"
  else
    LLAMA_CPP_INSTALL_DIR="${HOME}/.marina/lib-${TALVIK_LLAMA_BACKEND}"
  fi
fi
LLAMA_CPP_REPOSITORY="${LLAMA_CPP_REPOSITORY:-https://github.com/ggml-org/llama.cpp.git}"
LLAMA_CPP_COMMIT="${LLAMA_CPP_COMMIT:-18443257a30c884d5332abb8e7dc43c7ffe42fda}"

case "$TALVIK_LLAMA_BACKEND" in
  cpu) GGML_CUDA=OFF; GGML_VULKAN=OFF ;;
  cuda) GGML_CUDA=ON; GGML_VULKAN=OFF; command -v nvcc >/dev/null || { echo "nvcc is required for CUDA." >&2; exit 1; } ;;
  vulkan) GGML_CUDA=OFF; GGML_VULKAN=ON; command -v glslc >/dev/null || { echo "glslc from the Vulkan SDK is required." >&2; exit 1; } ;;
  *) echo "TALVIK_LLAMA_BACKEND must be cpu, cuda, or vulkan." >&2; exit 2 ;;
esac

[[ "$(uname -s)" == "Linux" ]] || {
  echo "This script requires Linux; detected $(uname -s)." >&2
  exit 1
}
case "$(uname -m)" in
  x86_64|amd64) ;;
  *)
    echo "This qualification script requires Linux x86_64; detected $(uname -m)." >&2
    exit 1
    ;;
esac

command -v git >/dev/null || { echo "git is required." >&2; exit 1; }
command -v cmake >/dev/null || { echo "cmake is required." >&2; exit 1; }

if [[ ! -d "$LLAMA_CPP_ROOT/.git" ]]; then
  git clone --depth 1 "$LLAMA_CPP_REPOSITORY" "$LLAMA_CPP_ROOT"
fi
git -C "$LLAMA_CPP_ROOT" fetch --depth 1 origin "$LLAMA_CPP_COMMIT"
git -C "$LLAMA_CPP_ROOT" checkout --detach "$LLAMA_CPP_COMMIT"

cmake -S "$LLAMA_CPP_ROOT" -B "$LLAMA_CPP_BUILD" \
  -DGGML_NATIVE=OFF \
  -DGGML_AVX=OFF \
  -DGGML_AVX2=OFF \
  -DGGML_AVX512=OFF \
  -DGGML_FMA=OFF \
  -DGGML_F16C=OFF \
  -DGGML_SSE42=OFF \
  -DGGML_BMI2=OFF \
  -DGGML_OPENMP=OFF \
  -DGGML_CUDA="$GGML_CUDA" \
  -DGGML_VULKAN="$GGML_VULKAN" \
  -DGGML_METAL=OFF \
  -DGGML_BLAS=OFF \
  -DLLAMA_BUILD_TESTS=OFF \
  -DLLAMA_BUILD_EXAMPLES=OFF \
  -DLLAMA_BUILD_MTMD=ON \
  -DMTMD_VIDEO=OFF \
  -DLLAMA_CURL=OFF \
  -DCMAKE_BUILD_RPATH='$ORIGIN' \
  -DCMAKE_BUILD_RPATH_USE_ORIGIN=ON \
  -DCMAKE_INSTALL_RPATH='$ORIGIN' \
  -DCMAKE_INSTALL_RPATH_USE_LINK_PATH=OFF \
  -DCMAKE_BUILD_TYPE=Release
cmake --build "$LLAMA_CPP_BUILD" --target llama mtmd --parallel "${CMAKE_BUILD_PARALLEL_LEVEL:-2}"

mkdir -p "$LLAMA_CPP_INSTALL_DIR"
find "$LLAMA_CPP_BUILD/bin" -maxdepth 1 \
  \( -type f -o -type l \) \
  \( -name 'libllama.so' -o -name 'libllama.so.*' -o -name 'libmtmd.so' -o -name 'libmtmd.so.*' -o -name 'libggml*.so' -o -name 'libggml*.so.*' \) \
  -exec cp -a {} "$LLAMA_CPP_INSTALL_DIR" \;

test -e "$LLAMA_CPP_INSTALL_DIR/libllama.so" || {
  echo "libllama.so was not produced in $LLAMA_CPP_INSTALL_DIR" >&2
  exit 1
}
test -e "$LLAMA_CPP_INSTALL_DIR/libmtmd.so" || {
  echo "libmtmd.so was not produced in $LLAMA_CPP_INSTALL_DIR" >&2
  exit 1
}

LLAMA_CPP_ROOT="$LLAMA_CPP_ROOT" LLAMA_CPP_INSTALL_DIR="$LLAMA_CPP_INSTALL_DIR" TALVIK_CHAT_TEMPLATE_BUILD="$LLAMA_CPP_BUILD/chat-template" CMAKE_COMMAND=cmake bash "$(dirname "$0")/setup-chat-template.sh"

echo
echo "Native Linux x86_64 llama.cpp is installed at:"
echo "$LLAMA_CPP_INSTALL_DIR"
echo "Build Talvik with:"
echo "LLAMA_CPP_LIB_DIR=$LLAMA_CPP_INSTALL_DIR LLAMA_CPP_REQUIRED=1 LLAMA_CPP_MTMD_REQUIRED=1 LLAMA_CPP_CHAT_TEMPLATE_REQUIRED=1 TALVIK_BACKEND=$TALVIK_LLAMA_BACKEND cargo build -p oid-console --release --bin talvik --bin talvikd"
echo "Run with:"
echo "LD_LIBRARY_PATH=$LLAMA_CPP_INSTALL_DIR \$HOME/.marina/bin/talvikd"
