//! Container and stream descriptions reported by probing and decoding.
use crate::{AudioFormat, MediaTime, TrackKind};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MediaInfo {
    /// Demuxer short names, for example `mov,mp4,m4a,3gp,3g2,mj2`.
    pub container: String,
    pub duration: Option<MediaTime>,
    pub bit_rate: Option<u64>,
    /// Whether the source can reposition (files and byte ranges, not live streams).
    pub seekable: bool,
    /// Container tags such as `title`, `artist`, `album`, keys lower-cased.
    pub metadata: BTreeMap<String, String>,
    pub streams: Vec<StreamInfo>,
}

impl MediaInfo {
    pub fn title(&self) -> Option<&str> {
        self.metadata.get("title").map(String::as_str)
    }

    pub fn artist(&self) -> Option<&str> {
        self.metadata
            .get("artist")
            .or_else(|| self.metadata.get("album_artist"))
            .map(String::as_str)
    }

    pub fn first(&self, kind: TrackKind) -> Option<&StreamInfo> {
        self.streams.iter().find(|stream| stream.kind == kind)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StreamInfo {
    /// Same identifier `PlayerSnapshot::tracks` uses, for example `video:0`.
    pub id: String,
    pub index: usize,
    pub kind: TrackKind,
    pub codec: String,
    pub decodable: bool,
    pub duration: Option<MediaTime>,
    pub bit_rate: Option<u64>,
    pub language: Option<String>,
    pub metadata: BTreeMap<String, String>,
    pub details: StreamDetails,
}

#[derive(Clone, Debug, PartialEq)]
pub enum StreamDetails {
    Video {
        width: u32,
        height: u32,
        /// Zero when unknown.
        frame_rate: (u32, u32),
        pixel_aspect_ratio: (u32, u32),
        pixel_format: String,
    },
    Audio {
        sample_rate: u32,
        channels: u16,
        sample_format: String,
    },
    Other,
}

/// What a `MediaDecoder` produces.
#[derive(Clone, Debug, PartialEq)]
pub struct DecoderOptions {
    pub video: bool,
    pub audio: bool,
    /// Bounding box for converted frames; aspect ratio is kept. `None` keeps the source size.
    pub video_size: Option<(u32, u32)>,
    /// Resampling target. `None` keeps the source rate and channel count.
    pub audio_format: Option<AudioFormat>,
}

impl Default for DecoderOptions {
    fn default() -> Self {
        Self {
            video: true,
            audio: true,
            video_size: None,
            audio_format: None,
        }
    }
}

impl DecoderOptions {
    pub fn video_only() -> Self {
        Self {
            audio: false,
            ..Self::default()
        }
    }

    pub fn audio_only() -> Self {
        Self {
            video: false,
            ..Self::default()
        }
    }

    /// Opens no decoder; only the description is read.
    pub fn probe() -> Self {
        Self {
            video: false,
            audio: false,
            ..Self::default()
        }
    }
}
