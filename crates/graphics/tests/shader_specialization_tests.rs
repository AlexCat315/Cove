//! Translation of `Graphics/Tests/NativeRHITests/ShaderSpecializationTests.swift`.
//!
//! An offline shader artifact declares reflected threadgroup dimensions and
//! typed specialization constants. Resolving a module descriptor must fill in
//! each declared constant's default, honour caller overrides, remap the
//! threadgroup size when `localSize` is specialized, and reject overrides that do
//! not match the reflected signature — a wrong id, a wrong type, or a duplicate.
//!
//! Reachability: the second Swift case (`testMetalPhysicalGroupsAndTypedFunctionConstants`)
//! compiles a `.slang` fixture through the first-party `cove-dev` tool, which
//! needs `SLANGC` plus a Swift toolchain build, and then runs a real compute
//! dispatch. That is the same environment gate `ShaderFixtures.compile` applies
//! (`throw XCTSkip("Set SLANGC ...")`), so it is recorded here as a documented
//! skip rather than asserted against a fabricated artifact. The pure resolution
//! logic — which is what the first Swift case covers, and what the dispatch case
//! depends on — is asserted in full below and needs no compiler.
//!
//! Behaviour checklist:
//! - a declared constant resolves to its reflected default
//! - a caller override replaces the default and remaps the threadgroup size
//! - an undeclared id, a mistyped value and a duplicate id are all rejected
//! - the artifact round-trips through JSON and still resolves
//! - GAP: the Slang-compiled dispatch case is gated on `SLANGC`

use cove_graphics::{
    ReflectedShaderConstant, ShaderArtifact, ShaderConstantType, ShaderConstantValue, ShaderFormat,
    ShaderSpecializationConstant, ShaderStage, ThreadgroupSize, ThreadgroupSpecialization,
};

fn artifact() -> ShaderArtifact {
    let mut artifact = ShaderArtifact::new(
        ShaderStage::Compute,
        ShaderFormat::MslSource,
        "main",
        vec![1],
        "test",
    );
    artifact.interface.threadgroup_size = ThreadgroupSize::new(64, 1, 1);
    artifact.interface.specialization_constants = vec![ReflectedShaderConstant::new(
        0,
        "localSize",
        ShaderConstantType::Uint32,
    )];
    artifact.interface.threadgroup_specialization = ThreadgroupSpecialization {
        x: Some(0),
        y: None,
        z: None,
    };
    artifact
}

fn uint(id: u32, value: u32) -> ShaderSpecializationConstant {
    ShaderSpecializationConstant::new(id, ShaderConstantValue::Uint32(value))
}

#[test]
fn specialized_dimensions_and_types_validate() {
    let artifact = artifact();

    let default = artifact
        .module_descriptor(&[])
        .expect("the reflected default resolves");
    assert_eq!(
        default.threadgroup_size,
        ThreadgroupSize::new(64, 1, 1),
        "the artifact's declared threadgroup size is used by default",
    );
    assert_eq!(
        default.specialization_constants,
        vec![uint(0, 64)],
        "the declared constant resolves to its reflected default",
    );

    let overridden = artifact
        .module_descriptor(&[uint(0, 37)])
        .expect("the override resolves");
    assert_eq!(
        overridden.threadgroup_size,
        ThreadgroupSize::new(37, 1, 1),
        "a specialized localSize remaps the threadgroup size",
    );

    // Undeclared id, wrong type, and a duplicate id are each refused.
    for (label, constants) in [
        ("undeclared id", vec![uint(1, 37)]),
        (
            "mistyped value",
            vec![ShaderSpecializationConstant::new(
                0,
                ShaderConstantValue::Float32(37.0),
            )],
        ),
        ("duplicate id", vec![uint(0, 37), uint(0, 37)]),
    ] {
        assert!(
            artifact.module_descriptor(&constants).is_err(),
            "an override with {label} is rejected",
        );
    }
}

#[test]
fn artifact_round_trips_through_json() {
    let artifact = artifact();
    let encoded = serde_json::to_vec(&artifact).expect("the artifact serialises");
    let decoded: ShaderArtifact =
        serde_json::from_slice(&encoded).expect("the artifact deserialises");
    assert_eq!(decoded, artifact, "the artifact survives a JSON round trip");
    assert_eq!(
        decoded
            .module_descriptor(&[uint(0, 255)])
            .expect("the round-tripped artifact still resolves")
            .threadgroup_size
            .x,
        255,
        "a specialized size still remaps after a round trip",
    );
}

#[test]
fn dispatch_case_is_gated_on_the_slang_compiler() {
    // Swift: `ShaderFixtures.compile("specialization", ...)` throws
    // `XCTSkip("Set SLANGC to Slang 2026.19 for shader integration tests")` when
    // `SLANGC` is unset, and otherwise builds `Tools/Developer` with Swift and
    // runs a real Metal/Vulkan compute dispatch. There is no Rust shader
    // compiler in this crate, so the same gate applies: assert the gate rather
    // than fabricate a compiled artifact.
    let has_slangc = std::env::var("SLANGC")
        .map(|value| !value.is_empty())
        .unwrap_or(false);
    if !has_slangc {
        eprintln!("skipping the specialized-dispatch case: set SLANGC to a Slang 2026.19 binary");
        return;
    }
    // With a compiler present the Swift case still needs the `cove-dev` shader
    // tool, which is a Swift package this crate does not depend on. Record that
    // rather than silently passing.
    assert!(
        std::env::var("COVE_DEV")
            .map(|value| !value.is_empty())
            .unwrap_or(false),
        "GAP: with SLANGC set the dispatch case also needs COVE_DEV pointing at \
         the cove-dev shader tool; the Rust tree has no equivalent artifact builder",
    );
}
