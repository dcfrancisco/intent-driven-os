#![allow(missing_docs)]

use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rustc-check-cfg=cfg(native_llama_cpp)");
    println!("cargo:rerun-if-env-changed=LLAMA_CPP_LIB_DIR");
    println!("cargo:rerun-if-env-changed=LLAMA_CPP_LIBRARY");
    println!("cargo:rerun-if-env-changed=LLAMA_CPP_REQUIRED");

    let configured_dir = env::var_os("LLAMA_CPP_LIB_DIR").map(PathBuf::from);
    let default_dir = env::var_os("HOME").map(|home| PathBuf::from(home).join(".marina/models"));
    let Some(lib_dir) = configured_dir.or(default_dir) else {
        assert!(
            env::var_os("LLAMA_CPP_REQUIRED").is_none(),
            "LLAMA_CPP_REQUIRED is set but LLAMA_CPP_LIB_DIR and HOME are not configured"
        );
        return;
    };
    let library = env::var("LLAMA_CPP_LIBRARY").unwrap_or_else(|_| "llama".to_owned());
    let library_name = library.as_str();

    let has_library = [
        lib_dir.join(format!("lib{library_name}.dylib")),
        lib_dir.join(format!("lib{library_name}.so")),
        lib_dir.join(format!("{library_name}.lib")),
    ]
    .iter()
    .any(|path| path.exists());

    if !lib_dir.is_dir() || !has_library {
        assert!(
            env::var_os("LLAMA_CPP_REQUIRED").is_none(),
            "configured LLAMA_CPP_LIB_DIR does not contain lib{library_name}: {}",
            lib_dir.display()
        );
        return;
    }

    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=dylib={library_name}");
    println!("cargo:rustc-cfg=native_llama_cpp");
}
