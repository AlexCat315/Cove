use crate::rhi::{GraphicsApi,ShaderArtifact,ShaderStage,RhiError,RhiResult};
pub(super) fn artifact(api:GraphicsApi,name:&str,stage:ShaderStage)->RhiResult<ShaderArtifact>{
    let backend=match api{GraphicsApi::Metal=>"metal",GraphicsApi::Vulkan=>"spirv",GraphicsApi::Dx12=>"dxil"};
    let stage=match stage{ShaderStage::Vertex=>"vertex",ShaderStage::Fragment=>"fragment",_=>return Err(RhiError::unsupported_feature("UI shader stage"))};
    let source=ShaderArtifact::from_json(bytes(backend,name,stage)?)?;
    if api==GraphicsApi::Metal{let platform=if cfg!(target_os="macos"){Some("metal-macos")}else if cfg!(all(target_os="ios",target_abi="sim")){Some("metal-ios-simulator")}else if cfg!(target_os="ios"){Some("metal-ios")}else{None};
        if let Some(platform)=platform{return ShaderArtifact::prefer_library(source,ShaderArtifact::from_json(bytes(platform,name,stage)?)?);}}
    Ok(source)
}
fn bytes(backend:&str,name:&str,stage:&str)->RhiResult<&'static [u8]>{Ok(match(backend,name,stage){
    ("metal","ui","vertex")=>include_bytes!("../../Sources/CoveUIGraphics/Resources/Shaders/Native/metal/ui.vertex.json"),
    ("metal","ui","fragment")=>include_bytes!("../../Sources/CoveUIGraphics/Resources/Shaders/Native/metal/ui.fragment.json"),
    ("metal","ui_backdrop","vertex")=>include_bytes!("../../Sources/CoveUIGraphics/Resources/Shaders/Native/metal/ui_backdrop.vertex.json"),
    ("metal","ui_backdrop","fragment")=>include_bytes!("../../Sources/CoveUIGraphics/Resources/Shaders/Native/metal/ui_backdrop.fragment.json"),
    ("metal-macos","ui","vertex")=>include_bytes!("../../Sources/CoveUIGraphics/Resources/Shaders/Native/metal-macos/ui.vertex.json"),
    ("metal-macos","ui","fragment")=>include_bytes!("../../Sources/CoveUIGraphics/Resources/Shaders/Native/metal-macos/ui.fragment.json"),
    ("metal-macos","ui_backdrop","vertex")=>include_bytes!("../../Sources/CoveUIGraphics/Resources/Shaders/Native/metal-macos/ui_backdrop.vertex.json"),
    ("metal-macos","ui_backdrop","fragment")=>include_bytes!("../../Sources/CoveUIGraphics/Resources/Shaders/Native/metal-macos/ui_backdrop.fragment.json"),
    ("metal-ios","ui","vertex")=>include_bytes!("../../Sources/CoveUIGraphics/Resources/Shaders/Native/metal-ios/ui.vertex.json"),
    ("metal-ios","ui","fragment")=>include_bytes!("../../Sources/CoveUIGraphics/Resources/Shaders/Native/metal-ios/ui.fragment.json"),
    ("metal-ios","ui_backdrop","vertex")=>include_bytes!("../../Sources/CoveUIGraphics/Resources/Shaders/Native/metal-ios/ui_backdrop.vertex.json"),
    ("metal-ios","ui_backdrop","fragment")=>include_bytes!("../../Sources/CoveUIGraphics/Resources/Shaders/Native/metal-ios/ui_backdrop.fragment.json"),
    ("metal-ios-simulator","ui","vertex")=>include_bytes!("../../Sources/CoveUIGraphics/Resources/Shaders/Native/metal-ios-simulator/ui.vertex.json"),
    ("metal-ios-simulator","ui","fragment")=>include_bytes!("../../Sources/CoveUIGraphics/Resources/Shaders/Native/metal-ios-simulator/ui.fragment.json"),
    ("metal-ios-simulator","ui_backdrop","vertex")=>include_bytes!("../../Sources/CoveUIGraphics/Resources/Shaders/Native/metal-ios-simulator/ui_backdrop.vertex.json"),
    ("metal-ios-simulator","ui_backdrop","fragment")=>include_bytes!("../../Sources/CoveUIGraphics/Resources/Shaders/Native/metal-ios-simulator/ui_backdrop.fragment.json"),
    ("spirv","ui","vertex")=>include_bytes!("../../Sources/CoveUIGraphics/Resources/Shaders/Native/spirv/ui.vertex.json"),
    ("spirv","ui","fragment")=>include_bytes!("../../Sources/CoveUIGraphics/Resources/Shaders/Native/spirv/ui.fragment.json"),
    ("spirv","ui_backdrop","vertex")=>include_bytes!("../../Sources/CoveUIGraphics/Resources/Shaders/Native/spirv/ui_backdrop.vertex.json"),
    ("spirv","ui_backdrop","fragment")=>include_bytes!("../../Sources/CoveUIGraphics/Resources/Shaders/Native/spirv/ui_backdrop.fragment.json"),
    ("dxil","ui","vertex")=>include_bytes!("../../Sources/CoveUIGraphics/Resources/Shaders/Native/dxil/ui.vertex.json"),
    ("dxil","ui","fragment")=>include_bytes!("../../Sources/CoveUIGraphics/Resources/Shaders/Native/dxil/ui.fragment.json"),
    ("dxil","ui_backdrop","vertex")=>include_bytes!("../../Sources/CoveUIGraphics/Resources/Shaders/Native/dxil/ui_backdrop.vertex.json"),
    ("dxil","ui_backdrop","fragment")=>include_bytes!("../../Sources/CoveUIGraphics/Resources/Shaders/Native/dxil/ui_backdrop.fragment.json"),
    _=>return Err(RhiError::unsupported_feature("missing offline UI shader")),
})}
