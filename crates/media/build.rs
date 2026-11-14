//! Exposes the pinned FFmpeg CLI directory to the crate when `ffmpeg-tools` is on.

use std::env;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=COVE_FFMPEG_CLI_DIR");

    if env::var("CARGO_FEATURE_FFMPEG_TOOLS").is_err() {
        return;
    }

    // An explicit directory wins; otherwise use whatever the pinned ffmpeg-sys-next
    // build installed, so the CLIs always match the libraries in the same binary.
    let directory = env::var("COVE_FFMPEG_CLI_DIR")
        .ok()
        .or_else(|| env::var("DEP_FFMPEG_BIN_DIR").ok())
        .filter(|directory| !directory.is_empty());

    if let Some(directory) = directory {
        println!("cargo:rustc-env=COVE_FFMPEG_CLI_DIR={directory}");
    }
}
