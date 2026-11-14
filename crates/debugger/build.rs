//! Builds the native LLDB bridge as a runtime-loaded shared library.
//!
//! The controller never links liblldb: the shim is compiled to a cdylib, embedded
//! in the crate, and loaded on demand. When liblldb is absent at build time a
//! stub shim is compiled instead, so the backend reports `unavailable` truthfully
//! and the DAP adapter stays the fallback.
//!
//! The LLDB SB API headers under `native/include` are unmodified upstream
//! copies; `native/include/README.md` records where they come from and how to
//! refresh them. The SB API is ABI-stable across LLDB releases, so the shim
//! builds against these headers and links the liblldb found on the host. Only
//! the link/runtime library is discovered.

use std::env;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=native/cove_lldb.h");
    println!("cargo:rerun-if-changed=native/shim.cpp");
    println!("cargo:rerun-if-changed=native/stub.c");
    println!("cargo:rerun-if-changed=native/include");
    println!("cargo:rerun-if-env-changed=COVE_LLDB_DIR");
    println!("cargo:rerun-if-env-changed=COVE_LIBLLDB");

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target = env::var("TARGET").unwrap_or_default();
    let desktop = matches!(target_os.as_str(), "windows" | "macos" | "linux");

    let discovery = if desktop { discover(&target_os) } else { None };
    let shim = out_dir.join(shim_file_name(&target_os));

    match &discovery {
        Some(lldb) => build_shim(&out_dir, &shim, &target, lldb),
        None => build_stub(&out_dir, &shim, &target),
    }

    // The runtime library the shim needs. Empty when unknown; the loader then
    // searches the platform defaults and COVE_LIBLLDB at run time.
    let runtime = discovery
        .as_ref()
        .map(|lldb| lldb.runtime.to_string_lossy().into_owned())
        .unwrap_or_default();
    println!("cargo:rustc-env=COVE_LLDB_SHIM={}", shim.display());
    println!("cargo:rustc-env=COVE_LLDB_LIB={runtime}");
}

fn shim_file_name(target_os: &str) -> &'static str {
    match target_os {
        "windows" => "cove_lldb_shim.dll",
        "macos" => "libcove_lldb_shim.dylib",
        _ => "libcove_lldb_shim.so",
    }
}

/// Where the liblldb link library and runtime library live.
struct Lldb {
    libdir: PathBuf,
    runtime: PathBuf,
    /// macOS LLDB.framework layout (link with `-framework LLDB`).
    framework: Option<PathBuf>,
}

fn discover(target_os: &str) -> Option<Lldb> {
    if let Some(root) = env::var_os("COVE_LLDB_DIR") {
        let root = PathBuf::from(root);
        if let Some(lldb) = from_root(&root, target_os) {
            return Some(lldb);
        }
        println!(
            "cargo:warning=COVE_LLDB_DIR={} has no usable liblldb",
            root.display()
        );
        return None;
    }
    match target_os {
        "windows" => windows_roots()
            .iter()
            .find_map(|root| from_root(root, target_os)),
        "linux" => linux_roots()
            .iter()
            .find_map(|root| from_root(root, target_os)),
        "macos" => macos_lldb(),
        _ => None,
    }
}

/// A standard LLVM install root with a linkable liblldb.
fn from_root(root: &Path, target_os: &str) -> Option<Lldb> {
    let libdir = root.join("lib");
    let (link, runtime) = match target_os {
        "windows" => (libdir.join("liblldb.lib"), root.join("bin/liblldb.dll")),
        "macos" => (libdir.join("liblldb.dylib"), libdir.join("liblldb.dylib")),
        _ => {
            let runtime = versioned_so(&libdir)?;
            (libdir.join("liblldb.so"), runtime)
        }
    };
    if !link.is_file() {
        return None;
    }
    Some(Lldb {
        libdir,
        runtime,
        framework: None,
    })
}

fn versioned_so(libdir: &Path) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = std::fs::read_dir(libdir)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("liblldb.so"))
        })
        .collect();
    candidates.sort();
    candidates.into_iter().next_back()
}

fn windows_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for variable in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(root) = env::var_os(variable) {
            roots.push(PathBuf::from(root).join("LLVM"));
        }
    }
    roots
}

