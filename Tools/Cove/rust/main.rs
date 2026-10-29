use clap::{Args, Parser, Subcommand};
use serde::Serialize;
use std::{
    env,
    ffi::OsString,
    fs,
    io::{self, BufRead},
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

#[derive(Parser)]
#[command(name = "cove", version, about = "Cove Rust build and migration tools")]
struct Cli {
    #[command(subcommand)]
    command: Operation,
}

#[derive(Subcommand)]
enum Operation {
    /// Build Rust crates. Native UI migration is still in progress.
    Build(BuildOptions),
    /// Run a Rust binary.
    Run(RunOptions),
    /// Run Rust tests.
    Test(TestOptions),
    /// Type-check Rust crates.
    Check(BuildOptions),
    /// Check formatting.
    Fmt {
        #[arg(default_value = ".")]
        directory: PathBuf,
        #[arg(long)]
        fix: bool,
    },
    /// Run Rust lints, denying warnings.
    Clippy(BuildOptions),
    /// Remove Cargo-generated output.
    Clean {
        #[arg(default_value = ".")]
        directory: PathBuf,
    },
    /// Inspect the Rust and C toolchain.
    Doctor,
    /// Install Rust formatter/lints and optionally a compilation target.
    Setup {
        #[arg(long)]
        target: Option<String>,
    },
    /// Inspect the remaining Swift migration work.
    Migration {
        #[command(subcommand)]
        command: MigrationCommand,
    },
}

#[derive(Args)]
struct BuildOptions {
    #[arg(default_value = ".")]
    directory: PathBuf,
    #[arg(long)]
    release: bool,
    #[arg(long)]
    target: Option<String>,
    #[arg(long)]
    package: Option<String>,
    #[arg(long)]
    all_targets: bool,
    #[arg(long)]
    locked: bool,
    #[arg(long)]
    offline: bool,
}
#[derive(Args)]
struct RunOptions {
    #[command(flatten)]
    build: BuildOptions,
    #[arg(long)]
    bin: Option<String>,
    #[arg(last = true)]
    arguments: Vec<OsString>,
}
#[derive(Args)]
struct TestOptions {
    #[command(flatten)]
    build: BuildOptions,
    #[arg(long)]
    filter: Option<String>,
    #[arg(long)]
    build_only: bool,
    #[arg(long)]
    no_parallel: bool,
    #[arg(last = true)]
    arguments: Vec<OsString>,
}
#[derive(Subcommand)]
enum MigrationCommand {
    Status {
        #[arg(long)]
        json: bool,
        #[arg(long)]
        require_complete: bool,
        #[arg(long)]
        root: Option<PathBuf>,
    },
}

fn sdk_root() -> PathBuf {
    env::var_os("COVE_SDK_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .ancestors()
                .nth(2)
                .unwrap()
                .to_owned()
        })
}

fn cargo() -> Command {
    Command::new(
        env::var_os("COVE_CARGO_EXECUTABLE")
            .or_else(|| env::var_os("CARGO"))
            .unwrap_or_else(|| "cargo".into()),
    )
}

fn manifest(directory: &Path) -> io::Result<PathBuf> {
    let directory = directory.canonicalize()?;
    let path = directory.join("Cargo.toml");
    if !path.is_file() {
        let message = if directory.join("Package.swift").is_file() {
            format!(
                "{} has not been migrated to Rust; see Docs/rust-migration.md",
                directory.display()
            )
        } else {
            format!("{} does not contain Cargo.toml", directory.display())
        };
        return Err(io::Error::new(io::ErrorKind::NotFound, message));
    }
    Ok(path)
}
fn cargo_command(operation: &str, options: &BuildOptions) -> io::Result<Command> {
    let manifest = manifest(&options.directory)?;
    let mut command = cargo();
    command.arg(operation).arg("--manifest-path").arg(&manifest);
    if operation != "run"
        && manifest == sdk_root().canonicalize()?.join("Cargo.toml")
        && options.package.is_none()
    {
        command.arg("--workspace");
    }
    if options.release {
        command.arg("--release");
    }
    if let Some(target) = &options.target {
        command.arg("--target").arg(target);
    }
    if let Some(package) = &options.package {
        command.arg("--package").arg(package);
    }
    if options.all_targets {
        command.arg("--all-targets");
    }
    if options.locked {
        command.arg("--locked");
    }
    if options.offline {
        command.arg("--offline");
    }
    Ok(command)
}
fn execute(command: &mut Command) -> io::Result<u8> {
    let status = command.status()?;
    Ok(status
        .code()
        .and_then(|s| u8::try_from(s).ok())
        .unwrap_or(1))
}

