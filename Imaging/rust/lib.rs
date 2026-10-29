//! Native image decoding, retained textures and bounded CPU asset ownership.
mod native;
mod image_decoder;
mod animation;
mod orientation;
mod rasterization;
mod progressive;
mod texture_image;
mod bundle_image_resource;
mod image_asset_registry;
pub use cove_core as core;
pub use image_decoder::{ImageDecodeError, ImageDecoder, PixelSize};
pub use animation::DecodedAnimation;
pub use orientation::ImageOrientation;
pub use texture_image::{TextureImage, TextureImageBacking};
pub use bundle_image_resource::BundleImageResource;
pub use image_asset_registry::{ImageAsset, ImageAssetRegistry, ImageAssetRegistryHolder, ImageCacheLimits};
pub use cove_core::Bitmap as DecodedImage;
