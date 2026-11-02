//! Translation of `Tests/CoveUIComposeTests/ImagePipelineTests.swift`.
//!
//! Behaviour checklist:
//! - the registry evicts least-recently-used assets past its byte and count
//!   budgets, and an asset larger than the whole budget is still returned
//! - EXIF orientation is read from the APP1 block and applied to pixels
//! - a two-frame GIF decodes as an animation with browser-style durations
//! - playback picks frames by elapsed time and stops after finite loops
//!
//! Model difference: the Swift suite's two `async` cases (`inflightSharing`,
//! `inflightCancellation`) test an actor-based `ImageInflightLoads` with `await`
//! continuations and `Task` cancellation. The Rust `ImageInflightLoads`
//! (`image_load_services.rs:248-306`) is a synchronous, single-threaded
//! `RefCell` join table with no cancellation path — there is no `Task` to cancel
//! and no `await` to interleave at. `concurrent_requests_share_one_load` ports
//! the sharing invariant that is expressible (a second request for an in-flight
//! key never re-enters `start`, and the table empties afterwards); the
//! cancellation invariant has no Rust analogue and is recorded here.

use crate::support;

use std::sync::Arc;

use cove_imaging::{DecodedAnimation, DecodedImage, ImageAsset, ImageDecoder};
use cove_imaging::{ImageAssetRegistry, ImageCacheLimits, ImageOrientation, PixelSize};
use cove_ui_compose::primitives::media::AsyncImageAnimation;

use support::GlobalTestLock;

fn image(value: u8, width: u32, height: u32) -> DecodedImage {
    DecodedImage::new(width, height, vec![value; (width * height * 4) as usize])
        .expect("a valid RGBA bitmap")
}

fn pixel_size(width: u32, height: u32) -> PixelSize {
    PixelSize::new(width, height).expect("a valid pixel size")
}

fn limits(maximum_bytes: usize, maximum_count: usize) -> ImageCacheLimits {
    ImageCacheLimits {
        maximum_bytes,
        maximum_count,
    }
}

#[test]
fn the_registry_evicts_least_recently_used_assets() {
    GlobalTestLock::locked(|| {
        // Three 4x4 assets include their 2x2 and 1x1 levels: 84 bytes each.
        let registry = ImageAssetRegistry::new(limits(84 * 3, 10));
        for (key, value) in [("a", 1), ("b", 2), ("c", 3)] {
            registry
                .register(key, image(value, 4, 4))
                .expect("the asset registers");
        }
        assert_eq!(
            registry.cached_bytes(),
            252,
            "three 4x4 assets include their 2x2 and 1x1 levels"
        );

        assert!(registry.cached("a").is_some(), "touching a makes it recent");

        registry
            .register("d", image(4, 4, 4))
            .expect("the fourth asset registers");
        assert!(
            registry.cached("b").is_none(),
            "the least recently used asset is evicted"
        );
        assert!(registry.cached("a").is_some(), "a survived its touch");
        assert!(registry.cached("c").is_some(), "c is still within budget");
        assert!(registry.cached("d").is_some(), "the new asset is cached");

        registry.set_limits(limits(usize::MAX, 1));
        assert_eq!(
            registry.cached_count(),
            1,
            "lowering the count budget evicts down to one"
        );

        // An entry larger than the whole budget is still returned and kept.
        let tiny = ImageAssetRegistry::new(limits(8, 4));
        let big = tiny
            .register("big", image(9, 4, 4))
            .expect("an oversized asset still registers");
        assert!(
            tiny.cached("big")
                .is_some_and(|hit| Arc::ptr_eq(&hit, &big)),
            "an oversized asset is returned rather than dropped"
        );

        registry.clear();
        assert_eq!(registry.cached_bytes(), 0, "clear resets the byte count");
        assert_eq!(registry.cached_count(), 0, "clear drops every entry");
    });
}

#[test]
fn exif_orientation_is_read_from_app1_and_applied_to_pixels() {
    GlobalTestLock::locked(|| {
        let mut jpeg: Vec<u8> = vec![0xFF, 0xD8, 0xFF, 0xE1, 0x00, 0x22];
        jpeg.extend_from_slice(b"Exif");
        jpeg.extend_from_slice(&[0, 0]);
        jpeg.extend_from_slice(&[0x4D, 0x4D, 0x00, 0x2A, 0x00, 0x00, 0x00, 0x08]);
        jpeg.extend_from_slice(&[
            0x00, 0x01, 0x01, 0x12, 0x00, 0x03, 0x00, 0x00, 0x00, 0x01, 0x00, 0x06, 0x00, 0x00,
        ]);
        jpeg.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0xFF, 0xD9]);

        assert_eq!(
            ImageOrientation::read(&jpeg),
            6,
            "an orientation-6 EXIF tag is read from APP1"
        );
        assert_eq!(
            ImageOrientation::read(&[0x89, 0x50, 0x4E, 0x47]),
            1,
            "non-JPEG data reports the identity orientation"
        );

        // 2x1: red then green.
        let source = DecodedImage::new(2, 1, vec![255, 0, 0, 255, 0, 255, 0, 255])
            .expect("a valid 2x1 bitmap");

        let rotated = ImageOrientation::apply(6, source.clone());
        assert_eq!(
            (rotated.width(), rotated.height()),
            (1, 2),
            "orientation 6 rotates a 2x1 into a 1x2"
        );
        assert_eq!(
            &rotated.pixels()[0..4],
            &[255, 0, 0, 255],
            "the red pixel lands on top"
        );

        let counter_clockwise = ImageOrientation::apply(8, source.clone());
        assert_eq!(
            &counter_clockwise.pixels()[0..4],
            &[0, 255, 0, 255],
            "orientation 8 puts green on top"
        );

        let mirrored = ImageOrientation::apply(2, source);
        assert_eq!(
            &mirrored.pixels()[0..4],
            &[0, 255, 0, 255],
            "orientation 2 mirrors horizontally"
        );

        assert!(
            ImageOrientation::swaps_axes(5),
            "orientation 5 transposes the axes"
        );
        assert!(
            !ImageOrientation::swaps_axes(3),
            "orientation 3 preserves the axes"
        );
    });
}

