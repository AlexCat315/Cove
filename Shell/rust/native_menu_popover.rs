use crate::{NativeWindowReference, ShellError};
use cove_platform::NativeMenuSnapshot;
pub struct NativeMenuPopover;
impl NativeMenuPopover {
    pub fn is_supported() -> bool { cfg!(target_os="macos") }
    pub fn pop_up(snapshot:&NativeMenuSnapshot,x:f32,y:f32,window:Option<NativeWindowReference>)->Result<Option<i64>,ShellError>{
        #[cfg(target_os="macos")]
        {
            use cove_platform::NativeMenuKind;
            use std::ffi::{CString,c_char};
            #[repr(C)] struct Item{parent:usize,id:i64,kind:u32,flags:u32,title:*const c_char,key:*const c_char,png:*const u8,png_size:usize}
            unsafe extern "C"{fn cove_shell_menu_popup(items:*const Item,count:usize,x:f32,y:f32,window:usize,chosen:*mut i64)->bool;}
            fn append(snapshot:&NativeMenuSnapshot,parent:usize,items:&mut Vec<Item>,strings:&mut Vec<CString>){
                for item in &snapshot.items{
                    strings.push(CString::new(item.title.replace('\0',"")).unwrap());let title=strings.last().unwrap().as_ptr();
                    strings.push(CString::new(item.shortcut.as_ref().map_or("",|s|s.key.as_str()).replace('\0',"")).unwrap());let key=strings.last().unwrap().as_ptr();
                    let mut flags=if item.enabled{1}else{0}|if item.checked{2}else{0};
                    if let Some(s)=&item.shortcut{if s.meta{flags|=4;}if s.ctrl{flags|=8;}if s.alt{flags|=16;}if s.shift{flags|=32;}}
                    if item.icon.as_ref().is_some_and(|i|i.is_template){flags|=64;}
                    let index=items.len();items.push(Item{parent,id:item.id,kind:match &item.kind{NativeMenuKind::Regular=>0,NativeMenuKind::Separator=>1,NativeMenuKind::Submenu(_)=>2},flags,title,key,png:item.icon.as_ref().map_or(std::ptr::null(),|i|i.png.as_ptr()),png_size:item.icon.as_ref().map_or(0,|i|i.png.len())});
                    if let NativeMenuKind::Submenu(branch)=&item.kind{append(branch,index,items,strings);}
                }
            }
            let mut items=Vec::new();let mut strings=Vec::new();append(snapshot,usize::MAX,&mut items,&mut strings);
            let window=match window{Some(NativeWindowReference::AppKit(raw))=>raw,_=>0};let mut chosen=0;
            // SAFETY: native popup runs synchronously; all C strings, icons and window references remain valid throughout.
            Ok(unsafe{cove_shell_menu_popup(items.as_ptr(),items.len(),x,y,window,&mut chosen)}.then_some(chosen))
        }
        #[cfg(not(target_os="macos"))]
        {let _=(snapshot,x,y,window);Err(ShellError::UnsupportedPlatform("native context-menu snapshots require AppKit; use the Compose menu presenter on this platform".into()))}
    }
}
