#![allow(missing_docs)]

use std::env;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let argument = if env::var_os("CARGO_CFG_TARGET_OS").as_deref() == Some("macos".as_ref()) {
        "-Wl,-rpath,@loader_path/../lib"
    } else if env::var_os("CARGO_CFG_TARGET_FAMILY").as_deref() == Some("unix".as_ref()) {
        "-Wl,-rpath,$ORIGIN/../lib"
    } else {
        return;
    };
    for binary in ["marina", "marinactl", "oid-console"] {
        println!("cargo:rustc-link-arg-bin={binary}={argument}");
    }
}
