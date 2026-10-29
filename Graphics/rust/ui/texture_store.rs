use std::{collections::HashMap,sync::{Arc,Mutex}};
use cove_core::{DrawList,DrawListResources,TextureId,Vertex};
use crate::rhi::*;
use super::{asset_residency::{UIAssetResidency,AssetOwner},shader_resources::NativeUIShaderResources,texture_image::NativeTextureImageBacking};
#[derive(Clone)]
pub(super) struct TexturePatch{pub sequence:u64,pub region:TextureUploadRegion,pub pixels:Arc<[u8]>}
struct SlotState{binding:Option<BindingSet>,patches:Vec<TexturePatch>,sequence:u64}
pub(super) struct TextureSlot{pub resource:Arc<TextureResource>,pub accepts_uploads:bool,state:Mutex<SlotState>}
impl TextureSlot{
    fn new(resource:Arc<TextureResource>,accepts_uploads:bool)->Arc<Self>{Arc::new(Self{resource,accepts_uploads,state:Mutex::new(SlotState{binding:None,patches:Vec::new(),sequence:0})})}
    fn stage(&self,pixels:Arc<[u8]>,region:TextureUploadRegion){let mut state=self.state.lock().unwrap_or_else(|e|e.into_inner());state.sequence=state.sequence.checked_add(1).expect("texture patch sequence exhausted");let sequence=state.sequence;state.patches.push(TexturePatch{sequence,region,pixels});}
    pub fn snapshot(&self)->Vec<TexturePatch>{self.state.lock().unwrap_or_else(|e|e.into_inner()).patches.clone()}
    pub fn commit(&self,sequence:u64){self.state.lock().unwrap_or_else(|e|e.into_inner()).patches.retain(|patch|patch.sequence>sequence);}
    fn binding(&self,shaders:&NativeUIShaderResources)->RhiResult<BindingSet>{let mut state=self.state.lock().unwrap_or_else(|e|e.into_inner());if let Some(binding)=state.binding{return Ok(binding);}let binding=shaders.binding(self.resource.texture())?;state.binding=Some(binding);Ok(binding)}
}
#[derive(Clone,Copy)]
struct Placement{atlas:usize,x:usize,y:usize,w:usize,h:usize}
#[derive(Clone)]
struct ImageAtlas{texture:Arc<TextureResource>,binding:Option<BindingSet>,size:usize,x:usize,y:usize,shelf:usize,free:Vec<Placement>}
#[derive(Clone)]
struct AtlasUpload{sequence:u64,pixels:Arc<[u8]>,region:TextureUploadRegion,logical_bytes:usize,atlas:usize}
pub(super) struct NativeUITextureStore{pub device:Arc<Device>,pub shaders:Arc<NativeUIShaderResources>,pub fallback:Arc<TextureSlot>,pub textures:HashMap<TextureId,Arc<TextureSlot>>,residency:UIAssetResidency,atlases:Vec<ImageAtlas>,placements:HashMap<TextureId,Placement>,pending:Vec<AtlasUpload>,sequence:u64}
impl NativeUITextureStore{
    const ATLAS_SIZE:usize=2048;const MAX_ATLAS_DIMENSION:usize=512;
    pub fn new(device:Arc<Device>,shaders:Arc<NativeUIShaderResources>)->RhiResult<Self>{
        let mut d=TextureDescriptor::new(1,1,TextureFormat::Rgba8Unorm,TextureUsage::SAMPLED|TextureUsage::TRANSFER_DESTINATION);d.label=Some("native-ui-white");let fallback=TextureSlot::new(Arc::new(TextureResource::new(device.clone(),d)?),true);fallback.stage(Arc::from([255u8;4]),TextureUploadRegion::new(1,1));
        Ok(Self{device,shaders,fallback,textures:HashMap::new(),residency:Default::default(),atlases:Vec::new(),placements:HashMap::new(),pending:Vec::new(),sequence:0})
    }
    pub fn stage(&mut self,id:TextureId,pixels:Arc<[u8]>,size:[usize;2],region:TextureUploadRegion,format:TextureFormat)->RhiResult<()>{
        let channels=match format{TextureFormat::R8Unorm=>1,TextureFormat::Rgba8Unorm=>4,_=>return Err(RhiError::invalid_argument("invalid UI texture format"))};
        let w=usize::try_from(region.width).ok();let h=usize::try_from(region.height).ok();let x=usize::try_from(region.origin[0]).ok();let y=usize::try_from(region.origin[1]).ok();
        let valid=(||{let(w,h,x,y)=(w?,h?,x?,y?);Some(id!=0&&size[0]>0&&size[1]>0&&w>0&&h>0&&x<=size[0]&&y<=size[1]&&w<=size[0]-x&&h<=size[1]-y&&w.checked_mul(h)?.checked_mul(channels)?==pixels.len())})().unwrap_or(false);
        if !valid{return Err(RhiError::invalid_argument("invalid UI texture patch"));}
        let slot=match self.textures.get(&id){Some(slot)if slot.accepts_uploads&&slot.resource.descriptor().width==size[0]&&slot.resource.descriptor().height==size[1]&&slot.resource.descriptor().format==format=>slot.clone(),_=>{
            let mut d=TextureDescriptor::new(size[0],size[1],format,TextureUsage::SAMPLED|TextureUsage::TRANSFER_DESTINATION|TextureUsage::TRANSFER_SOURCE);d.label=Some("native-ui-texture");let slot=TextureSlot::new(Arc::new(TextureResource::new(self.device.clone(),d)?),true);
            if region.origin!=[0,0]||region.width!=size[0] as i64||region.height!=size[1] as i64{let bytes=size[0].checked_mul(size[1]).and_then(|n|n.checked_mul(channels)).ok_or_else(||RhiError::invalid_argument("UI texture dimensions overflow"))?;slot.stage(vec![0;bytes].into(),TextureUploadRegion::new(size[0] as i64,size[1] as i64));}slot}};
        slot.stage(pixels,region);self.textures.insert(id,slot);Ok(())
    }
    pub fn register(&mut self,id:TextureId,resource:Arc<TextureResource>)->RhiResult<()>{let d=resource.descriptor();
        if id==0||!Arc::ptr_eq(resource.device(),&self.device)||d.dimension!=TextureDimension::Texture2D||d.sample_count!=1||!d.usage.contains(TextureUsage::SAMPLED)||!matches!(d.format,TextureFormat::Rgba8Unorm|TextureFormat::Bgra8Unorm){return Err(RhiError::invalid_argument("external UI textures require encoded RGBA/BGRA on the same device"));}
        if self.textures.get(&id).is_some_and(|slot|Arc::ptr_eq(&slot.resource,&resource)){return Ok(());}self.textures.insert(id,TextureSlot::new(resource,false));Ok(())
    }
    pub fn prepare_assets(&mut self,resources:&DrawListResources)->RhiResult<()>{
        for id in self.residency.expired(){self.unregister(id);}
        for(id,asset,owner)in self.residency.pending(resources){match asset{
            AssetOwner::Pixels(asset)=>{let image=asset.image();let(w,h)=(image.width() as usize,image.height() as usize);
                if w<=Self::MAX_ATLAS_DIMENSION&&h<=Self::MAX_ATLAS_DIMENSION&&!self.placements.contains_key(&id){let placement=self.place(w,h,image.pixels())?;self.placements.insert(id,placement);}
                else{self.stage(id,image.pixels().to_vec().into(),[w,h],TextureUploadRegion::new(w as i64,h as i64),TextureFormat::Rgba8Unorm)?;}},
            AssetOwner::Texture(asset)=>{let backing:Arc<dyn std::any::Any+Send+Sync>=asset.backing.clone();let backing=backing.downcast::<NativeTextureImageBacking>().map_err(|_|RhiError::invalid_argument("texture image belongs to another graphics backend"))?;self.register(id,backing.resource.clone())?;}}
            self.residency.registered(id,&owner);
        }Ok(())
    }
    pub fn unregister(&mut self,id:TextureId){if let Some(placement)=self.placements.remove(&id){if let Some(atlas)=self.atlases.get_mut(placement.atlas){atlas.free.push(placement);}}self.textures.remove(&id);self.residency.forget(id);}
    fn place(&mut self,w:usize,h:usize,pixels:&[u8])->RhiResult<Placement>{
        let mut placement=None;
        for atlas in &mut self.atlases{if let Some(index)=atlas.free.iter().position(|p|p.w==w&&p.h==h){placement=Some(atlas.free.swap_remove(index));break;}}
        if placement.is_none(){for(index,atlas)in self.atlases.iter_mut().enumerate(){if let Some((x,y))=allocate(atlas,w,h){placement=Some(Placement{atlas:index,x,y,w,h});break;}}}
        let placement=if let Some(value)=placement{value}else{
            let mut d=TextureDescriptor::new(Self::ATLAS_SIZE,Self::ATLAS_SIZE,TextureFormat::Rgba8Unorm,TextureUsage::SAMPLED|TextureUsage::TRANSFER_DESTINATION|TextureUsage::TRANSFER_SOURCE);d.label=Some("native-ui-atlas");
            let mut atlas=ImageAtlas{texture:Arc::new(TextureResource::new(self.device.clone(),d)?),binding:None,size:Self::ATLAS_SIZE,x:0,y:0,shelf:0,free:Vec::new()};let(x,y)=allocate(&mut atlas,w,h).ok_or_else(||RhiError::invalid_argument("image does not fit in UI atlas"))?;let index=self.atlases.len();self.atlases.push(atlas);Placement{atlas:index,x,y,w,h}};
        self.sequence=self.sequence.checked_add(1).ok_or_else(||RhiError::invalid_argument("atlas upload sequence exhausted"))?;
        let mut padded=vec![0;(w+2)*(h+2)*4];
        // SAFETY: image dimensions and RGBA length come from validated Bitmap; destination includes the exact one-pixel border.
        unsafe{cove_ui_pad_rgba(pixels.as_ptr(),w,h,padded.as_mut_ptr());}
        self.pending.push(AtlasUpload{sequence:self.sequence,pixels:padded.into(),region:TextureUploadRegion::with_origin((w+2) as i64,(h+2) as i64,placement.x as i64,placement.y as i64),logical_bytes:w*h*4,atlas:placement.atlas});Ok(placement)
    }
    pub fn has_atlased_placements(&self)->bool{!self.placements.is_empty()}
    pub fn remap_atlas_uvs(&self,list:&DrawList)->Vec<Vertex>{
        let mut vertices=list.vertices().to_vec();for batch in list.batches(){let Some(p)=self.placements.get(&batch.texture_id)else{continue;};let size=self.atlases[p.atlas].size as f32;
            for &index in &list.indices()[batch.index_offset as usize..(batch.index_offset+batch.index_count) as usize]{let mut v=list.vertices()[index as usize];let base=if v.u>=15.0{20.0}else if v.u>=5.0{10.0}else{continue;};v.u=base+((p.x+1) as f32+(v.u-base)*p.w as f32)/size;v.v=((p.y+1) as f32+v.v*p.h as f32)/size;vertices[index as usize]=v;}}
        vertices
    }
    pub fn build_atlas_uploads(&self)->RhiResult<(Vec<TextureBufferUpload>,usize,u64)>{let(mut result,mut bytes,mut sequence)=(Vec::new(),0,0);for upload in &self.pending{let location=self.device.upload_transient(&upload.pixels,4)?;let mut command=TextureBufferUpload::new(location.buffer,upload.region.width as usize*4,self.atlases[upload.atlas].texture.texture(),upload.region);command.offset=location.offset;result.push(command);bytes+=upload.logical_bytes;sequence=upload.sequence;}Ok((result,bytes,sequence))}
    pub fn commit_atlas_uploads(&mut self,sequence:u64){self.pending.retain(|upload|upload.sequence>sequence);}
    pub fn binding_set(&mut self,id:TextureId)->RhiResult<BindingSet>{if let Some(p)=self.placements.get(&id){let atlas=&mut self.atlases[p.atlas];if let Some(set)=atlas.binding{return Ok(set);}let set=self.shaders.binding(atlas.texture.texture())?;atlas.binding=Some(set);return Ok(set);}self.textures.get(&id).unwrap_or(&self.fallback).binding(&self.shaders)}
    pub fn synchronize(&mut self,source:&Self)->RhiResult<()>{if !Arc::ptr_eq(&self.device,&source.device){return Err(RhiError::invalid_argument("UI texture synchronization requires the same device"));}self.textures=source.textures.clone();self.residency=source.residency.clone();self.atlases=source.atlases.clone();self.placements=source.placements.clone();self.pending=source.pending.clone();self.sequence=source.sequence;Ok(())}
}
fn allocate(atlas:&mut ImageAtlas,w:usize,h:usize)->Option<(usize,usize)>{let(pw,ph)=(w+2,h+2);if pw>atlas.size||ph>atlas.size{return None;}if atlas.x+pw>atlas.size{atlas.x=0;atlas.y+=atlas.shelf;atlas.shelf=0;}if atlas.y+ph>atlas.size{return None;}let result=(atlas.x,atlas.y);atlas.x+=pw;atlas.shelf=atlas.shelf.max(ph);Some(result)}
unsafe extern "C"{fn cove_ui_pad_rgba(source:*const u8,width:usize,height:usize,destination:*mut u8);}
