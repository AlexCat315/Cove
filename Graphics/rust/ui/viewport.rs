use crate::rhi::{RhiError,RhiResult,ScissorRect};
use cove_core::Rect;
#[derive(Clone,Copy,Debug,PartialEq)]
pub struct NativeUIViewport{pub pixels:[usize;2],pub logical:[f32;2]}
impl NativeUIViewport{
    pub fn new(pixels:[usize;2],logical:[f32;2])->Self{Self{pixels,logical}}
    pub fn validate(self)->RhiResult<()>{if self.pixels.iter().all(|&n|n>0&&n<=i32::MAX as usize)&&self.logical.iter().all(|n|n.is_finite()&&*n>0.0){Ok(())}else{Err(RhiError::invalid_argument("invalid UI viewport dimensions"))}}
    pub fn scissor(self,rect:Option<Rect>)->RhiResult<Option<ScissorRect>>{
        let Some(rect)=rect else{return Ok(Some(ScissorRect::new(self.pixels[0] as i64,self.pixels[1] as i64)));};
        if ![rect.x,rect.y,rect.max_x(),rect.max_y()].iter().all(|n|n.is_finite()){return Err(RhiError::invalid_argument("non-finite UI scissor"));}
        if rect.width<=0.0||rect.height<=0.0{return Ok(None);}
        let bound=|n:f32,axis:usize,upper:bool|{let scaled=n*(self.pixels[axis] as f32/self.logical[axis]);let value=if scaled.is_finite(){scaled as f64}else{n as f64*self.pixels[axis] as f64/self.logical[axis] as f64};let rounded=if upper{value.ceil()}else{value.floor()};rounded.clamp(0.0,self.pixels[axis] as f64) as i64};
        let(x,y,r,b)=(bound(rect.x,0,false),bound(rect.y,1,false),bound(rect.max_x(),0,true),bound(rect.max_y(),1,true));
        Ok((r>x&&b>y).then_some(ScissorRect::at(x,y,r-x,b-y)))
    }
}
