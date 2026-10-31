//! Translation of Graphics/Tests/NativeRHITests/RenderAttachmentValidationTests.swift.
//!
//! Attachment signature construction and the rejections around resolve targets,
//! depth mismatches, duplicate sources and aliasing. Pure CPU validation, so it
//! runs on every host.

use cove_graphics::{
    RenderColorTarget, RenderDepthTarget, RenderPassDescriptor, RenderTextureInfo, Texture,
    TextureFormat, TextureUsage, rhi_render_pass_signature,
};

fn info(
    samples: usize,
    format: TextureFormat,
    extent: [usize; 2],
    usage: TextureUsage,
    single_layer_2d: bool,
) -> RenderTextureInfo<TextureFormat> {
    RenderTextureInfo {
        supports_color_resolve: true,
        extent,
        sample_count: samples,
        format,
        usage,
        single_layer_2d,
        is_depth: format.is_depth(),
    }
}

fn default_info(samples: usize, format: TextureFormat) -> RenderTextureInfo<TextureFormat> {
    info(samples, format, [32, 24], TextureUsage::COLOR_TARGET, true)
}

fn color_target(id: u32) -> RenderColorTarget {
    RenderColorTarget::new(Texture::new(id))
}

#[test]
fn resolve_and_pipeline_signature() {
    let mut color = RenderColorTarget::new(Texture::new(1));
    color.resolve_texture = Some(Texture::new(2));
    let pass =
        RenderPassDescriptor::new(vec![color]).with_depth(RenderDepthTarget::new(Texture::new(3)));

    // Resolve target: single sample, sampled-capable.
    let mut resolve = default_info(1, TextureFormat::Rgba8Unorm);
    resolve.usage = TextureUsage::COLOR_TARGET;
    let depth = info(
        4,
        TextureFormat::Depth32Float,
        [32, 24],
        TextureUsage::DEPTH_STENCIL_TARGET,
        true,
    );
    let signature = rhi_render_pass_signature(&pass, |texture| {
        Ok(match texture.id() {
            1 => default_info(4, TextureFormat::Rgba8Unorm),
            2 => resolve,
            _ => depth,
        })
    })
    .expect("signature");

    assert_eq!(signature.extent, [32, 24]);
    assert_eq!(signature.sample_count, 4);
    signature
        .validate(
            &[TextureFormat::Rgba8Unorm],
            Some(&TextureFormat::Depth32Float),
            4,
        )
        .expect("matching pipeline");

    // A sample-count mismatch is rejected.
    assert!(
        signature
            .validate(
                &[TextureFormat::Rgba8Unorm],
                Some(&TextureFormat::Depth32Float),
                1
            )
            .is_err()
    );
    // A format mismatch is rejected.
    assert!(
        signature
            .validate(
                &[TextureFormat::Bgra8Unorm],
                Some(&TextureFormat::Depth32Float),
                4
            )
            .is_err()
    );
    // Omitting the depth attachment is rejected.
    assert!(
        signature
            .validate(&[TextureFormat::Rgba8Unorm], None, 4)
            .is_err()
    );
}

