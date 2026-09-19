//! Build the static FFmpeg that the extension links, then link it and
//! generate the bindings to the parts of it that `src/ffmpeg.rs` uses.
//!
//! FFmpeg is built once by `scripts/build-ffmpeg.sh` into `build/ffmpeg`
//! (or `$ITERFRAMES_FFMPEG_DIR`), which later builds reuse: the script does
//! nothing when that directory already holds the versions it would build.

use std::env;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The headers bindgen reads.
const HEADERS: &str = "
#include <libavcodec/avcodec.h>
#include <libavformat/avformat.h>
#include <libavutil/error.h>
#include <libavutil/hwcontext.h>
#include <libavutil/opt.h>
#include <libswscale/swscale.h>
";

fn main() {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("set by Cargo"));
    let script = root.join("scripts/build-ffmpeg.sh");
    println!("cargo:rerun-if-changed={}", script.display());
    println!("cargo:rerun-if-env-changed=ITERFRAMES_FFMPEG_DIR");
    let prefix = env::var_os("ITERFRAMES_FFMPEG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("build/ffmpeg"));

    build_ffmpeg(&script, &prefix);
    // Rebuild when the FFmpeg build is replaced or deleted.
    println!(
        "cargo:rerun-if-changed={}",
        prefix.join("VERSION").display()
    );
    let include_paths = link_ffmpeg(&prefix);
    generate_bindings(&include_paths);
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        link_compiler_rt();
    }
}

/// Run the build script, which returns at once when FFmpeg is up to date.
fn build_ffmpeg(script: &Path, prefix: &Path) {
    let parent = prefix.parent().expect("the FFmpeg prefix has a parent");
    fs::create_dir_all(parent).expect("cannot create the FFmpeg build directory");
    // rust-analyzer, clippy, and maturin may build at the same time; only
    // one of them may run the script. The lock goes with the process.
    let lock = File::create(prefix.with_extension("lock")).expect("cannot create the lock");
    lock.lock().expect("cannot lock the FFmpeg build");

    let log = prefix.with_extension("log");
    let status = Command::new(bash())
        .arg(script)
        .arg(prefix)
        .stdout(File::create(&log).expect("cannot create the build log"))
        .stderr(
            File::options()
                .append(true)
                .open(&log)
                .expect("cannot open the build log"),
        )
        // Cargo's variables for build scripts would reach make and meson
        // as variables of their own.
        .env_remove("TARGET")
        .env_remove("HOST")
        .env_remove("DEBUG")
        .env_remove("PROFILE")
        .env_remove("OPT_LEVEL")
        .env_remove("OUT_DIR")
        .env_remove("NUM_JOBS")
        .env_remove("MAKEFLAGS")
        .env_remove("MFLAGS")
        .env_remove("CARGO_MAKEFLAGS")
        .status()
        .expect("cannot run bash");
    if !status.success() {
        let log_text = fs::read_to_string(&log).unwrap_or_default();
        let tail: Vec<_> = log_text.lines().rev().take(40).collect();
        let tail: Vec<_> = tail.into_iter().rev().collect();
        panic!(
            "building FFmpeg failed; full log in {}\n{}",
            log.display(),
            tail.join("\n")
        );
    }
}

/// The shell that runs the build script. On Windows, Rust looks for a bare
/// `bash` in the system directory before `PATH`, and would find WSL's
/// there instead of MSYS2's.
fn bash() -> PathBuf {
    if cfg!(windows) {
        let path = env::var_os("PATH").unwrap_or_default();
        if let Some(bash) = env::split_paths(&path)
            .map(|dir| dir.join("bash.exe"))
            .find(|bash| bash.is_file())
        {
            return bash;
        }
    }
    PathBuf::from("bash")
}

/// Link the static libraries and return their include paths.
fn link_ffmpeg(prefix: &Path) -> Vec<PathBuf> {
    // SAFETY: the build script is single-threaded.
    unsafe { env::set_var("PKG_CONFIG_PATH", prefix.join("lib/pkgconfig")) };
    let mut include_paths = Vec::new();
    for library in ["libavformat", "libavcodec", "libswscale", "libavutil"] {
        let library = pkg_config::Config::new()
            .statik(true)
            .cargo_metadata(true)
            .probe(library)
            .unwrap_or_else(|err| panic!("{err}"));
        include_paths.extend(library.include_paths);
    }
    include_paths
}

fn generate_bindings(include_paths: &[PathBuf]) {
    let out = PathBuf::from(env::var("OUT_DIR").expect("set by Cargo")).join("ffmpeg.rs");
    bindgen::Builder::default()
        .header_contents("ffmpeg.h", HEADERS)
        .clang_args(
            include_paths
                .iter()
                .map(|path| format!("-I{}", path.display())),
        )
        .allowlist_function("(av|avcodec|avformat|sws)_.*")
        .allowlist_type("(AV|Sws).*")
        .allowlist_var("(AV|FF)_.*")
        // Constants in a transparent struct: an unknown value from C is
        // then a value, not undefined behavior as with a Rust enum.
        .default_enum_style(bindgen::EnumVariation::NewType {
            is_bitfield: false,
            is_global: false,
        })
        .layout_tests(false)
        .generate_comments(false)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("cannot generate the FFmpeg bindings")
        .write_to_file(out)
        .expect("cannot write the FFmpeg bindings");
}

/// FFmpeg's VideoToolbox code guards newer APIs with `@available`, which
/// clang compiles to calls into its compiler-rt builtins. rustc links with
/// `-nodefaultlibs`, so they must be linked explicitly.
fn link_compiler_rt() {
    println!("cargo:rerun-if-env-changed=CC");
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