#[test]
fn a_two_frame_gif_decodes_as_an_animation() {
    GlobalTestLock::locked(|| {
        let mut gif: Vec<u8> = b"GIF89a".to_vec();
        gif.extend_from_slice(&[0x01, 0x00, 0x01, 0x00, 0x80, 0x00, 0x00]);
        gif.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x00]);

        fn frame(data: u8, delay: u8) -> Vec<u8> {
            let mut out = vec![
                0x21, 0xF9, 0x04, 0x00, delay, 0x00, 0x00, 0x00, 0x2C, 0x00, 0x00, 0x00, 0x00,
                0x01, 0x00, 0x01, 0x00, 0x00, 0x02, 0x02, data, 0x01, 0x00,
            ];
            out.truncate(23);
            out
        }

        gif.extend_from_slice(&frame(0x44, 10));
        gif.extend_from_slice(&frame(0x4C, 1));
        gif.push(0x3B);

        assert!(
            ImageDecoder::is_animatable(&gif, "gif"),
            "a GIF stream is animatable"
        );
        let animation = ImageDecoder::decode_animation(&gif, "gif", pixel_size(8, 8), u64::MAX, 64)
            .expect("the GIF decodes")
            .expect("the GIF yields an animation");
        assert_eq!(animation.frames.len(), 2, "both frames decode");
        assert_eq!(
            animation.durations,
            vec![0.1, 0.1],
            "a 1-centisecond delay is clamped to the 0.1s browser floor"
        );
        assert_eq!(
            &animation.frames[0].pixels()[0..3],
            &[255, 255, 255],
            "the first frame's palette entry is white"
        );
        assert_eq!(
            &animation.frames[1].pixels()[0..3],
            &[0, 0, 0],
            "the second frame's palette entry is black"
        );

        let not_animated =
            ImageDecoder::decode_animation(&[0x89, 0x50], "png", pixel_size(8, 8), u64::MAX, 64)
                .expect("a non-animatable stream decodes without error");
        assert!(
            not_animated.is_none(),
            "a non-animatable stream yields no animation"
        );
    });
}

#[test]
fn playback_picks_frames_by_elapsed_time_and_stops_after_finite_loops() {
    GlobalTestLock::locked(|| {
        let frames: Vec<Arc<ImageAsset>> = (0..3)
            .map(|_| ImageAsset::new(image(0, 1, 1)).expect("a 1x1 asset"))
            .collect();
        let durations = vec![0.1, 0.2, 0.1];

        let looping = AsyncImageAnimation::new(frames.clone(), durations.clone(), 0);
        assert_eq!(
            looping.frame_at(0.05),
            (0, false),
            "inside the first frame's duration"
        );
        assert_eq!(
            looping.frame_at(0.15),
            (1, false),
            "inside the second frame"
        );
        assert_eq!(looping.frame_at(0.35), (2, false), "inside the third frame");
        assert_eq!(
            looping.frame_at(0.45),
            (0, false),
            "a looping animation wraps to the first frame"
        );

        let once = AsyncImageAnimation::new(frames, durations, 1);
        assert_eq!(
            once.frame_at(0.5),
            (2, true),
            "a finite loop holds the last frame and reports completion"
        );
    });
}

#[test]
fn concurrent_requests_share_one_load() {
    GlobalTestLock::locked(|| {
        use cove_ui_compose::primitives::media::{AsyncImageError, ImageInflightLoads};

        let loads = ImageInflightLoads::shared();
        let starts = std::cell::Cell::new(0u32);

        // The in-flight table is synchronous: a nested request for the same key
        // joins the outer load instead of starting a second one. That is the
        // shareable half of the Swift invariant.
        let outer = loads.load("k", || {
            starts.set(starts.get() + 1);
            let inner = loads.load("k", || {
                starts.set(starts.get() + 1);
                Err(AsyncImageError::InvalidResponse)
            });
            assert!(
                inner.is_err(),
                "a nested request joins the outer load rather than starting one"
            );
            Err(AsyncImageError::InvalidResponse)
        });

        assert!(outer.is_err(), "the outer load reports its own failure");
        assert_eq!(
            starts.get(),
            1,
            "only one decode started for the shared key"
        );
        assert_eq!(
            loads.active_count(),
            0,
            "the in-flight table is empty once the load finishes"
        );

        // ``DecodedAnimation`` exists so the module's import is exercised for the
        // animation half of this suite.
        let _ = std::any::TypeId::of::<DecodedAnimation>();
    });
}
