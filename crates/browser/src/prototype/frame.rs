//! Translation of the `Frame`, `Label` and `Batch` codable payloads from
//! `Browser/Sources/CoveUIBrowserPrototype/main.swift`.
//!
//! JavaScript reads these snapshots, so field names and JSON shape are the
//! wire contract and must stay identical to the Swift originals. Swift uses
//! `JSONEncoder`'s default camelCase key strategy; the explicit `serde`
//! renames below record where the Rust field name differs from the wire name.

use serde::Serialize;

/// One shaped text run published with the frame snapshot.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Label {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub size: f32,
    pub color: String,
    pub width: f32,
    pub glyphs: Vec<u32>,
    pub clusters: Vec<u32>,
    #[serde(rename = "fontIDs")]
    pub font_ids: Vec<i64>,
}

/// One draw batch; `offset`/`count` are indices into the shared index buffer.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Batch {
    pub offset: u32,
    pub count: u32,
    #[serde(rename = "textureID")]
    pub texture_id: u32,
    pub clip: Option<[f32; 4]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct RasterAsset {
    pub id: u32,
    pub width: u32,
    pub height: u32,
}

/// The complete per-frame snapshot returned to JavaScript.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Frame {
    pub count: i64,
    pub dark: bool,
    pub note: String,
    pub note_width: f32,
    pub focused: Option<String>,
    pub width: f32,
    pub height: f32,
    pub vertices: usize,
    pub indices: usize,
    pub batches: Vec<Batch>,
    pub raster_assets: Vec<RasterAsset>,
    pub labels: Vec<Label>,
    pub tree: cove_devtools_protocol::TreeSnapshot,
    pub font_count: usize,
    pub atlas_width: usize,
    pub atlas_height: usize,
    pub atlas_update: Option<[usize; 4]>,
    pub atlas_full: bool,
}

/// Clamps the requested viewport exactly like the Swift renderer.
pub fn viewport_size(raw_width: f32, raw_height: f32) -> (f32, f32) {
    let clamp = |value: f32, minimum: f32, fallback: f32| {
        if value.is_finite() {
            value.max(minimum).min(4096.0)
        } else {
            fallback
        }
    };
    (
        clamp(raw_width, 280.0, 640.0),
        clamp(raw_height, 360.0, 460.0),
    )
}