#[derive(Serialize)]
struct MigrationStatus {
    complete: bool,
    swift_files: usize,
    swift_lines: usize,
    rust_files: usize,
    modules: Vec<ModuleStatus>,
}
#[derive(Serialize)]
struct ModuleStatus {
    name: String,
    swift_files: usize,
    rust_files: usize,
}
fn inventory(root: &Path) -> io::Result<MigrationStatus> {
    let mut modules = std::collections::BTreeMap::<String, ModuleStatus>::new();
    let mut status = MigrationStatus {
        complete: false,
        swift_files: 0,
        swift_lines: 0,
        rust_files: 0,
        modules: vec![],
    };
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            let path = entry.path();
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                if [
                    ".git",
                    ".build",
                    ".swiftpm",
                    "target",
                    "third-party",
                    "vendor",
                    "node_modules",
                    "build",
                    "dist",
                    ".cache",
                    ".compare",
                    ".workbuddy-ai",
                ]
                .iter()
                .any(|name| entry.file_name() == *name)
                {
                    continue;
                }
                pending.push(path);
                continue;
            }
            let swift = path.extension().is_some_and(|e| e == "swift")
                || path.file_name().is_some_and(|n| n == "Package.swift.in");
            let rust = path.extension().is_some_and(|e| e == "rs");
            if !swift && !rust {
                continue;
            }
            let relative = path.strip_prefix(root).unwrap();
            let name = relative
                .components()
                .next()
                .unwrap()
                .as_os_str()
                .to_string_lossy()
                .into_owned();
            let module = modules.entry(name.clone()).or_insert(ModuleStatus {
                name,
                swift_files: 0,
                rust_files: 0,
            });
            if swift {
                module.swift_files += 1;
                status.swift_files += 1;
                for line in io::BufReader::new(fs::File::open(&path)?).lines() {
                    line?;
                    status.swift_lines += 1;
                }
            }
            if rust {
                module.rust_files += 1;
                status.rust_files += 1;
            }
        }
    }
    status.modules = modules.into_values().collect();
    status.complete = status.swift_files == 0;
    Ok(status)
}
fn run(cli: Cli) -> io::Result<u8> {
    match cli.command {
        Operation::Build(options) => execute(&mut cargo_command("build", &options)?),
        Operation::Check(options) => execute(&mut cargo_command("check", &options)?),
        Operation::Run(options) => {
            let mut command = cargo_command("run", &options.build)?;
            if let Some(bin) = options.bin {
                command.arg("--bin").arg(bin);
            }
            command.arg("--").args(options.arguments);
            execute(&mut command)
        }
        Operation::Test(options) => {
            let mut command = cargo_command("test", &options.build)?;
            if options.build_only {
                command.arg("--no-run");
            }
            if let Some(filter) = options.filter {
                command.arg(filter);
            }
            if options.no_parallel || !options.arguments.is_empty() {
                command.arg("--");
            }
            if options.no_parallel {
                command.arg("--test-threads=1");
            }
            command.args(options.arguments);
            execute(&mut command)
        }
        Operation::Clippy(options) => {
            let mut command = cargo_command("clippy", &options)?;
            command.args(["--", "-D", "warnings"]);
            execute(&mut command)
        }
        Operation::Fmt { directory, fix } => {
            let mut command = cargo();
            command
                .arg("fmt")
                .arg("--manifest-path")
                .arg(manifest(&directory)?)
                .arg("--all");
            if !fix {
                command.args(["--", "--check"]);
            }
            execute(&mut command)
        }
        Operation::Clean { directory } => {
            let mut command = cargo();
            command
                .arg("clean")
                .arg("--manifest-path")
                .arg(manifest(&directory)?);
            execute(&mut command)
        }
        Operation::Doctor => {
            let mut failed = false;
            for (tool, arguments) in [
                ("rustc", vec!["--version"]),
                ("cargo", vec!["--version"]),
                (
                    if cfg!(windows) { "cl" } else { "cc" },
                    if cfg!(windows) {
                        vec!["/?"]
                    } else {
                        vec!["--version"]
                    },
                ),
            ] {
                let mut command = if tool == "cargo" {
                    cargo()
                } else {
                    Command::new(tool)
                };
                match command.args(arguments).output() {
                    Ok(output) if output.status.success() => {
                        let text = if output.stdout.is_empty() {
                            &output.stderr
                        } else {
                            &output.stdout
                        };
                        println!(
                            "{}",
                            String::from_utf8_lossy(text).lines().next().unwrap_or(tool)
                        );
                    }
                    _ => {
                        eprintln!("missing tool: {tool}");
                        failed = true;
                    }
                }
            }
            Ok(u8::from(failed))
        }
        Operation::Setup { target } => {
            let code =
                execute(Command::new("rustup").args(["component", "add", "rustfmt", "clippy"]))?;
            if code != 0 {
                return Ok(code);
            }
            if let Some(target) = target {
                return execute(Command::new("rustup").args(["target", "add", &target]));
            }
            Ok(0)
        }
        Operation::Migration {
            command:
                MigrationCommand::Status {
                    json,
                    require_complete,
                    root,
                },
        } => {
            let status = inventory(&root.unwrap_or_else(sdk_root).canonicalize()?)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&status)?);
            } else {
                println!(
                    "Swift: {} files / {} lines; Rust: {} files",
                    status.swift_files, status.swift_lines, status.rust_files
                );
                println!("{:<18} {:>8} {:>8}", "Module", "Swift", "Rust");
                for module in &status.modules {
                    println!(
                        "{:<18} {:>8} {:>8}",
                        module.name, module.swift_files, module.rust_files
                    );
                }
                println!(
                    "{}",
                    if status.complete {
                        "Swift source removal complete; platform acceptance is tracked separately."
                    } else {
                        "Migration incomplete. See Docs/rust-migration.md."
                    }
                );
            }
            Ok(if require_complete && !status.complete {
                2
            } else {
                0
            })
        }
    }
}
fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("cove: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_without_passing_test_flags_to_cargo_build() {
        let cli = Cli::try_parse_from([
            "cove",
            "test",
            "System",
            "--no-parallel",
            "--filter",
            "timeout",
            "--",
            "--nocapture",
        ])
        .unwrap();
        let Operation::Test(options) = cli.command else {
            panic!()
        };
        assert!(options.no_parallel);
        assert_eq!(options.filter.as_deref(), Some("timeout"));
        assert_eq!(options.arguments, [OsString::from("--nocapture")]);
        assert!(Cli::try_parse_from(["cove", "build", "--no-parallel"]).is_err());
    }
    #[test]
    fn audit_excludes_dependencies_and_counts_swift_templates() {
        let root = tempfile::tempdir().unwrap();
        for path in [
            "Core/a.swift",
            "Core/rust/lib.rs",
            "Tools/Package.swift.in",
            "third-party/b.swift",
            "target/c.swift",
        ] {
            let path = root.path().join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "first\nsecond\n").unwrap();
        }
        let status = inventory(root.path()).unwrap();
        assert_eq!(
            (
                status.swift_files,
                status.swift_lines,
                status.rust_files,
                status.complete
            ),
            (2, 4, 1, false)
        );
    }
    #[test]
    fn swift_only_package_is_rejected_without_compiler_fallback() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("Package.swift"), "").unwrap();
        assert!(
            manifest(root.path())
                .unwrap_err()
                .to_string()
                .contains("has not been migrated")
        );
    }
}
