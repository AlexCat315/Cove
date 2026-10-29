use cove_core::DrawList;
use crate::rhi::{RhiError,RhiResult};
pub(super) struct NativeUIGeometry{snapshot:DrawList,pub relies_on_clip:Vec<bool>}
impl NativeUIGeometry{
    pub fn new(list:&DrawList)->RhiResult<Self>{
        let valid=list.vertices().iter().all(|v|v.x.is_finite()&&v.y.is_finite()&&v.u.is_finite()&&v.v.is_finite())&&list.indices().iter().all(|&i|(i as usize)<list.vertices().len());
        if !valid{return Err(RhiError::invalid_argument("invalid UI vertices or indices"));}
        let mut relies_on_clip=Vec::with_capacity(list.batches().len());
        for batch in list.batches(){let start=batch.index_offset as usize;let count=batch.index_count as usize;if start>list.indices().len()||count>list.indices().len()-start||count%3!=0{return Err(RhiError::invalid_argument("UI batch index range exceeds the draw list"));}
            relies_on_clip.push(batch.scissor.is_some_and(|rect|list.indices()[start..start+count].iter().any(|&i|{let v=list.vertices()[i as usize];v.x<rect.x-0.01||v.y<rect.y-0.01||v.x>rect.max_x()+0.01||v.y>rect.max_y()+0.01})));}
        let mut snapshot=list.clone();snapshot.resources.reset();Ok(Self{snapshot,relies_on_clip})
    }
    pub fn matches(&self,list:&DrawList)->bool{self.snapshot.geometry_shares_storage(list)}
}
