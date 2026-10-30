use cove_core::DrawList;
use crate::rhi::{RhiError,RhiResult};
pub(super) struct NativeUIGeometry{snapshot:DrawList,pub relies_on_clip:Vec<bool>}
impl NativeUIGeometry{
    pub fn new(list:&DrawList)->RhiResult<Self>{
        // SAFETY: slices are initialized contiguous storage; the C validator bounds-checks indices before any dereference.
        let valid=unsafe{cove_ui_validate_geometry(list.vertices().as_ptr(),list.vertices().len(),list.indices().as_ptr(),list.indices().len())}!=0;
        if !valid{return Err(RhiError::invalid_argument("invalid UI vertices or indices"));}
        let mut relies_on_clip=Vec::with_capacity(list.batches().len());
        for batch in list.batches(){let start=batch.index_offset as usize;let count=batch.index_count as usize;if start>list.indices().len()||count>list.indices().len()-start||count%3!=0{return Err(RhiError::invalid_argument("UI batch index range exceeds the draw list"));}
            let relies=if let Some(rect)=batch.scissor {
                // SAFETY: all indices and this exact range were validated above; C borrows read-only storage.
                // Parenthesised: a block-like expression in statement position cannot take a binary operator.
                (unsafe{cove_ui_relies_on_clip(list.vertices().as_ptr(),list.indices()[start..start+count].as_ptr(),count,rect.x-0.01,rect.y-0.01,rect.max_x()+0.01,rect.max_y()+0.01)})!=0
            }else{false};relies_on_clip.push(relies);}

        let mut snapshot=list.clone();snapshot.resources.reset();Ok(Self{snapshot,relies_on_clip})
    }
    pub fn matches(&self,list:&DrawList)->bool{self.snapshot.geometry_shares_storage(list)}
}

unsafe extern "C"{
    fn cove_ui_validate_geometry(vertices:*const cove_core::Vertex,vertex_count:usize,indices:*const u32,index_count:usize)->i32;
    fn cove_ui_relies_on_clip(vertices:*const cove_core::Vertex,indices:*const u32,count:usize,x:f32,y:f32,r:f32,b:f32)->i32;
}
