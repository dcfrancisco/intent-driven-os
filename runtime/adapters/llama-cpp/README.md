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
`libllama` when building live inference. The local Marina default is
`$HOME/.marina/models` (for the current machine:
`/Users/dannyfrancisco/.marina/models`):

```sh
export LLAMA_CPP_LIB_DIR="${LLAMA_CPP_LIB_DIR:-$HOME/.marina/models}"
```

Without this configuration, normal workspace builds remain portable and the
adapter reports `BackendUnavailable`.
