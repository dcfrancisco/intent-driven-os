#![allow(missing_docs)]

use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rustc-check-cfg=cfg(native_llama_cpp)");
    println!("cargo:rerun-if-env-changed=LLAMA_CPP_LIB_DIR");
    println!("cargo:rerun-if-env-changed=LLAMA_CPP_LIBRARY");
    println!("cargo:rerun-if-env-changed=LLAMA_CPP_REQUIRED");

    let Some(lib_dir) = env::var_os("LLAMA_CPP_LIB_DIR").map(PathBuf::from) else {
        assert!(
            env::var_os("LLAMA_CPP_REQUIRED").is_none(),
            "LLAMA_CPP_REQUIRED is set but LLAMA_CPP_LIB_DIR is not configured"
        );
        return;
    };

    if !lib_dir.is_dir() {
        assert!(
            env::var_os("LLAMA_CPP_REQUIRED").is_none(),
            "configured LLAMA_CPP_LIB_DIR does not exist: {}",
            lib_dir.display()
        );
        return;
    }

    let library = env::var("LLAMA_CPP_LIBRARY").unwrap_or_else(|_| "llama".to_owned());
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=dylib={library}");
    println!("cargo:rustc-cfg=native_llama_cpp");
}
