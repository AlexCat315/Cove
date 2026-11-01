//! Translation of `Tests/CoveUIRuntimeTests/ImageAssetRegistryTests.swift`.
//!
//! Registration is the one place concurrent decodes meet: sixty-four threads
//! asking for the same key must observe a single immutable asset, an invalid
//! payload must never poison the cache, and a retained draw list must keep its
//! snapshot alive after the registry forgets it.
//!
//! Behaviour checklist:
//! - concurrent registration publishes one immutable asset per key
//! - invalid payloads never enter the cache
//! - `clear` preserves retained snapshots
//!
//! Difference from Swift: the Swift `Asset` is a reference type and the test
//! compares identity (`===`). The Rust `ImageAsset` is published as an `Arc`,
//! so identity is `Arc::ptr_eq`.

use std::sync::Arc;
use std::sync::mpsc;

use cove_imaging::{
    DecodedImage, ImageAsset, ImageAssetRegistry, ImageCacheLimits, ImageDecodeError,
};

fn registry() -> ImageAssetRegistry {
    ImageAssetRegistry::new(ImageCacheLimits::default())
}

fn pixel(r: u8, g: u8, b: u8, a: u8) -> DecodedImage {
    DecodedImage::new(1, 1, vec![r, g, b, a]).expect("a 1x1 RGBA payload is valid")
}

#[test]
fn concurrent_registration_publishes_one_immutable_asset_per_key() {
    let registry = Arc::new(registry());
    let image = pixel(17, 99, 211, 255);

    let (sender, receiver) = mpsc::channel::<Result<Arc<ImageAsset>, ImageDecodeError>>();
    let threads: Vec<_> = (0..64)
        .map(|_| {
            let registry = Arc::clone(&registry);
            let image = image.clone();
            let sender = sender.clone();
            std::thread::spawn(move || {
                let _ = sender.send(registry.register("shared", image));
            })
        })
        .collect();
    drop(sender);

    let values: Vec<Arc<ImageAsset>> = receiver
        .iter()
        .map(|result| result.expect("every registration succeeds"))
        .collect();
    for thread in threads {
        thread.join().expect("the registering thread did not panic");
    }
    assert_eq!(values.len(), 64, "all 64 registrations reported");

    let asset = values.first().expect("at least one registration").clone();
    assert!(
        values.iter().all(|value| Arc::ptr_eq(value, &asset)),
        "concurrent callers all observe the same published asset",
    );
    assert!(
        registry
            .cached("shared")
            .is_some_and(|cached| Arc::ptr_eq(&cached, &asset)),
        "the cache holds that exact asset",
    );
    assert_eq!(asset.image.pixels(), image.pixels());

    let foreign = ImageAssetRegistry::new(ImageCacheLimits::default())
        .register("shared", image)
        .expect("a foreign registry registers the same key");
    assert_ne!(
        foreign.texture_id, asset.texture_id,
        "a different registry allocates its own texture id",
    );
    assert!(
        asset.texture_id >= 0x1000_0000 && asset.texture_id < 0x4000_0000,
        "texture ids live in the image-asset range, got {:#x}",
        asset.texture_id,
    );
}

#[test]
fn invalid_payloads_never_enter_the_cache() {
    let registry = registry();
    for pixels in [vec![], vec![0u8]] {
        assert!(
            DecodedImage::new(1, 1, pixels.clone()).is_err(),
            "a {}-byte RGBA payload for a 1x1 image is invalid",
            pixels.len(),
        );
        assert!(
            registry.cached("bad").is_none(),
            "a rejected payload never reached the cache",
        );
    }
    // A zero-height or absurdly wide image is rejected by the constructor too,
    // so an asset can never be built from it.
    assert!(DecodedImage::new(0, 1, vec![]).is_err(), "zero width");
    assert!(
        DecodedImage::new(u32::MAX, 2, vec![]).is_err(),
        "an image wider than the atlas limit",
    );
}

#[test]
fn clear_preserves_retained_snapshots() {
    let registry = registry();
    let mut asset = Some(
        registry
            .register("pixel", pixel(1, 2, 3, 4))
            .expect("a 1x1 payload registers"),
    );
    // A weak reference, so the test can observe when the last owner goes away.
    let retained = Arc::downgrade(asset.as_ref().expect("the asset exists"));

    let mut list = cove_core::DrawList::default();
    list.resources
        .retain(asset.clone().expect("the asset exists"));
    list.resources
        .retain(asset.clone().expect("the asset exists"));
    assert_eq!(
        list.resources.len(),
        1,
        "retaining the same asset twice keeps one strong reference",
    );

    let mut snapshot =
        cove_core::scene::DrawListSnapshot::new(Vec::new(), Vec::new(), Vec::new(), [1.0, 1.0]);
    snapshot.resources = list.resources.clone();
    list.reset();
    registry.clear();
    asset = None;

    assert_eq!(registry.revision(), 1, "clear bumps the revision once");
    assert_eq!(snapshot.resources.len(), 1);
    assert!(
        retained.upgrade().is_some(),
        "the snapshot still owns the asset after the registry forgot it",
    );

    let mut bytes: Vec<u8> = Vec::new();
    snapshot
        .resources
        .for_each(|asset: &ImageAsset| bytes = asset.image.pixels().to_vec());
    assert_eq!(bytes, vec![1, 2, 3, 4]);

    snapshot.resources.reset();
    assert!(
        retained.upgrade().is_none(),
        "releasing the snapshot releases the last strong reference",
    );
}
