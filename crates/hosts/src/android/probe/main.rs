//! Translation of Android/CoveAndroidProbe/main.swift.
//!
//! Swift's `main.swift` was the probe's process entry: it parsed the artifacts
//! and output directories, asserted the Android backend defaults, ran every
//! check and wrote `report.json`. A Rust library crate cannot own an entry
//! point, so the same sequence is exposed as [`main`] for the probe binary the
//! Android build produces.

use super::runner::{ProbeContext, ProbeFailure, ProbeReport, ProbeResult, require};
use cove_graphics::rhi::{
    Device, DeviceConfig, GraphicsApi, NativeSurfaceKind, SurfaceDescriptor, TextureFormat,
};
use cove_logging::{Level, LogEvent, LogSystem, LogValue, LoggingConfiguration};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

/// The Swift entry point's body, returning the process exit code.
pub fn main(arguments: &[String]) -> i32 {
    let _ = LogSystem::bootstrap(
        Some("com.cove.android.probe"),
        LoggingConfiguration::default(),
        None,
    );
    println!("Cove Android native Rust probe");
    let code = match run(arguments) {
        Ok(code) => code,
        Err(error) => {
            println!("FAIL initialization: {error}");
            let mut metadata = BTreeMap::new();
            metadata.insert("error".to_owned(), LogValue::String(error.0.clone()));
            LogSystem::logger("com.cove.android.probe")
                .log(&LogEvent::new(Level::Error, "Initialization failed").with_metadata(metadata));
            1
        }
    };
    LogSystem::flush();
    code
}

fn run(arguments: &[String]) -> Result<i32, ProbeFailure> {
    let (Some(artifacts), Some(output)) = (arguments.get(1), arguments.get(2)) else {
        return Err(ProbeFailure::new(
            "usage: CoveAndroidProbe ARTIFACTS OUTPUT",
        ));
    };
    let output = Path::new(output);
    std::fs::create_dir_all(output)
        .map_err(|error| ProbeFailure::new(format!("{}: {error}", output.display())))?;

    require(
        cove_graphics::rhi::platform_default_backends() == vec![GraphicsApi::Vulkan],
        "Android must select Vulkan by default",
    )?;
    let surface = SurfaceDescriptor::new(0);
    require(
        surface.kind == NativeSurfaceKind::AndroidWindow
            && surface.color_format == TextureFormat::Rgba8Unorm,
        "Android surface defaults are incorrect",
    )?;

    let device = Arc::new(
        Device::make(DeviceConfig {
            enable_validation: false,
            ..DeviceConfig::default()
        })
        .map_err(|error| ProbeFailure::new(error.to_string()))?,
    );
    println!("GPU: {}", device.device_name());
    let context = ProbeContext {
        device,
        artifacts: Path::new(artifacts).to_path_buf(),
        output: output.to_path_buf(),
    };

    let checks: [(&str, fn(&ProbeContext) -> Result<String, ProbeFailure>); 7] = [
        ("layout", |context| context.layout()),
        ("transfer", |context| context.transfer()),
        ("compute", |context| context.compute()),
        ("raster-1x", |context| context.raster(1)),
        ("raster-4x", |context| context.raster(4)),
        ("coveui-1x", |context| context.ui(1)),
        ("coveui-4x", |context| context.ui(4)),
    ];
    let mut results: Vec<ProbeResult> = Vec::with_capacity(checks.len());
    for (name, check) in checks {
        match check(&context) {
            Ok(detail) => {
                println!("PASS {name}: {detail}");
                results.push(ProbeResult {
                    name: name.to_owned(),
                    passed: true,
                    detail,
                });
            }
            Err(error) => {
                println!("FAIL {name}: {error}");
                let mut metadata = BTreeMap::new();
                metadata.insert("error".to_owned(), LogValue::String(error.0.clone()));
                LogSystem::logger("com.cove.android.probe")
                    .log(&LogEvent::new(Level::Error, name).with_metadata(metadata));
                results.push(ProbeResult {
                    name: name.to_owned(),
                    passed: false,
                    detail: error.0,
                });
            }
        }
    }

    let report = ProbeReport {
        device: context.device.device_name(),
        backend: format!("{:?}", context.device.backend_api()),
        results,
    };
    let encoded =
        serde_json::to_vec_pretty(&report).map_err(|error| ProbeFailure::new(error.to_string()))?;
    std::fs::write(context.output_directory().join("report.json"), encoded)
        .map_err(|error| ProbeFailure::new(error.to_string()))?;
    Ok(if report.results.iter().any(|result| !result.passed) {
        1
    } else {
        0
    })
}
