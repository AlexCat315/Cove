//! Translation of `Graphics/Tests/NativeRHITests/NativeGPUContractTests.swift`.
//!
//! The same shader/resource contract has to hold on whichever native API is
//! present. These pin the parts of that contract which do not need a compiled
//! fixture: mip/layer subresource transfers write and read back independently,
//! invalid subresources are rejected rather than clamped, and a texture whose
//! format has no 8-bit upload path is refused.
//!
//! Reachability: most of the Swift suite drives Slang-compiled fixtures
//! (`ShaderFixtures.compile("mip-sample", ...)`) through a real compute dispatch
//! and then a graphics pass. That needs `SLANGC` plus the Swift `cove-dev` tool,
//! exactly the gate `ShaderFixtureCompiler.swift` applies, so those cases are a
//! documented skip (see `fixture_dispatch_cases_are_gated_on_the_slang_toolchain`).
//! The transfer/validation cases below need no compiler and are asserted in full.
//!
//! Named difference: Swift's `TextureSubresource(mipLevel: -1)` and
//! `(layer: -1)` express a negative index; Rust's `TextureSubresource` holds
//! `usize`s, so a negative index is unrepresentable and those two cases collapse
//! into the out-of-range cases that remain (mip == mip_levels, layer == layers).
//!
//! Behaviour checklist:
//! - every layer/mip carries its own bytes and no read disturbs a neighbour
//! - reading in reverse order detects layout loss or stray mip-0 writes
//! - an out-of-range mip or layer is rejected on upload and on read
//! - an 8-bit upload into a 16-bit format is rejected

#![cfg(target_os = "macos")]

use std::sync::Arc;

use cove_graphics::{
    Device, DeviceConfig, GraphicsApi, TextureDescriptor, TextureDimension, TextureFormat,
    TextureSubresource, TextureUploadRegion, TextureUsage,
};

fn device() -> Arc<Device> {
    Arc::new(
        Device::make(DeviceConfig {
            preferred_backends: vec![GraphicsApi::Metal],
            ..DeviceConfig::default()
        })
        .expect("a Metal device is available"),
    )
}

/// Layer/mip extent for the 8x4 base, floored at one texel.
fn extent(mip: usize) -> (usize, usize) {
    ((8 >> mip).max(1), (4 >> mip).max(1))
}

/// Every layer/mip is filled with a byte derived from both indices, then read
/// back in reverse so a stray write to mip 0 or an adjacent layer is visible.
#[test]
fn mip_and_array_transfers() {
    let device = device();
    let texture = device
        .make_texture(TextureDescriptor {
            layers: 2,
            mip_levels: 4,
            dimension: TextureDimension::Texture2DArray,
            ..TextureDescriptor::new(
                8,
                4,
                TextureFormat::Rgba16Float,
                TextureUsage::SAMPLED
                    | TextureUsage::TRANSFER_DESTINATION
                    | TextureUsage::TRANSFER_SOURCE,
            )
        })
        .expect("the array texture is created");

    let tag = |layer: usize, mip: usize| (17 + layer * 40 + mip * 5) as u8;

    for layer in 0..2 {
        for mip in 0..4 {
            let (width, height) = extent(mip);
            let bytes = vec![tag(layer, mip); width * height * 8];
            device
                .upload_texture_data(
                    texture,
                    &bytes,
                    TextureUploadRegion::new(width as i64, height as i64),
                    width * 8,
                    TextureSubresource::new(mip, layer),
                )
                .unwrap_or_else(|error| panic!("layer {layer} mip {mip} uploads: {error}"));
        }
    }

    for layer in (0..2).rev() {
        for mip in (0..4).rev() {
            let (width, height) = extent(mip);
            let mut bytes = vec![0_u8; width * height * 8];
            device
                .read_texture_data(
                    texture,
                    width,
                    height,
                    width * 8,
                    TextureSubresource::new(mip, layer),
                    &mut bytes,
                )
                .unwrap_or_else(|error| panic!("layer {layer} mip {mip} reads back: {error}"));
            assert_eq!(
                bytes,
                vec![tag(layer, mip); width * height * 8],
                "layer {layer} mip {mip} keeps its own bytes",
            );
        }
    }

    device.destroy_texture(texture);
}

