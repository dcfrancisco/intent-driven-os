# llama.cpp Adapter

This crate implements OID's `oid-model-runner::ModelRunner` contract using the
official llama.cpp C API through the adjacent `oid-llama-cpp-sys` crate.

The dependency direction is intentional:

```text
oid-runtime → oid-model-runner
oid-llama-cpp-adapter → oid-model-runner + oid-llama-cpp-sys
```

The runtime bridge translates the existing service API to the model-runner
contract. No llama.cpp types cross the model-runner boundary.

Native linking is opt-in. Set `LLAMA_CPP_LIB_DIR` to a directory containing
`libllama` when building live inference. The local Marina native-library
default is `$HOME/.marina/lib` (for the current machine:
`/Users/dannyfrancisco/.marina/lib`):

```sh
export LLAMA_CPP_LIB_DIR="${LLAMA_CPP_LIB_DIR:-$HOME/.marina/lib}"
```

Without this configuration, normal workspace builds remain portable and the
adapter reports `BackendUnavailable`.

On Windows, `scripts/setup-llama-windows.ps1` builds the pinned CPU library by
default. Pass `-Vulkan` from a Visual Studio Developer PowerShell after
installing the Vulkan SDK to build a separate Vulkan-enabled library and
`llama-cli` under the temporary build directory. The Vulkan native libraries
are placed in `$HOME/.marina/lib-vulkan` by default, leaving the CPU library
untouched. Run the printed `llama-cli --list-devices` command to confirm
llama.cpp can enumerate the GPU.

This only builds the Vulkan-capable llama.cpp artifacts. Talvik does not yet
select GPU layers or qualify Vulkan inference; see
[WP-0093](../../../docs/work-packages/WP-0093-talvik-vulkan-acceleration.md).

The native wrapper treats `llama_decode` return value `2` as cancellation only
when Marina's cancellation flag is active. The same status without an active
cancellation, and all other nonzero decode statuses, remain native failures.
The upstream llama.cpp graph logger may still print error-looking diagnostics
for an intentional abort.
