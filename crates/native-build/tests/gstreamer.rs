use cove_native_build::gstreamer::{Build, Platform};
use std::path::PathBuf;

fn plan(target: &str) -> Build {
    Build {
        source: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../third-party/gstreamer"),
        out_dir: "target/native-media-plan".into(),
        target: target.into(),
        host: "aarch64-apple-darwin".into(),
        cross_file: None,
        jobs: 4,
    }
}
#[test]
fn every_native_platform_selects_a_real_audio_sink_and_decoder_profile() {
    for (target, platform, audio, decoder) in [
        (
            "aarch64-apple-darwin",
            Platform::Mac,
            "gst-plugins-good:osxaudio",
            "gst-plugins-bad:applemedia",
        ),
        (
            "aarch64-apple-ios",
            Platform::Ios,
            "gst-plugins-good:osxaudio",
            "gst-plugins-bad:applemedia",
        ),
        (
            "aarch64-apple-ios-sim",
            Platform::Ios,
            "gst-plugins-good:osxaudio",
            "gst-plugins-bad:applemedia",
        ),
        (
            "x86_64-pc-windows-msvc",
            Platform::Windows,
            "gst-plugins-bad:wasapi",
            "gst-plugins-bad:d3d11",
        ),
        (
            "x86_64-unknown-linux-gnu",
            Platform::Linux,
            "gst-plugins-base:alsa",
            "gst-plugins-base:rawparse",
        ),
        (
            "aarch64-linux-android",
            Platform::Android,
            "gst-plugins-bad:opensles",
            "gst-plugins-bad:androidmedia",
        ),
        (
            "x86_64-linux-android",
            Platform::Android,
            "gst-plugins-bad:opensles",
            "gst-plugins-bad:androidmedia",
        ),
    ] {
        let build = plan(target);
        assert_eq!(build.platform().expect("native platform"), platform);
        let options = build.options().expect("build profile");
        assert!(options.contains(&format!("-D{audio}=enabled")), "{target}");
        assert!(
            options.contains(&format!("-D{decoder}=enabled")),
            "{target}"
        );
        assert!(options.contains(&"--auto-features=disabled".into()));
        assert!(options.contains(&"--default-library=static".into()));
        assert!(options.contains(&"--wrap-mode=forcefallback".into()));
    }
}
#[test]
fn selected_plugin_options_exist_in_the_pinned_upstream() {
    for target in [
        "aarch64-apple-darwin",
        "aarch64-apple-ios",
        "x86_64-pc-windows-msvc",
        "x86_64-unknown-linux-gnu",
        "aarch64-linux-android",
    ] {
        let build = plan(target);
        for option in build.options().expect("options") {
            let Some((project, option)) = option.strip_prefix("-D").and_then(|s| s.split_once(':'))
            else {
                continue;
            };
            let name = option.split_once('=').expect("option assignment").0;
            if !project.starts_with("gst-") {
                continue;
            }
            let source = std::fs::read_to_string(
                build
                    .source
                    .join("subprojects")
                    .join(project)
                    .join("meson.options"),
            )
            .expect("upstream options");
            assert!(
                source.contains(&format!("option('{name}'")),
                "{target}: {project}:{name}"
            );
        }
    }
}
#[test]
fn the_browser_is_not_misrepresented_as_a_native_gstreamer_target() {
    assert!(plan("wasm32-wasip1").platform().is_err());
}
