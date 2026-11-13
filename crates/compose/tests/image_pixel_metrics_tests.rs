//! Decoded-versus-display pixel metrics recorded by the image primitive.
//!
//! The DevTools oversized-image overlay may only use real data, so these tests
//! pin what the draw path attaches: the decoded bitmap extent, the painted
//! rect scaled by the draw list's pixel scale, the vector flag, and no metrics
//! at all when the source size is unknown (including after it becomes unknown).

use cove_core::scene::{
    IMAGE_DISPLAY_PIXELS, IMAGE_SOURCE_PIXELS, IMAGE_VECTOR_SOURCE, Node, image_pixel_metrics,
};
use cove_core::{DrawList, Point, Rect, Size};
use cove_imaging::{DecodedImage, ImageAsset};
use cove_ui_compose::PrimitiveView;
use cove_ui_compose::primitives::media::{Image, ImageContentMode, ImageSource};

fn asset(width: u32, height: u32) -> ImageSource {
    let pixels = vec![255u8; (width * height * 4) as usize];
    let decoded = DecodedImage::new(width, height, pixels).expect("a valid decoded bitmap");
    ImageSource::Asset(ImageAsset::new(decoded).expect("a valid image asset"))
}

fn paint(image: &Image, frame: Rect, pixel_scale: f32) -> Node {
    let node = image.make_node();
    node.set_frame(frame);
    image.update_node(&node);
    let mut list = DrawList::default();
    list.pixel_scale = pixel_scale;
    (node.draw().expect("the image installs a draw hook"))(&mut list, Point::new(frame.x, frame.y));
    node
}

#[test]
fn metrics_use_the_decoded_source_and_the_scaled_draw_rect() {
    let image = Image::new(
        asset(400, 300),
        Size {
            width: 200.0,
            height: 150.0,
        },
    );
    let node = paint(&image, Rect::new(10.0, 20.0, 100.0, 75.0), 2.0);

    assert_eq!(
        node.attachment::<(f32, f32)>(IMAGE_SOURCE_PIXELS),
        Some((400.0, 300.0)),
        "source pixels are the decoded extent, not the authored points"
    );
    assert_eq!(
        node.attachment::<(f32, f32)>(IMAGE_DISPLAY_PIXELS),
        Some((200.0, 150.0)),
        "display pixels scale the painted rect by the draw list pixel scale"
    );
    assert_eq!(
        node.attachment::<bool>(IMAGE_VECTOR_SOURCE),
        Some(false),
        "a bitmap asset is not a vector source"
    );
    let metrics = image_pixel_metrics(&node).expect("both extents are recorded");
    assert_eq!(metrics.area_ratio(), Some(4.0));
}

#[test]
fn fit_records_the_letterboxed_display_rect() {
    let image = Image::new(
        asset(200, 100),
        Size {
            width: 100.0,
            height: 100.0,
        },
    )
    .content_mode(ImageContentMode::Fit);
    let node = paint(&image, Rect::new(0.0, 0.0, 100.0, 100.0), 1.0);

    assert_eq!(
        node.attachment::<(f32, f32)>(IMAGE_SOURCE_PIXELS),
        Some((200.0, 100.0))
    );
    assert_eq!(
        node.attachment::<(f32, f32)>(IMAGE_DISPLAY_PIXELS),
        Some((100.0, 50.0)),
        "`.fit` paints the letterboxed rect, not the container"
    );
}

#[test]
fn a_known_texture_size_records_real_metrics() {
    let image = Image::new(
        ImageSource::Texture {
            texture_id: 7,
            size: Some(Size {
                width: 640.0,
                height: 480.0,
            }),
        },
        Size {
            width: 160.0,
            height: 120.0,
        },
    );
    let node = paint(&image, Rect::new(0.0, 0.0, 80.0, 60.0), 2.0);
    assert_eq!(
        image_pixel_metrics(&node),
        Some(cove_core::scene::ImagePixelMetrics {
            source: (640.0, 480.0),
            display: (160.0, 120.0),
            vector: false,
        })
    );
}

#[test]
fn an_unknown_source_size_records_nothing_and_clears_stale_metrics() {
    let known = Image::new(
        asset(64, 64),
        Size {
            width: 32.0,
            height: 32.0,
        },
    );
    let node = paint(&known, Rect::new(0.0, 0.0, 32.0, 32.0), 1.0);
    assert!(image_pixel_metrics(&node).is_some());

    // The same live node now samples a texture with no reported extent, as a
    // failed decode does. A previous frame's metrics must not survive.
    let unknown = Image::new(
        ImageSource::Texture {
            texture_id: 0,
            size: None,
        },
        Size {
            width: 32.0,
            height: 32.0,
        },
    );
    unknown.update_node(&node);
    let mut list = DrawList::default();
    (node.draw().expect("the image installs a draw hook"))(&mut list, Point::default());
    assert!(
        image_pixel_metrics(&node).is_none(),
        "an unknown source size must not leave a stale ratio"
    );
    assert_eq!(node.attachment::<(f32, f32)>(IMAGE_SOURCE_PIXELS), None);
    assert_eq!(node.attachment::<(f32, f32)>(IMAGE_DISPLAY_PIXELS), None);
}
