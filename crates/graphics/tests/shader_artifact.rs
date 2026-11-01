//! Translation of Graphics/Tests/NativeRHITests/ShaderArtifactTests.swift.
//!
//! Artifact round-tripping, workgroup-size validation and the conservative
//! capability defaults. Pure CPU, so it runs on every host.

use cove_graphics::{Capabilities, ShaderArtifact, ShaderFormat, ShaderStage, ThreadgroupSize};

fn artifact() -> ShaderArtifact {
    ShaderArtifact::new(
        ShaderStage::Compute,
        ShaderFormat::MslSource,
        "main",
        b"shader".to_vec(),
        "test",
    )
}

#[test]
fn artifact_round_trip_keeps_workgroup_size_and_code() {
    let mut artifact = artifact();
    artifact.interface.threadgroup_size = ThreadgroupSize::new(8, 2, 1);

    // The artifact crosses the process boundary as JSON (precompiled libraries
    // are cached to disk), so the round trip must preserve both the workgroup
    // shape and the code bytes.
    let encoded = serde_json::to_vec(&artifact).expect("serialize");
    let decoded: ShaderArtifact = serde_json::from_slice(&encoded).expect("deserialize");

    let descriptor = decoded.module_descriptor(&[]).expect("module descriptor");
    assert_eq!(descriptor.threadgroup_size, ThreadgroupSize::new(8, 2, 1));
    assert_eq!(descriptor.code, artifact.code);
}

#[test]
fn invalid_and_overflowing_workgroup_sizes_are_rejected() {
    assert!(
        ThreadgroupSize::new(0, 1, 1).validate().is_err(),
        "zero x is invalid"
    );
    assert!(
        ThreadgroupSize::new(-1, 1, 1).validate().is_err(),
        "negative x is invalid"
    );
    assert!(
        ThreadgroupSize::new(i64::MAX, 2, 1).validate().is_err(),
        "a product that overflows is invalid"
    );
    // The base case must stay valid so the rejections above mean something.
    assert!(ThreadgroupSize::new(64, 1, 1).validate().is_ok());
}

#[test]
fn unimplemented_capabilities_default_to_false() {
    let capabilities = Capabilities::default();
    assert!(
        !capabilities.ray_tracing.pipelines,
        "no backend advertises ray pipelines by default"
    );
    assert!(
        !capabilities.ray_tracing.update,
        "no backend advertises ray updates by default"
    );
    assert!(
        !capabilities.mesh_shading.indirect,
        "no backend advertises indirect mesh by default"
    );
    assert_eq!(
        capabilities.max_queues.graphics, 0,
        "a default device reports no queues"
    );
}

#[test]
fn empty_artifact_is_rejected() {
    // An artifact with no entry point or no code cannot become a module.
    let mut missing_code = artifact();
    missing_code.code.clear();
    assert!(
        missing_code.module_descriptor(&[]).is_err(),
        "empty code is rejected"
    );

    let mut missing_entry = artifact();
    missing_entry.entry_point.clear();
    assert!(
        missing_entry.module_descriptor(&[]).is_err(),
        "empty entry point is rejected"
    );
}
