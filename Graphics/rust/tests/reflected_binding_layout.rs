//! Translation of Graphics/Tests/NativeRHITests/ReflectedBindingLayoutTests.swift.
//!
//! Pure CPU: reflection merges shader interfaces without a device.
//!
//! The companion Vulkan `PresentationPolicyTests.swift` is already covered by the
//! internal unit tests in `vulkan/presentation.rs` (`choose_presentation_mode`
//! cases); that module is crate-private, so an integration test cannot reach it
//! and duplicating it here would add no coverage.

use cove_graphics::{
    BindingType, BufferBindingLayout, ReflectedShaderBinding, ShaderArtifact, ShaderFormat,
    ShaderStage, ShaderVisibility, reflected_binding_layout,
};

fn artifact(stage: ShaderStage, bindings: Vec<ReflectedShaderBinding>) -> ShaderArtifact {
    let mut artifact = ShaderArtifact::new(stage, ShaderFormat::MslSource, "main", vec![1], "test");
    artifact.interface.bindings = bindings;
    artifact
}

fn binding(name: &str, slot: u32, binding_type: BindingType) -> ReflectedShaderBinding {
    ReflectedShaderBinding {
        buffer: BufferBindingLayout::default(),
        name: name.to_string(),
        slot,
        space: 0,
        binding_type,
    }
}

// MARK: Reflection

#[test]
fn stage_merge_preserves_buffer_abi_and_sorts_slots() {
    let mut uniform = binding("uniform", 0, BindingType::UniformBuffer);
    uniform.buffer.read_only = true;
    uniform.buffer.element_stride = 16;
    let texture = binding("image", 1, BindingType::Texture);
    let sampler = binding("sampler", 2, BindingType::Sampler);

    // Vertex declares the uniform; fragment declares the sampler, texture and
    // the same uniform. The merged layout must keep one uniform entry with the
    // original ABI and widen its visibility to graphics.
    let layout = reflected_binding_layout(&[
        artifact(ShaderStage::Vertex, vec![uniform.clone()]),
        artifact(
            ShaderStage::Fragment,
            vec![sampler, texture, uniform.clone()],
        ),
    ])
    .expect("reflected layout");

    let slots: Vec<u32> = layout.entries.iter().map(|entry| entry.slot).collect();
    assert_eq!(slots, vec![0, 1, 2], "entries are sorted by slot");
    assert_eq!(
        layout.entries[0].buffer, uniform.buffer,
        "the uniform keeps its element stride and read-only flag"
    );
    assert_eq!(
        layout.entries[0].visibility,
        ShaderVisibility::VERTEX | ShaderVisibility::FRAGMENT,
        "a binding seen by both stages is visible to both"
    );
    assert_eq!(
        layout.entries[1].visibility,
        ShaderVisibility::FRAGMENT,
        "a fragment-only binding stays fragment-visible"
    );
}

#[test]
fn conflicting_resources_spaces_and_duplicate_slots_are_rejected() {
    let original = binding("uniform", 0, BindingType::UniformBuffer);

    // Changing any part of the ABI for the same slot is a hard error rather
    // than a silent overwrite.
    let mut storage = original.clone();
    storage.binding_type = BindingType::StorageBuffer;
    assert!(
        reflected_binding_layout(&[
            artifact(ShaderStage::Vertex, vec![original.clone()]),
            artifact(ShaderStage::Fragment, vec![storage]),
        ])
        .is_err(),
        "a type conflict on one slot is rejected"
    );

    let mut stride = original.clone();
    stride.buffer.element_stride = 16;
    assert!(
        reflected_binding_layout(&[
            artifact(ShaderStage::Vertex, vec![original.clone()]),
            artifact(ShaderStage::Fragment, vec![stride]),
        ])
        .is_err(),
        "a stride conflict on one slot is rejected"
    );

    let mut read_only = original.clone();
    read_only.buffer.read_only = true;
    assert!(
        reflected_binding_layout(&[
            artifact(ShaderStage::Vertex, vec![original.clone()]),
            artifact(ShaderStage::Fragment, vec![read_only]),
        ])
        .is_err(),
        "a mutability conflict on one slot is rejected"
    );

    let mut space = original.clone();
    space.space = 1;
    assert!(
        reflected_binding_layout(&[
            artifact(ShaderStage::Vertex, vec![original.clone()]),
            artifact(ShaderStage::Fragment, vec![space]),
        ])
        .is_err(),
        "a descriptor-space conflict on one slot is rejected"
    );

    // The same slot declared twice within one stage is also rejected.
    assert!(
        reflected_binding_layout(&[artifact(
            ShaderStage::Vertex,
            vec![original.clone(), original]
        )])
        .is_err(),
        "a duplicate slot within one stage is rejected"
    );
}
