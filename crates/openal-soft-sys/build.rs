use std::env;
use std::path::PathBuf;
use std::process::Command;

/// Pinned OpenAL Soft release tag. Update this to pull a newer version.
const OPENAL_SOFT_TAG: &str = "1.25.1";

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let src_dir = out_dir.join("openal-soft-src");

    // Clone OpenAL Soft at pinned release tag if not already present.
    if !src_dir.join("CMakeLists.txt").exists() {
        eprintln!("openal-soft-sys: Cloning OpenAL Soft {} ...", OPENAL_SOFT_TAG);
        let status = Command::new("git")
            .args([
                "clone",
                "--depth",
                "1",
                "--branch",
                OPENAL_SOFT_TAG,
                "https://github.com/kcat/openal-soft.git",
            ])
            .arg(src_dir.to_str().unwrap())
            .status()
            .expect("Failed to run git. Is git installed and in PATH?");
        assert!(
            status.success(),
            "git clone of OpenAL Soft {} failed",
            OPENAL_SOFT_TAG
        );
    }

    // Build OpenAL Soft as a static library via cmake.
    // Always build Release to avoid CRT mismatch (debug CRT symbols like
    // __imp__CrtDbgReport are not available when Rust links with release CRT).
    let mut cfg = cmake::Config::new(&src_dir);
    cfg.profile("Release")
        .define("LIBTYPE", "STATIC")
        .define("ALSOFT_UTILS", "OFF")
        .define("ALSOFT_EXAMPLES", "OFF")
        .define("ALSOFT_TESTS", "OFF")
        .define("ALSOFT_INSTALL", "ON")
        .define("ALSOFT_INSTALL_CONFIG", "OFF")
        .define("ALSOFT_INSTALL_HRTF_DATA", "OFF")
        .define("ALSOFT_INSTALL_AMBDEC_PRESETS", "OFF");

    let dst = cfg.build();

    // Link search paths — cmake installs to {dst}/lib
    println!("cargo:rustc-link-search=native={}/lib", dst.display());

    if cfg!(target_os = "windows") {
        // Static OpenAL on Windows (MSVC produces OpenAL32.lib)
        println!("cargo:rustc-link-lib=static=OpenAL32");
        // System libraries required by OpenAL Soft audio backends
        println!("cargo:rustc-link-lib=winmm");
        println!("cargo:rustc-link-lib=ole32");
    } else if cfg!(target_os = "linux") {
        println!("cargo:rustc-link-lib=static=openal");
        println!("cargo:rustc-link-lib=pthread");
        println!("cargo:rustc-link-lib=dl");
        println!("cargo:rustc-link-lib=m");
    } else if cfg!(target_os = "macos") {
        println!("cargo:rustc-link-lib=static=openal");
        println!("cargo:rustc-link-lib=framework=AudioToolbox");
        println!("cargo:rustc-link-lib=framework=CoreAudio");
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
    }
}
