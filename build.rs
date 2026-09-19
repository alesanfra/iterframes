use std::env;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=CC");
    let statik = env::var_os("CARGO_FEATURE_STATIC").is_some();
    let macos = env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos");
    if statik && macos {
        link_compiler_rt();
    }
}

/// FFmpeg's VideoToolbox code guards newer APIs with `@available`, which
/// clang compiles to calls into its compiler-rt builtins. rustc links with
/// `-nodefaultlibs`, so the static FFmpeg needs them linked explicitly.
fn link_compiler_rt() {
    let cc = env::var("CC").unwrap_or_else(|_| "clang".to_owned());
    let output = Command::new(&cc)
        .arg("--print-runtime-dir")
        .output()
        .unwrap_or_else(|err| panic!("cannot run {cc}: {err}"));
    assert!(
        output.status.success(),
        "{cc} --print-runtime-dir failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let dir = String::from_utf8(output.stdout).expect("runtime dir is not UTF-8");
    println!("cargo:rustc-link-search=native={}", dir.trim());
    println!("cargo:rustc-link-lib=static=clang_rt.osx");
}
