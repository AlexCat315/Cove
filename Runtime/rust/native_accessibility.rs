use std::{cell::{Cell,RefCell},rc::{Rc,Weak}};
use cove_core::scene::*;
use crate::{Sdl3PlatformHost,PlatformWindowSession};
pub struct NativeAccessibility;
impl NativeAccessibility{
    pub fn synchronize(host:&Sdl3PlatformHost,session:&Rc<PlatformWindowSession>,root:&Node,timeline:&RefCell<PerformanceTimeline>){
        #[cfg(any(target_os="macos",target_os="ios"))]
        {let native=match host.native_window_reference(session.id){Some(cove_shell::NativeWindowReference::AppKit(raw))|Some(cove_shell::NativeWindowReference::UIKit(raw))=>raw,_=>return};
            let bridge=if let Some(bridge)=root.first_resource::<AppleAccessibilityBridge>(){bridge}else{let bridge=AppleAccessibilityBridge::new(native);root.add_resource(bridge.clone());bridge};bridge.update(session,root,timeline);}
        #[cfg(not(any(target_os="macos",target_os="ios")))]
        {let _=(host,session,root,timeline);}
    }
}
#[cfg(any(target_os="macos",target_os="ios"))]
mod apple{
use super::*;
use std::{any::Any,collections::HashMap,ffi::{CString,CStr,c_char,c_void}};
#[repr(C)]
struct Item{id:u64,parent:u64,role:u32,flags:u32,actions:u32,x:f32,y:f32,width:f32,height:f32,label:*const c_char,value:*const c_char,help:*const c_char,identifier:*const c_char,revision:u64}
unsafe extern "C"{
    fn cove_runtime_ax_create(window:usize,context:*mut c_void,action:unsafe extern "C" fn(*mut c_void,u64,u32,*const c_char)->bool,value:unsafe extern "C" fn(*mut c_void,u64)->*mut c_char,free_value:unsafe extern "C" fn(*mut c_char))->*mut c_void;
    fn cove_runtime_ax_destroy(bridge:*mut c_void);
    fn cove_runtime_ax_signature(bridge:*mut c_void,out:*mut f64);
    fn cove_runtime_ax_update(bridge:*mut c_void,items:*const Item,count:usize)->usize;
}
#[derive(Clone,PartialEq)]
struct Signature{roots:Vec<ElementId>,focused:Option<ElementId>,frame:cove_core::Rect,native:[f64;8]}
pub(super) struct AppleAccessibilityBridge{pointer:Cell<*mut c_void>,context:Cell<*mut c_void>,nodes:RefCell<HashMap<u64,NodeWeak>>,session:RefCell<Weak<PlatformWindowSession>>,last:RefCell<Option<Signature>>,structure:Cell<u64>,geometry:Cell<u64>,last_time:Cell<f64>,pub updated_element_count:Cell<usize>}
impl AppleAccessibilityBridge{
    pub fn new(window:usize)->Rc<Self>{let bridge=Rc::new(Self{pointer:Cell::new(std::ptr::null_mut()),context:Cell::new(std::ptr::null_mut()),nodes:RefCell::new(HashMap::new()),session:RefCell::new(Weak::new()),last:RefCell::new(None),structure:Cell::new(u64::MAX),geometry:Cell::new(u64::MAX),last_time:Cell::new(0.0),updated_element_count:Cell::new(0)});
        unsafe extern "C" fn action(context:*mut c_void,id:u64,kind:u32,value:*const c_char)->bool{
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(||{ // SAFETY: bridge owns a boxed Weak until native callbacks are invalidated.
                let bridge=unsafe{&*context.cast::<Weak<AppleAccessibilityBridge>>()}.upgrade();let Some(bridge)=bridge else{return false;};let session=bridge.session.borrow().upgrade();let Some(session)=session else{return false;};let weak=bridge.nodes.borrow().get(&id).cloned();let Some(node)=weak.as_ref().and_then(Node::upgrade)else{return false;};
                if !node.accepts_subtree_input()||!node.accessibility().is_some_and(|s|s.state.is_enabled)||!session.input_context.focus_chain.permits_input(&node){return false;}
                let actions=node.accessibility_actions();let handled=session.with_current(||match kind{1=>{if let Some(f)=actions.activate{f();true}else{false}},2=>{if let Some(f)=actions.set_value{if value.is_null(){return false;} // SAFETY: native setter passes a NUL terminated UTF-8 value for this synchronous callback.
                    let text=unsafe{CStr::from_ptr(value)}.to_string_lossy();f(&text);true}else{false}},3=>{if let Some(f)=actions.increment{f();true}else{false}},4=>{if let Some(f)=actions.decrement{f();true}else{false}},5=>{session.input_context.focus_chain.focus(Some(&node),true);true},_=>false});
                if handled{session.with_current(||{session.recomposer.commit_all();});session.request_display();}handled
            })).unwrap_or(false)
        }
        unsafe extern "C" fn value(context:*mut c_void,id:u64)->*mut c_char{std::panic::catch_unwind(std::panic::AssertUnwindSafe(||{ // SAFETY: context remains live while this native element is registered.
            let bridge=unsafe{&*context.cast::<Weak<AppleAccessibilityBridge>>()}.upgrade()?;let weak=bridge.nodes.borrow().get(&id).cloned()?;let node=Node::upgrade(&weak)?;let text=if let Some(provider)=node.accessibility_value_provider(){(provider.read)()}else{let s=node.accessibility()?;if matches!(s.role,AccessibilityRole::StaticText|AccessibilityRole::Status)&&s.value.is_empty(){s.label}else{s.value}};CString::new(text.replace('\0',"")).ok().map(CString::into_raw)
        })).ok().flatten().unwrap_or(std::ptr::null_mut())}
        unsafe extern "C" fn free_value(value:*mut c_char){if !value.is_null(){ // SAFETY: pointer was returned by CString::into_raw in value(), and native frees it once.
            unsafe{drop(CString::from_raw(value));}}}
        let context=Box::into_raw(Box::new(Rc::downgrade(&bridge))).cast();bridge.context.set(context); // SAFETY: boxed weak and callbacks outlive native bridge; window is owned by the session shell.
        bridge.pointer.set(unsafe{cove_runtime_ax_create(window,context,action,value,free_value)});bridge
    }
    pub fn update(&self,session:&Rc<PlatformWindowSession>,root:&Node,timeline:&RefCell<PerformanceTimeline>){
        *self.session.borrow_mut()=Rc::downgrade(session);if self.pointer.get().is_null(){return;}
        let focus=&session.input_context.focus_chain;let roots=if let Some(modal)=focus.modal_root(){let mut roots=vec![modal];roots.extend(focus.accessory_roots());roots}else{vec![root.clone()]};let mut native=[0.0;8]; // SAFETY: native bridge is live; output has exactly eight double slots.
        unsafe{cove_runtime_ax_signature(self.pointer.get(),native.as_mut_ptr())};let signature=Signature{roots:roots.iter().map(Node::id).collect(),focused:focus.focused().map(|n|n.id()),frame:root.frame(),native};let structure=AccessibilityRevision::structure();let geometry=AccessibilityRevision::geometry();let now=cove_shell::monotonic_seconds();let changed=structure!=self.structure.get()||self.last.borrow().as_ref()!=Some(&signature);
        if !changed&&(geometry==self.geometry.get()||now-self.last_time.get()<1.0/30.0){self.updated_element_count.set(0);return;}
        self.structure.set(structure);self.geometry.set(geometry);self.last_time.set(now);*self.last.borrow_mut()=Some(signature);
        let trace=timeline.borrow().begin();let snapshots=roots.iter().flat_map(|node|AccessibilityTree::snapshot_in(node,Some(root.absolute_frame()),false)).collect::<Vec<_>>();timeline.borrow_mut().end(trace,"frame","Accessibility snapshot",None,&[]);
        fn flatten(elements:&[AccessibilityElement],parent:u64,items:&mut Vec<Item>,strings:&mut Vec<CString>,nodes:&mut HashMap<u64,NodeWeak>,focus:&FocusChain){
            for item in elements{let id=item.id().0;nodes.insert(id,item.node.downgrade());let s=&item.semantics;let st=&s.state;let actions=item.node.accessibility_actions();let interactive=item.node.is_focusable()||actions.activate.is_some()||actions.set_value.is_some()||actions.increment.is_some()||actions.decrement.is_some();let enabled=st.is_enabled&&(!interactive||item.accepts_input);
                let flags=u32::from(enabled)|u32::from(st.is_selected)<<1|u32::from(st.is_expanded.is_some())<<2|u32::from(st.is_expanded==Some(true))<<3|u32::from(st.is_required)<<4|u32::from(st.is_invalid)<<5|u32::from(st.is_read_only)<<6|u32::from(st.is_secure)<<7|u32::from(focus.focused().is_some_and(|n|n.id()==item.id()))<<8|u32::from(item.accepts_input)<<9|u32::from(item.node.is_focusable())<<10;
                let action_flags=u32::from(actions.activate.is_some())|u32::from(actions.set_value.is_some())<<1|u32::from(actions.increment.is_some())<<2|u32::from(actions.decrement.is_some())<<3|u32::from(item.node.is_focusable())<<4;
                let mut cstr=|text:&str|{strings.push(CString::new(text.replace('\0',"")).unwrap());strings.last().unwrap().as_ptr()};let static_text=matches!(s.role,AccessibilityRole::StaticText|AccessibilityRole::Status);let label=cstr(if static_text&&s.value.is_empty(){""}else{&s.label});let value=cstr(if static_text&&s.value.is_empty(){&s.label}else{&s.value});let help=cstr(&s.help);let identifier=cstr(if s.identifier.is_empty(){""}else{&s.identifier});
                let revision=item.node.accessibility_value_provider().map_or(0,|p|p.revision);items.push(Item{id,parent,role:role_id(s.role),flags,actions:action_flags,x:item.frame.x,y:item.frame.y,width:item.frame.width,height:item.frame.height,label,value,help,identifier,revision});flatten(&item.children,id,items,strings,nodes,focus);
            }
        }
        let commit=timeline.borrow().begin();let mut items=Vec::new();let mut strings=Vec::new();let mut nodes=HashMap::new();flatten(&snapshots,0,&mut items,&mut strings,&mut nodes,focus);*self.nodes.borrow_mut()=nodes; // SAFETY: native commits copied strings and geometry synchronously; nodes remain weakly registered.
        let updated=unsafe{cove_runtime_ax_update(self.pointer.get(),items.as_ptr(),items.len())};self.updated_element_count.set(updated);timeline.borrow_mut().end(commit,"frame","Accessibility bridge commit",None,&[]);
    }
    fn invalidate(&self){let pointer=self.pointer.replace(std::ptr::null_mut());if !pointer.is_null(){ // SAFETY: native destroy invalidates all callbacks before the weak context is freed.
        unsafe{cove_runtime_ax_destroy(pointer)};}let context=self.context.replace(std::ptr::null_mut());if !context.is_null(){ // SAFETY: context was created by Box::into_raw and is released once, after native invalidation.
        unsafe{drop(Box::from_raw(context.cast::<Weak<AppleAccessibilityBridge>>()));}}self.nodes.borrow_mut().clear();}
}
impl NodeResource for AppleAccessibilityBridge{fn mount(&self,_:&Node){}fn unmount(&self,_:&Node){self.invalidate();}fn as_any_rc(self:Rc<Self>)->Rc<dyn Any>{self}}
impl Drop for AppleAccessibilityBridge{fn drop(&mut self){self.invalidate();}}
fn role_id(role:AccessibilityRole)->u32{use AccessibilityRole::*;match role{Group=>0,Toolbar=>1,Button=>2,StaticText=>3,Heading=>4,TextField=>5,Checkbox=>6,Toggle=>7,Slider=>8,Radio=>9,Tab=>10,List=>11,ListItem=>12,Tree=>13,TreeItem=>14,Table=>15,Row=>16,Cell=>17,Dialog=>18,Status=>19,Progress=>20,Image=>21,Link=>22,ScrollArea=>23,ComboBox=>24,TabList=>25,TabPanel=>26}}
}
#[cfg(any(target_os="macos",target_os="ios"))]
use apple::AppleAccessibilityBridge;
