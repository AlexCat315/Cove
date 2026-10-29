//! ShaderArtifactLoading.swift: source fingerprints guard platform Metal libraries.
use super::{ShaderArtifact, ShaderFormat, ShaderStage, GraphicsApi, RhiError, RhiResult};
use std::path::Path;
use base64::Engine;
use sha2::{Digest, Sha256};
impl ShaderArtifact {
    /// Reads both Rust artifacts and the existing offline Swift JSON resources.
    pub fn from_json(bytes:&[u8])->RhiResult<Self>{
        fn snake(key:&str)->String{let mut out=String::new();for c in key.chars(){if c.is_uppercase(){out.push('_');out.extend(c.to_lowercase());}else{out.push(c);}}out}
        fn normalize(value:serde_json::Value)->serde_json::Value{match value{
            serde_json::Value::Object(map)=>serde_json::Value::Object(map.into_iter().map(|(key,value)|(snake(&key),normalize(value))).collect()),
            serde_json::Value::Array(values)=>serde_json::Value::Array(values.into_iter().map(normalize).collect()),other=>other}}
        let mut value=normalize(serde_json::from_slice(bytes).map_err(|e|RhiError::layout_mismatch(format!("invalid shader JSON: {e}")))?);
        if let Some(code)=value.get("code").and_then(|v|v.as_str()){
            let decoded=base64::engine::general_purpose::STANDARD.decode(code).map_err(|e|RhiError::layout_mismatch(format!("invalid shader bytecode: {e}")))?;
            value["code"]=serde_json::Value::Array(decoded.into_iter().map(|b|serde_json::Value::from(b)).collect());
        }
        serde_json::from_value(value).map_err(|e|RhiError::layout_mismatch(format!("invalid shader interface: {e}")))
    }
    pub fn load_directory(root:&Path,name:&str,stage:ShaderStage,api:GraphicsApi,directory:&Path,target:Option<&str>)->RhiResult<Self>{
        let backend=target.unwrap_or(match api{GraphicsApi::Metal=>"metal",GraphicsApi::Vulkan=>"spirv",GraphicsApi::Dx12=>"dxil"});
        let stage_name=match stage{ShaderStage::Vertex=>"vertex",ShaderStage::Fragment=>"fragment",ShaderStage::Compute=>"compute",ShaderStage::Task=>"task",ShaderStage::Mesh=>"mesh"};
        let file=format!("{name}.{stage_name}.json");let path=root.join(directory).join(backend).join(&file);
        let bytes=std::fs::read(&path).map_err(|e|RhiError::unsupported_feature(format!("missing offline shader {}: {e}",path.display())))?;
        let source=Self::from_json(&bytes)?;if source.stage!=stage{return Err(RhiError::layout_mismatch("shader artifact stage mismatch"));}
        if api==GraphicsApi::Metal { if let Some(platform)=metal_library_platform(){let path=root.join(directory).join(format!("metal-{platform}")).join(file);if path.is_file(){let library=Self::from_json(&std::fs::read(path).map_err(|e|RhiError::invalid_argument(e.to_string()))?)?;return Self::prefer_library(source,library);}} }
        Ok(source)
    }
    pub(crate) fn prefer_library(source:Self,mut library:Self)->RhiResult<Self>{
        let fingerprint=format!("{:x}",Sha256::digest(&source.code));
        if library.source_fingerprint.as_deref()!=Some(&fingerprint){return Ok(source);}
        if library.format!=ShaderFormat::Metallib||library.stage!=source.stage||library.entry_point!=source.entry_point{return Err(RhiError::layout_mismatch("precompiled Metal library ABI mismatch"));}
        library.interface=source.interface;Ok(library)
    }
}
pub(crate) fn metal_library_platform()->Option<&'static str>{
    #[cfg(target_os="macos")] {return Some("macos");}
    #[cfg(all(target_os="ios",target_abi="sim"))] {return Some("ios-simulator");}
    #[cfg(all(target_os="ios",not(target_abi="sim")))] {return Some("ios");}
    #[cfg(not(any(target_os="macos",target_os="ios")))] {None}
}
