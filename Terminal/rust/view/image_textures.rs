//! GPU-ready assets for Kitty graphics images, rebuilt only when an image's
//! generation changes and dropped when no placement shows it any more.

use crate::engine::TerminalImagePlacement;
use cove_imaging::{DecodedImage, ImageAsset};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

#[derive(Default)]
pub struct ImageTextures {
    assets: HashMap<u32, (u64, Arc<ImageAsset>)>,
}

impl ImageTextures {
    pub fn new() -> Self {
        Self::default()
    }

    /// Assets for the frame's placements, in draw order. `pixels` is consulted
    /// only for images that are new or whose generation changed.
    pub fn resolve(
        &mut self,
        placements: &[TerminalImagePlacement],
        pixels: impl Fn(u32) -> Option<DecodedImage>,
    ) -> Vec<(TerminalImagePlacement, Arc<ImageAsset>)> {
        let mut live = HashSet::new();
        let mut result = Vec::with_capacity(placements.len());
        for placement in placements {
            live.insert(placement.image_id);
            if let Some((generation, asset)) = self.assets.get(&placement.image_id)
                && *generation == placement.generation
            {
                result.push((*placement, asset.clone()));
                continue;
            }
            let Some(image) = pixels(placement.image_id) else { continue };
            let Ok(asset) = ImageAsset::new(image) else { continue };
            self.assets.insert(placement.image_id, (placement.generation, asset.clone()));
            result.push((*placement, asset));
        }
        self.assets.retain(|image_id, _| live.contains(image_id));
        result
    }

    pub fn remove_all(&mut self) {
        self.assets.clear();
    }

    pub fn len(&self) -> usize {
        self.assets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.assets.is_empty()
    }
}