#[test]
fn resolve_rejects_shape_format_usage_samples_and_aliasing() {
    let mut target = RenderColorTarget::new(Texture::new(1));
    target.resolve_texture = Some(Texture::new(2));
    let pass = RenderPassDescriptor::new(vec![target]);

    // Every one of these resolve descriptions is invalid against a 4x MSAA
    // color attachment.
    let invalid_resolves = [
        default_info(4, TextureFormat::Rgba8Unorm),
        default_info(1, TextureFormat::Bgra8Unorm),
        info(
            1,
            TextureFormat::Rgba8Unorm,
            [33, 24],
            TextureUsage::COLOR_TARGET,
            true,
        ),
        info(
            1,
            TextureFormat::Rgba8Unorm,
            [32, 24],
            TextureUsage::SAMPLED,
            true,
        ),
        info(
            1,
            TextureFormat::Rgba8Unorm,
            [32, 24],
            TextureUsage::COLOR_TARGET,
            false,
        ),
    ];
    for resolve in invalid_resolves {
        let result = rhi_render_pass_signature(&pass, |texture| {
            Ok(if texture.id() == 1 {
                default_info(4, TextureFormat::Rgba8Unorm)
            } else {
                resolve
            })
        });
        assert!(
            result.is_err(),
            "resolve shape/format/usage/sample mismatch must be rejected"
        );
    }

    // A single-sample color target cannot carry a resolve destination.
    assert!(
        rhi_render_pass_signature(&pass, |_| Ok(default_info(1, TextureFormat::Rgba8Unorm)))
            .is_err(),
        "resolve requires a multisampled source"
    );

    // Integer formats without color-resolve support are rejected.
    let mut integer = default_info(1, TextureFormat::R32Uint);
    integer.supports_color_resolve = false;
    assert!(
        rhi_render_pass_signature(&pass, |texture| {
            Ok(if texture.id() == 1 {
                integer
            } else {
                default_info(1, TextureFormat::R32Uint)
            })
        })
        .is_err(),
        "an integer format without resolve support cannot resolve"
    );

    // Resolving to the attachment itself aliases.
    let mut aliased = RenderColorTarget::new(Texture::new(1));
    aliased.resolve_texture = Some(Texture::new(1));
    assert!(
        rhi_render_pass_signature(&RenderPassDescriptor::new(vec![aliased]), |_| Ok(
            default_info(4, TextureFormat::Rgba8Unorm)
        ))
        .is_err(),
        "a resolve destination must not alias its source"
    );

    // The same texture appearing as both an attachment and a resolve target
    // across two color targets also aliases.
    let mut first = RenderColorTarget::new(Texture::new(1));
    first.resolve_texture = Some(Texture::new(2));
    assert!(
        rhi_render_pass_signature(
            &RenderPassDescriptor::new(vec![first, color_target(2)]),
            |_| Ok(default_info(4, TextureFormat::Rgba8Unorm))
        )
        .is_err(),
        "duplicate attachment and resolve sources must be rejected"
    );
}

#[test]
fn attachments_reject_mismatched_depth_and_duplicate_sources() {
    let pass = RenderPassDescriptor::new(vec![color_target(1)])
        .with_depth(RenderDepthTarget::new(Texture::new(2)));

    // Depth attachment must be a depth format, share the extent, and be a
    // single-layer 2D texture.
    let invalid_depths = [
        info(
            1,
            TextureFormat::Depth32Float,
            [32, 24],
            TextureUsage::DEPTH_STENCIL_TARGET,
            true,
        ),
        info(
            4,
            TextureFormat::Depth32Float,
            [32, 25],
            TextureUsage::DEPTH_STENCIL_TARGET,
            true,
        ),
        info(
            4,
            TextureFormat::Depth32Float,
            [32, 24],
            TextureUsage::DEPTH_STENCIL_TARGET,
            false,
        ),
    ];
    for depth in invalid_depths {
        assert!(
            rhi_render_pass_signature(&pass, |texture| {
                Ok(if texture.id() == 1 {
                    default_info(4, TextureFormat::Rgba8Unorm)
                } else {
                    depth
                })
            })
            .is_err(),
            "a depth attachment that mismatches samples, extent or layering is rejected"
        );
    }

    // A color attachment in the depth slot is rejected.
    assert!(
        rhi_render_pass_signature(&pass, |_| Ok(default_info(4, TextureFormat::Rgba8Unorm)))
            .is_err(),
        "the depth slot requires a depth format"
    );

    // The same texture twice as a color target is rejected.
    assert!(
        rhi_render_pass_signature(
            &RenderPassDescriptor::new(vec![color_target(1), color_target(1)]),
            |_| Ok(default_info(4, TextureFormat::Rgba8Unorm))
        )
        .is_err(),
        "duplicate color attachments must be rejected"
    );
}

#[test]
fn empty_pass_is_rejected() {
    assert!(
        rhi_render_pass_signature(&RenderPassDescriptor::new(vec![]), |_| Ok(default_info(
            1,
            TextureFormat::Rgba8Unorm
        )))
        .is_err(),
        "a render pass requires at least one attachment"
    );
}
