//! Build the pinned native prefix, optionally run Cargo against that prefix.
use cove_native_build::gstreamer::Build;
use std::{io, path::PathBuf, process::Command};

fn main() -> io::Result<()> {
    let cove = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let host = Command::new("rustc").arg("-vV").output()?;
    if !host.status.success() {
        return Err(io::Error::other("rustc -vV failed"));
    }
    let host = String::from_utf8_lossy(&host.stdout)
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .ok_or_else(|| io::Error::other("rustc host is missing"))?
        .to_owned();
    let mut target = host.clone();
    let mut out = None;
    let mut cross_file = None;
    let mut jobs = 4;
    let mut cargo = Vec::new();
    let mut notices_only = false;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == "--" {
            cargo.extend(arguments);
            break;
        }
        if argument == "--notices-only" {
            notices_only = true;
            continue;
        }
        let value = arguments
            .next()
            .ok_or_else(|| io::Error::other(format!("missing value for {argument}")))?;
        match argument.as_str() {
            "--target" => target = value,
            "--out-dir" => out = Some(PathBuf::from(value)),
            "--cross-file" => cross_file = Some(PathBuf::from(value)),
            "--jobs" => {
                jobs = value
                    .parse()
                    .map_err(|_| io::Error::other("invalid --jobs"))?
            }
            _ => {
                return Err(io::Error::other(format!(
                    "unknown native build option {argument}"
                )));
            }
        }
    }
    let out_dir = out.unwrap_or_else(|| cove.join("target/media-native").join(&target).join("out"));
    let build = Build {
        source: cove.join("third-party/gstreamer"),
        out_dir,
        target: target.clone(),
        host: host.clone(),
        cross_file,
        jobs,
    };
    if notices_only {
        if !cargo.is_empty() {
            return Err(io::Error::other(
                "--notices-only cannot run Cargo against an unrebuilt prefix",
            ));
        }
        build.write_notices()?;
    } else {
        build.build()?;
    }
    if !cargo.is_empty() {
        if target != host {
            let position = cargo
                .iter()
                .position(|arg| arg == "--")
                .unwrap_or(cargo.len());
            cargo.splice(position..position, ["--target".into(), target]);
        }
        let mut command = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
        command.args(cargo);
        build.configure_cargo(&mut command)?;
        if !command.status()?.success() {
            return Err(io::Error::other(
                "Cargo failed with the source-built media prefix",
            ));
        }
    }
    Ok(())
}
