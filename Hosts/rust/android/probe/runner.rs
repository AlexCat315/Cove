//! Translation of Android/CoveAndroidProbe/Runner.swift.

use cove_graphics::rhi::{Device, QueueClass, ShaderArtifact, Texture, TextureSubresource};
use std::sync::Arc;
use std::path::{Path, PathBuf};

/// Probe failure with the Swift `CustomStringConvertible` description.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProbeFailure(pub String);

impl std::fmt::Display for ProbeFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ProbeFailure {}

impl ProbeFailure {
    pub fn new(description: impl Into<String>) -> Self {
        Self(description.into())
    }
}

pub fn require(condition: bool, message: &str) -> Result<(), ProbeFailure> {
    if condition { Ok(()) } else { Err(ProbeFailure::new(message)) }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProbeResult {
    pub name: String,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProbeReport {
    pub device: String,
    pub backend: String,
    pub results: Vec<ProbeResult>,
}

/// Shared probe state: the device, the shader artifact directory and the
/// artifact output directory.
pub struct ProbeContext {
    pub device: Arc<Device>,
    pub artifacts: PathBuf,
    pub output: PathBuf,
}

impl ProbeContext {
    /// Decodes one offline-compiled shader artifact from `artifacts/<name>.json`.
    pub fn shader(&self, name: &str) -> Result<ShaderArtifact, ProbeFailure> {
        let path = self.artifacts.join(format!("{name}.json"));
        let bytes = std::fs::read(&path)
            .map_err(|error| ProbeFailure::new(format!("{}: {error}", path.display())))?;
        serde_json::from_slice(&bytes)
            .map_err(|error| ProbeFailure::new(format!("{}: {error}", path.display())))
    }

    /// Runs one command buffer inside a frame and waits for the GPU to retire
    /// it, so the caller can read the target back immediately.
    pub fn submit(
        &self,
        commands: &cove_graphics::rhi::CommandBuffer,
        queue: QueueClass,
    ) -> Result<(), ProbeFailure> {
        let fail = |error: cove_graphics::rhi::RhiError| ProbeFailure::new(error.to_string());
        self.device.begin_frame().map_err(fail)?;
        if let Err(error) = self.device.submit(commands, queue, &[], &[], None) {
            self.device.end_frame();
            return Err(fail(error));
        }
        self.device.end_frame();
        self.device.wait_until_idle().map_err(fail)
    }

    pub fn read(
        &self,
        texture: Texture,
        width: usize,
        height: usize,
    ) -> Result<Vec<u8>, ProbeFailure> {
        let mut bytes = vec![0_u8; width * height * 4];
        self.device
            .read_texture_data(
                texture,
                width,
                height,
                width * 4,
                TextureSubresource::default(),
                &mut bytes,
            )
            .map_err(|error| ProbeFailure::new(error.to_string()))?;
        Ok(bytes)
    }

    /// Writes a binary PPM (P6) alongside the JSON report.
    pub fn write_image(
        &self,
        bytes: &[u8],
        width: usize,
        height: usize,
        name: &str,
    ) -> Result<(), ProbeFailure> {
        let mut data = format!("P6\n{width} {height}\n255\n").into_bytes();
        for pixel in bytes.chunks_exact(4) {
            data.extend_from_slice(&pixel[..3]);
        }
        let path = self.output.join(format!("{name}.ppm"));
        std::fs::write(&path, data)
            .map_err(|error| ProbeFailure::new(format!("{}: {error}", path.display())))
    }

    pub fn artifact_directory(&self) -> &Path {
        &self.artifacts
    }

    pub fn output_directory(&self) -> &Path {
        &self.output
    }
}
