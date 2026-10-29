use cove_graphics::rhi::{SurfaceDescriptor,NativeSurfaceKind,TextureFormat};
use cove_shell::NativeRenderSurface;
pub struct SurfaceFactory;
impl SurfaceFactory{
    pub fn native_descriptor(native:NativeRenderSurface,size:[u32;2],vsync:bool)->SurfaceDescriptor{
        let mut result=match native{
            NativeRenderSurface::MetalLayer(pointer)=>SurfaceDescriptor::new(pointer).with_size(size[0] as usize,size[1] as usize),
            NativeRenderSurface::Win32Window{hwnd,..}=>{let mut d=SurfaceDescriptor::new(hwnd);d.kind=NativeSurfaceKind::Win32Window;d},
            NativeRenderSurface::WaylandSurface{display,surface}=>{let mut d=SurfaceDescriptor::new(surface);d.kind=NativeSurfaceKind::WaylandSurface;d.display=display;d},
            NativeRenderSurface::XlibWindow{display,window}=>{let mut d=SurfaceDescriptor::new(window as usize);d.kind=NativeSurfaceKind::XlibWindow;d.display=display;d},
            NativeRenderSurface::AndroidWindow(window)=>{let mut d=SurfaceDescriptor::new(window);d.kind=NativeSurfaceKind::AndroidWindow;d}};
        result.width=size[0] as usize;result.height=size[1] as usize;result.color_format=TextureFormat::Bgra8UnormSrgb;result.vsync_enabled=vsync;result
    }
}