fn linux_roots() -> Vec<PathBuf> {
    let mut roots = vec![PathBuf::from("/usr/local"), PathBuf::from("/usr")];
    for base in ["/usr/lib", "/usr/lib64"] {
        if let Ok(entries) = std::fs::read_dir(base) {
            let mut llvm: Vec<PathBuf> = entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| {
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.starts_with("llvm-"))
                })
                .collect();
            llvm.sort();
            roots.extend(llvm.into_iter().rev());
        }
    }
    roots
}

fn macos_lldb() -> Option<Lldb> {
    for root in ["/opt/homebrew/opt/llvm", "/usr/local/opt/llvm"] {
        if let Some(lldb) = from_root(Path::new(root), "macos") {
            return Some(lldb);
        }
    }
    for framework in macos_frameworks() {
        let binary = framework.join("LLDB");
        if binary.is_file() {
            let parent = framework.parent()?.to_path_buf();
            return Some(Lldb {
                libdir: parent.clone(),
                runtime: binary,
                framework: Some(parent),
            });
        }
    }
    None
}

fn macos_frameworks() -> Vec<PathBuf> {
    let mut paths = vec![
        PathBuf::from(
            "/Library/Developer/CommandLineTools/Library/PrivateFrameworks/LLDB.framework",
        ),
        PathBuf::from("/Applications/Xcode.app/Contents/SharedFrameworks/LLDB.framework"),
    ];
    if let Ok(entries) = std::fs::read_dir("/Applications") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("Xcode") && name.ends_with(".app") {
                paths.push(
                    entry
                        .path()
                        .join("Contents/SharedFrameworks/LLDB.framework"),
                );
            }
        }
    }
    paths
}

fn cc_build(cpp: bool) -> cc::Build {
    let mut build = cc::Build::new();
    build.cpp(cpp).opt_level(2);
    if env::var("TARGET").unwrap_or_default().contains("msvc") {
        build.flag("/EHsc");
        // The vendored SB headers hold std::unique_ptr/shared_ptr members whose
        // pointees are incomplete; C4251 is inherent to the SB API, not actionable.
        build.flag("/wd4251");
    } else {
        // `-std=c++17` is invalid for C sources, and the stub is compiled as C.
        if cpp {
            build.flag("-std=c++17");
        }
        build.flag("-fPIC");
    }
    build
}

fn build_shim(out_dir: &Path, shim: &Path, target: &str, lldb: &Lldb) {
    let mut build = cc_build(true);
    build.file("native/shim.cpp").include("native/include");
    let objects = build.compile_intermediates();
    link(out_dir, shim, target, &build, &objects, Some(lldb));
}

fn build_stub(out_dir: &Path, shim: &Path, target: &str) {
    let mut build = cc_build(false);
    build.file("native/stub.c");
    let objects = build.compile_intermediates();
    link(out_dir, shim, target, &build, &objects, None);
}

/// Links the objects into a cdylib by hand so the crate records no dependency on
/// the shim or liblldb; both are resolved only when the backend is loaded.
fn link(
    out_dir: &Path,
    shim: &Path,
    target: &str,
    build: &cc::Build,
    objects: &[PathBuf],
    lldb: Option<&Lldb>,
) {
    let tool = build.get_compiler();
    let mut command = tool.to_command();
    command.current_dir(out_dir);
    if target.contains("msvc") {
        command.arg("/LD");
        for object in objects {
            command.arg(object);
        }
        command.arg("/link");
        if let Some(lldb) = lldb {
            command.arg(format!("/LIBPATH:{}", lldb.libdir.display()));
            command.arg("liblldb.lib");
        }
        command.arg(format!("/OUT:{}", shim.display()));
        command.arg(format!(
            "/IMPLIB:{}",
            out_dir.join("cove_lldb_shim.lib").display()
        ));
    } else {
        command.arg("-shared").arg("-o").arg(shim);
        for object in objects {
            command.arg(object);
        }
        if let Some(lldb) = lldb {
            if let Some(framework) = &lldb.framework {
                command
                    .arg("-F")
                    .arg(framework)
                    .arg("-framework")
                    .arg("LLDB")
                    .arg(format!("-Wl,-rpath,{}", framework.display()));
            } else {
                command
                    .arg(format!("-L{}", lldb.libdir.display()))
                    .arg("-llldb")
                    .arg(format!("-Wl,-rpath,{}", lldb.libdir.display()));
            }
        }
    }
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("link {}: {error}", shim.display()));
    if !output.status.success() || !shim.is_file() {
        panic!(
            "linking the LLDB shim failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
