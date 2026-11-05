//! Pinned, out-of-source GStreamer build plans. `cargo cove setup --media`
//! builds the target's prefix and writes the pkg-config environment into the
//! project's Cargo config, so plain `cargo build` needs no separate native step.
use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};

pub const REVISION: &str = "1b2a95a1b72787cda146b46df9605b168e5b683b";
pub const VERSION: &str = "1.28.8";
pub use super::media_provider::{cached_build, prepare};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Mac,
    Ios,
    Windows,
    Linux,
    Android,
}
#[derive(Clone, Debug)]
pub struct Build {
    pub source: PathBuf,
    pub out_dir: PathBuf,
    pub target: String,
    pub host: String,
    /// For non-Apple/non-Android cross toolchains; never reuse host pkg-config.
    pub cross_file: Option<PathBuf>,
    pub jobs: usize,
}
impl Build {
    pub fn platform(&self) -> io::Result<Platform> {
        platform(&self.target)
    }
    pub fn prefix(&self) -> PathBuf {
        self.out_dir.join("install")
    }
    pub fn options(&self) -> io::Result<Vec<String>> {
        let mut options: Vec<String> = [
            "--default-library=static", "--buildtype=release", "--libdir=lib", "-Db_staticpic=true", "--auto-features=disabled",
            "--wrap-mode=forcefallback", "-Dgst-full=enabled", "-Dgst-full-target-type=static_library",
            "-Dgst-full-libraries=gstreamer-base-1.0,gstreamer-app-1.0,gstreamer-video-1.0,gstreamer-audio-1.0",
            "-Dbase=enabled", "-Dgood=enabled", "-Dbad=enabled", "-Dugly=disabled",
            "-Dlibav=disabled", "-Dgpl=disabled", "-Dbuild-tools-source=system", "-Dtools=enabled",
            "-Dtests=disabled", "-Dglib:tests=false", "-Dglib:installed_tests=false",
            "-Dgstreamer:registry=false",
        ].iter().map(|s| (*s).into()).collect();
        for name in [
            "app",
            "playback",
            "typefind",
            "audioconvert",
            "audioresample",
            "videoconvertscale",
            "videotestsrc",
            "audiotestsrc",
            "volume",
            "rawparse",
            "subparse",
        ] {
            options.push(format!("-Dgst-plugins-base:{name}=enabled"));
        }
        for name in [
            "avi",
            "wavparse",
            "wavenc",
            "audioparsers",
            "isomp4",
            "matroska",
            "autodetect",
        ] {
            options.push(format!("-Dgst-plugins-good:{name}=enabled"));
        }
        options.push("-Dgst-plugins-bad:videoparsers=enabled".into());
        if self.target.contains("msvc") {
            options.push(
                if std::env::var("CARGO_CFG_TARGET_FEATURE")
                    .is_ok_and(|v| v.split(',').any(|f| f == "crt-static"))
                {
                    "-Db_vscrt=mt"
                } else {
                    "-Db_vscrt=md"
                }
                .into(),
            );
        }
        match self.platform()? {
            Platform::Mac | Platform::Ios => {
                options.push("-Dgst-plugins-good:osxaudio=enabled".into());
                options.push("-Dgst-plugins-bad:applemedia=enabled".into());
                // applemedia 1.28 requires gst-gl even for system-memory output.
                options.push("-Dgst-plugins-base:gl=enabled".into());
                if self.platform()? == Platform::Mac {
                    options.extend(
                        [
                            "-Dgst-plugins-base:gl_api=opengl",
                            "-Dgst-plugins-base:gl_platform=cgl",
                            "-Dgst-plugins-base:gl_winsys=cocoa",
                        ]
                        .map(String::from),
                    );
                } else {
                    options.extend(
                        [
                            "-Dgst-plugins-base:gl_api=gles2",
                            "-Dgst-plugins-base:gl_platform=eagl",
                            "-Dgst-plugins-base:gl_winsys=eagl",
                        ]
                        .map(String::from),
                    );
                }
            }
            Platform::Windows => {
                options.push("-Dgst-plugins-bad:wasapi=enabled".into());
                options.push("-Dgst-plugins-bad:d3d11=enabled".into());
            }
            Platform::Linux => options.push("-Dgst-plugins-base:alsa=enabled".into()),
            Platform::Android => {
                options.push("-Dgst-plugins-bad:opensles=enabled".into());
                options.push("-Dgst-plugins-bad:androidmedia=enabled".into());
            }
        }
        Ok(options)
    }
    /// Builds from the recorded git revision; dependency wraps carry upstream hashes.
    pub fn build(&self) -> io::Result<PathBuf> {
        if self.jobs == 0 {
            return Err(io::Error::other("native build jobs must be positive"));
        }
        self.platform()?;
        let revision = output(
            Command::new("git")
                .arg("-C")
                .arg(&self.source)
                .args(["rev-parse", "HEAD"]),
        )?;
        if revision.trim() != REVISION {
            return Err(io::Error::other(format!(
                "GStreamer must be {REVISION}, got {}",
                revision.trim()
            )));
        }
        if !output(Command::new("git").arg("-C").arg(&self.source).args([
            "diff",
            "--name-only",
            "HEAD",
        ]))?
        .trim()
        .is_empty()
        {
            return Err(io::Error::other(
                "the pinned GStreamer source has unrecorded tracked changes",
            ));
        }
        fs::create_dir_all(&self.out_dir)?;
        let out = self.out_dir.canonicalize()?;
        if let Ok(manifest) = fs::read_to_string(out.join("build-manifest.txt"))
            && !manifest
                .lines()
                .any(|line| line == format!("target={}", self.target))
        {
            return Err(io::Error::other(
                "use a separate media output directory for each target",
            ));
        }
        let prefix = out.join("install");
        let directory = out.join("build");
        let mut setup = Command::new("meson");
        setup.arg("setup");
        if directory.join("meson-private/coredata.dat").exists() {
            setup.arg("--reconfigure");
        }
        setup
            .arg(&directory)
            .arg(self.source.canonicalize()?)
            .arg("--prefix")
            .arg(&prefix)
            .args(self.options()?);
        if let Some(cross) = &self.cross_file {
            setup.arg("--cross-file").arg(cross.canonicalize()?);
        } else if self.target != self.host {
            let cross = self.generated_cross_file()?;
            let cross_path = out.join("cross.ini");
            fs::write(&cross_path, cross)?;
            setup.arg("--cross-file").arg(cross_path);
        }
        if self.platform()? == Platform::Mac {
            setup.env("MACOSX_DEPLOYMENT_TARGET", super::APPLE_DEPLOYMENT_TARGET);
        }
        // Keep per-project logs, including failures, in the owning output directory.
        logged(&mut setup, &out.join("configure.log"))?;
        logged(
            Command::new("meson")
                .arg("compile")
                .arg("-C")
                .arg(&directory)
                .arg("-j")
                .arg(self.jobs.to_string()),
            &out.join("compile.log"),
        )?;
        logged(
            Command::new("meson")
                .arg("install")
                .arg("-C")
                .arg(&directory)
                .args(["--no-rebuild", "--only-changed"]),
            &out.join("install.log"),
        )?;
        self.write_aliases(&prefix)?;
        let toolchain = output(
            Command::new("meson")
                .arg("introspect")
                .arg("--compilers")
                .arg(&directory),
        )?;
        let manifest = format!(
            "gstreamer={VERSION}\nrevision={REVISION}\ntarget={}\nhost={}\nmeson={}\nninja={}\noptions={}\ntoolchain={}\n",
            self.target,
            self.host,
            output(Command::new("meson").arg("--version"))?.trim(),
            output(Command::new("ninja").arg("--version"))?.trim(),
            self.options()?.join(" "),
            toolchain.trim()
        );
        fs::write(out.join("build-manifest.txt"), manifest)?;
        self.write_notices()?;
        Ok(prefix)
    }
    /// Regenerate the license bundle for an existing native build.
    pub fn write_notices(&self) -> io::Result<()> {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        super::media_notices::generate(
            self,
            &workspace,
            &self.prefix().join("share/licenses/cove-media"),
        )
    }
    /// The pkg-config environment that makes official bindings resolve to this
    /// pinned tree instead of a machine SDK. `cargo cove setup --media` writes
    /// these pairs into the project's Cargo config, so no consumer build script
    /// has to inject them.
    pub fn cargo_environment(&self) -> io::Result<BTreeMap<String, String>> {
        let prefix = self.prefix().canonicalize()?;
        let pc = prefix.join("lib/pkgconfig");
        if !pc.join("gstreamer-full-1.0.pc").is_file() {
            return Err(io::Error::other(
                "no native GStreamer prefix for this target; run cargo cove setup --media",
            ));
        }
        let manifest = fs::read(self.out_dir.join("build-manifest.txt"))?;
        // Cargo tracks pkg-config environment variables, but not Meson's .pc inputs.
        // A new manifest must change that environment to invalidate official sys crates.
        let fingerprint = manifest.iter().fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        });
        let search = self
            .out_dir
            .canonicalize()?
            .join(format!("pc/{fingerprint:016x}"));
        fs::create_dir_all(&search)?;
        for directory in [pc, prefix.join("lib/gstreamer-1.0/pkgconfig")] {
            for entry in fs::read_dir(directory)? {
                let path = entry?.path();
                if path.extension().is_some_and(|ext| ext == "pc") {
                    fs::copy(&path, search.join(path.file_name().expect("pc filename")))?;
                }
            }
        }
        Ok(BTreeMap::from([
            (
                "PKG_CONFIG_LIBDIR".to_string(),
                search.to_string_lossy().into_owned(),
            ),
            ("PKG_CONFIG_PATH".to_string(), String::new()),
            ("PKG_CONFIG_ALLOW_CROSS".to_string(), "1".to_string()),
            ("PKG_CONFIG_ALL_STATIC".to_string(), "1".to_string()),
            ("GST_PLUGIN_SYSTEM_PATH_1_0".to_string(), String::new()),
            ("GST_PLUGIN_PATH_1_0".to_string(), String::new()),
            ("GST_PLUGIN_SYSTEM_PATH".to_string(), String::new()),
            ("GST_PLUGIN_PATH".to_string(), String::new()),
        ]))
    }
    fn write_aliases(&self, prefix: &Path) -> io::Result<()> {
        let pc = prefix.join("lib/pkgconfig");
        fs::create_dir_all(&pc)?;
        // gst-full owns the plugin registration and the selected public libraries.
        for name in [
            "gstreamer-1.0",
            "gstreamer-base-1.0",
            "gstreamer-app-1.0",
            "gstreamer-video-1.0",
            "gstreamer-audio-1.0",
        ] {
            fs::write(
                pc.join(format!("{name}.pc")),
                format!(
                    "Name: {name}\nDescription: Cove pinned static GStreamer\nVersion: {VERSION}\nRequires: gstreamer-full-1.0\n"
                ),
            )?;
        }
        Ok(())
    }
    fn generated_cross_file(&self) -> io::Result<String> {
        let platform = self.platform()?;
        let (family, cpu) = architecture(&self.target)?;
        let mut arguments = Vec::new();
        let (cc, cxx, ar, strip) = match platform {
            Platform::Mac => {
                let root =
                    output(Command::new("xcrun").args(["--sdk", "macosx", "--show-sdk-path"]))?;
                let compiler = |name: &str| {
                    output(Command::new("xcrun").args(["--sdk", "macosx", "--find", name]))
                };
                let arch = if family == "aarch64" {
                    "arm64"
                } else {
                    "x86_64"
                };
                arguments.extend([
                    format!(
                        "--target={arch}-apple-macos{}",
                        super::APPLE_DEPLOYMENT_TARGET
                    ),
                    format!("-isysroot{}", root.trim()),
                ]);
                (
                    compiler("clang")?,
                    compiler("clang++")?,
                    compiler("ar")?,
                    compiler("strip")?,
                )
            }
            Platform::Ios => {
                let simulator = self.target.ends_with("-sim") || family == "x86_64";
                let sdk = if simulator {
                    "iphonesimulator"
                } else {
                    "iphoneos"
                };
                let root = output(Command::new("xcrun").args(["--sdk", sdk, "--show-sdk-path"]))?;
                let compiler =
                    |name: &str| output(Command::new("xcrun").args(["--sdk", sdk, "--find", name]));
                let arch = if family == "aarch64" {
                    "arm64"
                } else {
                    "x86_64"
                };
                arguments.extend([
                    format!(
                        "--target={arch}-apple-ios{}{}",
                        super::APPLE_DEPLOYMENT_TARGET,
                        if simulator { "-simulator" } else { "" }
                    ),
                    "-isysroot".into(),
                    root.trim().into(),
                ]);
                (
                    compiler("clang")?,
                    compiler("clang++")?,
                    compiler("ar")?,
                    compiler("strip")?,
                )
            }
            Platform::Android => {
                let ndk = super::setup::setting("ANDROID_NDK_HOME")?
                    .or(super::setup::setting("ANDROID_NDK_ROOT")?)
                    .ok_or_else(|| {
                        io::Error::other("Android media build requires ANDROID_NDK_HOME")
                    })?;
                let host = if self.host.contains("apple") {
                    "darwin-x86_64"
                } else if self.host.contains("windows") {
                    "windows-x86_64"
                } else {
                    "linux-x86_64"
                };
                let bin = PathBuf::from(ndk)
                    .join("toolchains/llvm/prebuilt")
                    .join(host)
                    .join("bin");
                let triple = if family == "aarch64" {
                    "aarch64-linux-android"
                } else if family == "x86_64" {
                    "x86_64-linux-android"
                } else {
                    return Err(io::Error::other("unsupported Android media architecture"));
                };
                let suffix = if self.host.contains("windows") {
                    ".cmd"
                } else {
                    ""
                };
                let cc = bin.join(format!("{triple}{}{suffix}", super::ANDROID_PLATFORM));
                let cxx = bin.join(format!(
                    "{triple}{}-clang++{suffix}",
                    super::ANDROID_PLATFORM
                ));
                // NDK driver names have the clang suffix after the API number.
                let cc =
                    cc.with_file_name(format!("{triple}{}-clang{suffix}", super::ANDROID_PLATFORM));
                let exe = if self.host.contains("windows") {
                    ".exe"
                } else {
                    ""
                };
                (
                    cc.display().to_string(),
                    cxx.display().to_string(),
                    bin.join(format!("llvm-ar{exe}")).display().to_string(),
                    bin.join(format!("llvm-strip{exe}")).display().to_string(),
                )
            }
            _ => {
                return Err(io::Error::other(
                    "provide --cross-file for this cross toolchain; native builds use the host compiler",
                ));
            }
        };
        let system = match platform {
            Platform::Ios => "darwin",
            Platform::Android => "android",
            Platform::Mac => "darwin",
            Platform::Windows => "windows",
            Platform::Linux => "linux",
        };
        let args = arguments
            .iter()
            .map(|s| meson_string(s))
            .collect::<Vec<_>>()
            .join(", ");
        let subsystem = if platform == Platform::Ios {
            if self.target.ends_with("-sim") || family == "x86_64" {
                "subsystem = 'ios-simulator'\n"
            } else {
                "subsystem = 'ios'\n"
            }
        } else {
            ""
        };
        Ok(format!(
            "[binaries]\nc = {}\ncpp = {}\nobjc = {}\nobjcpp = {}\nar = {}\nstrip = {}\npkg-config = 'pkg-config'\n[host_machine]\nsystem = '{system}'\n{subsystem}cpu_family = '{family}'\ncpu = '{cpu}'\nendian = 'little'\n[built-in options]\nc_args = [{args}]\ncpp_args = [{args}]\nobjc_args = [{args}]\nobjcpp_args = [{args}]\nc_link_args = [{args}]\ncpp_link_args = [{args}]\n[properties]\nneeds_exe_wrapper = true\n",
            meson_string(cc.trim()),
            meson_string(cxx.trim()),
            meson_string(cc.trim()),
            meson_string(cxx.trim()),
            meson_string(ar.trim()),
            meson_string(strip.trim())
        ))
    }
}
pub fn platform(target: &str) -> io::Result<Platform> {
    if target.contains("android") {
        Ok(Platform::Android)
    } else if target.contains("apple-ios") {
        Ok(Platform::Ios)
    } else if target.contains("apple-darwin") {
        Ok(Platform::Mac)
    } else if target.contains("windows") {
        Ok(Platform::Windows)
    } else if target.contains("linux") {
        Ok(Platform::Linux)
    } else {
        Err(io::Error::other(format!(
            "no native GStreamer target for {target}"
        )))
    }
}
fn architecture(target: &str) -> io::Result<(&'static str, &'static str)> {
    if target.starts_with("aarch64") {
        Ok(("aarch64", "armv8"))
    } else if target.starts_with("x86_64") {
        Ok(("x86_64", "x86_64"))
    } else {
        Err(io::Error::other(format!(
            "media cross-file needs an explicit architecture for {target}"
        )))
    }
}
fn meson_string(s: &str) -> String {
    format!("'{}'", s.replace('\\', "/").replace('\'', "\\'"))
}
fn output(command: &mut Command) -> io::Result<String> {
    super::setup::apply(command)?;
    let output = command.output()?;
    if !output.status.success() {
        return Err(io::Error::other(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
fn logged(command: &mut Command, path: &Path) -> io::Result<()> {
    super::setup::apply(command)?;
    let log = fs::File::create(path)?;
    let status = command.stdout(log.try_clone()?).stderr(log).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "native command failed: see {}",
            path.display()
        )))
    }
}
