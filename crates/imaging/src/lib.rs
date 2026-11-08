//! Native image decoding, retained textures and bounded CPU asset ownership.
mod animation;
mod bundle_image_resource;
mod image_asset_registry;
mod image_decoder;
mod native;
mod orientation;
mod progressive;
mod rasterization;
mod texture_image;
pub use animation::DecodedAnimation;
pub use bundle_image_resource::BundleImageResource;
pub use cove_core as core;
pub use cove_core::Bitmap as DecodedImage;
pub use image_asset_registry::{
    ImageAsset, ImageAssetRegistry, ImageAssetRegistryHolder, ImageCacheLimits, allocate_texture_id,
};
pub use image_decoder::{ImageDecodeError, ImageDecoder, PixelSize};
pub use orientation::ImageOrientation;
pub use texture_image::{TextureImage, TextureImageBacking};
