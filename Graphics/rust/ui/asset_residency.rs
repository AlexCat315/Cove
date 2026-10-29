use std::{any::Any,collections::HashMap,sync::{Arc,Weak}};
use cove_core::{DrawListResources,TextureId,TextureAsset,RetainedTextureAsset};
use cove_imaging::{ImageAsset,TextureImage};
/// Weak CPU ownership controls residency; recorded frames retain their GPU slots.
#[derive(Clone,Default)]
pub(super) struct UIAssetResidency{owners:HashMap<TextureId,Weak<dyn Any+Send+Sync>>}
pub(super) enum AssetOwner{Pixels(Arc<dyn TextureAsset>),Texture(Arc<TextureImage>)}
impl UIAssetResidency{
    pub fn expired(&mut self)->Vec<TextureId>{let ids=self.owners.iter().filter(|(_,owner)|owner.strong_count()==0).map(|(&id,_)|id).collect::<Vec<_>>();for id in &ids{self.owners.remove(id);}ids}
    pub fn pending(&mut self,resources:&DrawListResources)->Vec<(TextureId,AssetOwner,Arc<dyn Any+Send+Sync>)>{
        let mut result=Vec::new();let mut consider=|id,asset,owner:Arc<dyn Any+Send+Sync>|{if self.owners.get(&id).is_none_or(|old|!old.ptr_eq(&Arc::downgrade(&owner))){result.push((id,asset,owner));}};
        resources.for_each_retained::<ImageAsset>(|asset|consider(asset.texture_id,AssetOwner::Pixels(asset.clone()),asset));
        resources.for_each_retained::<RetainedTextureAsset>(|holder|{let asset=holder.0.clone();let id=asset.texture_id();let owner:Arc<dyn Any+Send+Sync>=asset.clone();consider(id,AssetOwner::Pixels(asset),owner);});
        resources.for_each_retained::<TextureImage>(|asset|consider(asset.texture_id,AssetOwner::Texture(asset.clone()),asset));result
    }
    pub fn registered(&mut self,id:TextureId,owner:&Arc<dyn Any+Send+Sync>){self.owners.insert(id,Arc::downgrade(owner));}
    pub fn forget(&mut self,id:TextureId){self.owners.remove(&id);}
}
