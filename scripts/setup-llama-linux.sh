#!/usr/bin/env bash
set -euo pipefail

# Build the pinned llama.cpp CPU backend for a native Linux x86_64 Marina
# installation. The checkout and build remain outside the repository.
# This deliberately uses a conservative instruction-set baseline: operators
# may create a separate optimized build, but the default does not require
# AVX, AVX2, AVX-512, FMA, F16C, SSE4.2, or BMI2.
LLAMA_CPP_ROOT="${LLAMA_CPP_ROOT:-${TMPDIR:-/tmp}/oid-llama.cpp}"
LLAMA_CPP_BUILD="${LLAMA_CPP_BUILD:-${TMPDIR:-/tmp}/oid-llama-linux-cpu-build}"
LLAMA_CPP_INSTALL_DIR="${LLAMA_CPP_INSTALL_DIR:-${HOME}/.marina/lib}"
LLAMA_CPP_REPOSITORY="${LLAMA_CPP_REPOSITORY:-https://github.com/ggml-org/llama.cpp.git}"
LLAMA_CPP_COMMIT="${LLAMA_CPP_COMMIT:-18443257a30c884d5332abb8e7dc43c7ffe42fda}"

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
  -DGGML_METAL=OFF \
  -DGGML_BLAS=OFF \
  -DLLAMA_BUILD_TESTS=OFF \
  -DLLAMA_BUILD_EXAMPLES=OFF \
  -DLLAMA_CURL=OFF \
  -DCMAKE_BUILD_RPATH='$ORIGIN' \
  -DCMAKE_BUILD_RPATH_USE_ORIGIN=ON \
  -DCMAKE_INSTALL_RPATH='$ORIGIN' \
  -DCMAKE_INSTALL_RPATH_USE_LINK_PATH=OFF \
  -DCMAKE_BUILD_TYPE=Release
cmake --build "$LLAMA_CPP_BUILD" --target llama --parallel "${CMAKE_BUILD_PARALLEL_LEVEL:-2}"

mkdir -p "$LLAMA_CPP_INSTALL_DIR"
find "$LLAMA_CPP_BUILD/bin" -maxdepth 1 \
  \( -type f -o -type l \) \
  \( -name 'libllama.so' -o -name 'libllama.so.*' -o -name 'libggml*.so' -o -name 'libggml*.so.*' \) \
  -exec cp -a {} "$LLAMA_CPP_INSTALL_DIR" \;

test -e "$LLAMA_CPP_INSTALL_DIR/libllama.so" || {
  echo "libllama.so was not produced in $LLAMA_CPP_INSTALL_DIR" >&2
  exit 1
}

echo
echo "Native Linux x86_64 llama.cpp is installed at:"
echo "$LLAMA_CPP_INSTALL_DIR"
echo "Build Marina with:"
echo "LLAMA_CPP_LIB_DIR=$LLAMA_CPP_INSTALL_DIR LLAMA_CPP_REQUIRED=1 cargo build -p oid-console --release --bin marina --bin marinactl"
echo "Run with:"
echo "LD_LIBRARY_PATH=$LLAMA_CPP_INSTALL_DIR \$HOME/.marina/bin/marina"