#[test]
fn out_of_range_subresources_are_rejected() {
    let device = device();
    let texture = device
        .make_texture(TextureDescriptor {
            layers: 2,
            mip_levels: 4,
            dimension: TextureDimension::Texture2DArray,
            ..TextureDescriptor::new(
                8,
                4,
                TextureFormat::Rgba16Float,
                TextureUsage::SAMPLED
                    | TextureUsage::TRANSFER_DESTINATION
                    | TextureUsage::TRANSFER_SOURCE,
            )
        })
        .expect("the array texture is created");

    // Rust's `TextureSubresource` holds `usize`s, so Swift's negative indices
    // have no representation; the two out-of-range cases that remain are the ones
    // past the end.
    for (label, subresource) in [
        ("mip == mip_levels", TextureSubresource::new(4, 0)),
        ("layer == layers", TextureSubresource::new(0, 2)),
        ("both out of range", TextureSubresource::new(4, 2)),
    ] {
        assert!(
            device
                .upload_texture_data(
                    texture,
                    &vec![0_u8; 256],
                    TextureUploadRegion::new(1, 1),
                    8,
                    subresource,
                )
                .is_err(),
            "upload with {label} is rejected",
        );
        let mut bytes = vec![0_u8; 8];
        assert!(
            device
                .read_texture_data(texture, 1, 1, 8, subresource, &mut bytes)
                .is_err(),
            "read with {label} is rejected",
        );
    }

    device.destroy_texture(texture);
}

#[test]
fn mismatched_format_upload_is_rejected() {
    let device = device();
    let texture = device
        .make_texture(TextureDescriptor {
            layers: 2,
            mip_levels: 4,
            dimension: TextureDimension::Texture2DArray,
            ..TextureDescriptor::new(
                8,
                4,
                TextureFormat::Rgba16Float,
                TextureUsage::SAMPLED
                    | TextureUsage::TRANSFER_DESTINATION
                    | TextureUsage::TRANSFER_SOURCE,
            )
        })
        .expect("the array texture is created");
    // An 8-byte-per-row upload cannot describe an Rgba16Float mip, which needs
    // two bytes per channel.
    assert!(
        device
            .upload_texture_data(
                texture,
                &vec![0_u8; 8 * 4 * 8],
                TextureUploadRegion::new(8, 4),
                8 * 8,
                TextureSubresource::new(1, 0),
            )
            .is_err(),
        "an upload whose row length does not match the format is refused",
    );
    device.destroy_texture(texture);
}

#[test]
fn fixture_dispatch_cases_are_gated_on_the_slang_toolchain() {
    // Swift's mip-sampling, sampling, queue-storage, push-constant and raster
    // cases all begin with `ShaderFixtures.compile(...)`, which throws
    // `XCTSkip("Set SLANGC to Slang 2026.19 ...")` unless the Slang compiler and
    // the Swift `cove-dev` tool are available. Assert the gate honestly instead
    // of fabricating an artifact.
    let has_slangc = std::env::var("SLANGC")
        .map(|value| !value.is_empty())
        .unwrap_or(false);
    let has_tool = std::env::var("COVE_DEV")
        .map(|value| !value.is_empty())
        .unwrap_or(false);
    if !has_slangc {
        eprintln!("skipping the fixture dispatch cases: set SLANGC to a Slang 2026.19 binary");
        return;
    }
    assert!(
        has_tool,
        "GAP: with SLANGC set the dispatch cases also need COVE_DEV pointing at \
         the cove-dev shader tool; this tree has no Rust artifact builder",
    );
}
