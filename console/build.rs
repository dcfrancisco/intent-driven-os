#![allow(missing_docs)]

use std::env;
use std::fs;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../.git/HEAD");
    println!("cargo:rerun-if-changed=../.git/index");
    if let Ok(head) = fs::read_to_string("../.git/HEAD") {
        if let Some(reference) = head.strip_prefix("ref: ") {
            println!("cargo:rerun-if-changed=../.git/{}", reference.trim());
        }
    }
    println!("cargo:rerun-if-env-changed=TALVIK_GIT_SHA");
    println!("cargo:rerun-if-env-changed=TALVIK_GIT_DIRTY");
    println!(
        "cargo:rustc-env=TALVIK_BUILD_TARGET={}",
        env::var("TARGET").unwrap_or_else(|_| "unknown".to_owned())
    );
    println!(
        "cargo:rustc-env=TALVIK_BUILD_PROFILE={}",
        env::var("PROFILE").unwrap_or_else(|_| "unknown".to_owned())
    );
    println!(
        "cargo:rustc-env=TALVIK_GIT_SHA={}",
        env::var("TALVIK_GIT_SHA").unwrap_or_else(|_| git_output(&["rev-parse", "HEAD"]))
    );
    println!(
        "cargo:rustc-env=TALVIK_GIT_DIRTY={}",
        env::var("TALVIK_GIT_DIRTY").unwrap_or_else(|_| git_dirty())
    );
    let argument = if env::var_os("CARGO_CFG_TARGET_OS").as_deref() == Some("macos".as_ref()) {
        "-Wl,-rpath,@loader_path/../lib"
    } else if env::var_os("CARGO_CFG_TARGET_FAMILY").as_deref() == Some("unix".as_ref()) {
        "-Wl,-rpath,$ORIGIN/../lib"
    } else {
        return;
    };
    for binary in ["talvik", "talvikd", "oid-console"] {
        println!("cargo:rustc-link-arg-bin={binary}={argument}");
    }
}

fn git_output(arguments: &[&str]) -> String {
    Command::new("git")
        .args(arguments)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_owned())
}

fn git_dirty() -> String {
    let tracked = Command::new("git")
        .args(["diff", "--quiet", "HEAD", "--"])
        .status()
        .is_ok_and(|status| !status.success());
    tracked.to_string()
}
