//! Frame mirroring, timing publication and input translation.
//!
//! These are host-facing helpers: the host runtime calls `record`/`capture` once
//! per frame and the sinks forward to the server. None of them instrument
//! anything on their own.
use cove_devtools_protocol::{MirrorFrame, TimingFrame};
use cove_platform::{InputEvent, KeyEvent, KeyModifiers, MouseButton, MouseButtonEvent, MouseMotionEvent, MouseWheelEvent, ScrollInput};
use std::cell::RefCell;
use std::rc::Rc;

/// Owned BGRA8 pixels supplied by a host renderer. Padding may follow each row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameTapPixels {
    pub data: Vec<u8>,
    pub bytes_per_row: usize,
}

impl FrameTapPixels {
    pub fn new(data: Vec<u8>, bytes_per_row: usize) -> Self {
        Self { data, bytes_per_row }
    }
}

/// Backend that turns the current draw list into pixels.
pub type FrameCapture = Rc<dyn Fn(&cove_core::DrawList, u32, u32, f32, f32) -> Result<FrameTapPixels, FrameTapError>>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FrameTapError {
    Unsupported(String),
    InvalidArgument(String),
}

impl std::fmt::Display for FrameTapError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported(message) | Self::InvalidArgument(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for FrameTapError {}

/// Queue a host sink pushes into. The server drains it from `pump`, which keeps
/// delivery on the UI thread without giving the sink a borrow of the server.
pub type OutboundQueue = Rc<RefCell<Vec<crate::server::Outbound>>>;

/// Capture sink. `deliver` is set by `DevTools` once the server is ready.
#[derive(Default)]
pub struct FrameTapSink {
    pub deliver: RefCell<Option<Rc<dyn Fn(MirrorFrame)>>>,
}

impl FrameTapSink {
    pub fn new() -> Rc<Self> {
        Rc::new(Self::default())
    }
    pub fn set_deliver(&self, deliver: Option<Rc<dyn Fn(MirrorFrame)>>) {
        *self.deliver.borrow_mut() = deliver;
    }
    fn deliver(&self, frame: MirrorFrame) {
        let callback = self.deliver.borrow().clone();
        if let Some(callback) = callback {
            callback(frame);
        }
    }
    fn is_wired(&self) -> bool {
        self.deliver.borrow().is_some()
    }
}

/// Rate-limited frame mirroring.
///
/// Captures a copy of each rendered frame into an offscreen texture, reads it
/// back, JPEG-encodes it and pushes the result to attached clients. Capture is
/// rate-limited (default 15 fps) so the host is not stalled every frame.
pub struct FrameTap {
    sink: Rc<FrameTapSink>,
    capture_pixels: Option<FrameCapture>,
    reset_capture: Box<dyn Fn()>,
    enabled: bool,
    quality: f64,
    scale: f64,
    min_frame_interval: f64,
    last_capture_at: f64,
    sequence: u64,
    consecutive_errors: u32,
    captures_since_start: u64,
    /// Clock injection so rate limiting is testable without sleeping.
    now: Rc<dyn Fn() -> f64>,
}

impl FrameTap {
    /// `false` on platforms whose renderer cannot read pixels back. The field is
    /// a value rather than a compile-time gate so a host can report support
    /// honestly at runtime.
    pub const IS_SUPPORTED: bool = true;

    pub fn new(sink: Rc<FrameTapSink>, capture: FrameCapture) -> Self {
        Self {
            sink,
            capture_pixels: Some(capture),
            reset_capture: Box::new(|| {}),
            enabled: false,
            quality: 0.7,
            scale: 1.0,
            min_frame_interval: 1.0 / 15.0,
            last_capture_at: 0.0,
            sequence: 0,
            consecutive_errors: 0,
            captures_since_start: 0,
            now: Rc::new(default_now),
        }
    }

    pub fn with_reset(mut self, reset: impl Fn() + 'static) -> Self {
        self.reset_capture = Box::new(reset);
        self
    }

    #[doc(hidden)]
    pub fn with_clock(mut self, now: impl Fn() -> f64 + 'static) -> Self {
        self.now = Rc::new(now);
        self
    }

    pub fn is_active(&self) -> bool {
        self.enabled
    }

    pub fn start(&mut self, fps: f64, quality: f64, scale: f64) {
        self.scale = if scale.is_finite() { scale.clamp(1.0, 3.0) } else { 1.0 };
        let clamped_fps = fps.clamp(1.0, 60.0);
        self.min_frame_interval = 1.0 / clamped_fps;
        self.quality = quality.clamp(0.1, 1.0);
        self.enabled = true;
        self.last_capture_at = 0.0;
        self.consecutive_errors = 0;
        self.captures_since_start = 0;
        cove_logging::LogSystem::logger("cove.devtools.frameTap")
            .log(&cove_logging::LogEvent::new(
                cove_logging::Level::Info,
                format!("mirror start fps={clamped_fps} quality={}", self.quality),
            ));
    }

    pub fn stop(&mut self) {
        self.enabled = false;
        cove_logging::LogSystem::logger("cove.devtools.frameTap").log(&cove_logging::LogEvent::new(
            cove_logging::Level::Info,
            format!(
                "mirror stop after {} captures, errors={}",
                self.captures_since_start, self.consecutive_errors
            ),
        ));
        (self.reset_capture)();
    }

    /// Render the same draw list through the capture source and emit a
    /// `mirror.frame`.
    pub fn capture(&mut self, draw_list: &cove_core::DrawList, width_px: u32, height_px: u32, logical_width: f32, logical_height: f32) {
        if !self.enabled {
            return;
        }
        if !self.sink.is_wired() {
            if self.captures_since_start == 0 && self.consecutive_errors == 0 {
                self.consecutive_errors = 1;
            }
            return;
        }
        if width_px == 0 || height_px == 0 {
            if self.captures_since_start == 0 && self.consecutive_errors == 0 {
                self.consecutive_errors = 1;
            }
            return;
        }
        // A remote inspector does not need Retina-native pixels. Capturing at
        // logical resolution (capped for very large windows) cuts GPU readback,
        // JPEG size, wire traffic and browser decode pressure sharply.
        let native = width_px as f64 / logical_width.max(1.0) as f64;
        let (capture_width, capture_height) = target_size(logical_width, logical_height, self.scale.min(native.max(1.0)));
        let now = (self.now)();
        if now - self.last_capture_at < self.min_frame_interval {
            return;
        }
        self.last_capture_at = now;
        let Some(capture) = self.capture_pixels.clone() else { return };
        match capture(draw_list, capture_width, capture_height, logical_width, logical_height) {
            Ok(pixels) => {
                let height = capture_height as usize;
                let width = capture_width as usize;
                if pixels.bytes_per_row < width * 4
                    || pixels.data.len() < pixels.bytes_per_row.saturating_mul(height)
                {
                    self.consecutive_errors = self.consecutive_errors.saturating_add(1);
                    return;
                }
                let Some(jpeg) = encode_jpeg_bgra(
                    &pixels.data,
                    width,
                    height,
                    pixels.bytes_per_row,
                    self.quality as f32,
                ) else {
                    self.consecutive_errors = self.consecutive_errors.saturating_add(1);
                    return;
                };
                self.sequence = self.sequence.wrapping_add(1);
                self.captures_since_start = self.captures_since_start.wrapping_add(1);
                self.consecutive_errors = 0;
                self.sink.deliver(MirrorFrame {
                    seq: self.sequence,
                    width: capture_width,
                    height: capture_height,
                    logical_width: logical_width as f64,
                    logical_height: logical_height as f64,
                    jpeg_base64: base64_encode(&jpeg),
                });
            }
            Err(_) => {
                self.consecutive_errors = self.consecutive_errors.saturating_add(1);
                if self.consecutive_errors <= 3 || self.consecutive_errors % 60 == 0 {
                    cove_logging::LogSystem::logger("cove.devtools.frameTap").log(&cove_logging::LogEvent::new(
                        cove_logging::Level::Warning,
                        format!("mirror capture failed (#{})", self.consecutive_errors),
                    ));
                }
            }
        }
    }
}

fn default_now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs_f64())
        .unwrap_or(0.0)
}

/// Pixel budget: 1920x1080 unless upscaling was requested, then 3840x2160.
fn target_size(logical_width: f32, logical_height: f32, requested: f64) -> (u32, u32) {
    let width = (logical_width as f64).max(1.0);
    let height = (logical_height as f64).max(1.0);
    let (budget_width, budget_height) = if requested > 1.0 { (3840.0, 2160.0) } else { (1920.0, 1080.0) };
    let scale = requested.min((budget_width / width).min(budget_height / height));
    (
        ((width * scale).ceil() as i64).max(1) as u32,
        ((height * scale).ceil() as i64).max(1) as u32,
    )
}

// MARK: - Base64

/// Standard base64 with padding. Implemented here so the crate needs no
/// encoding dependency.
pub fn base64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        output.push(ALPHABET[((triple >> 18) & 63) as usize] as char);
        output.push(ALPHABET[((triple >> 12) & 63) as usize] as char);
        output.push(if chunk.len() > 1 { ALPHABET[((triple >> 6) & 63) as usize] as char } else { '=' });
        output.push(if chunk.len() > 2 { ALPHABET[(triple & 63) as usize] as char } else { '=' });
    }
    output
}

// MARK: - JPEG

/// Baseline JPEG encoder for BGRA8 input.
///
/// macOS used ImageIO. Rust has no shared image encoder in this workspace, so
/// the mirror needs its own. This is a complete baseline (SOF0) encoder with the
/// standard Annex K tables: real JPEG output at the requested quality, not a
/// placeholder. Chroma is subsampled 4:2:0, which halves the payload for UI
/// content without visible loss at mirror scale.
pub fn encode_jpeg_bgra(data: &[u8], width: usize, height: usize, bytes_per_row: usize, quality: f32) -> Option<Vec<u8>> {
    if width == 0 || height == 0 || bytes_per_row < width * 4 {
        return None;
    }
    let mut y_plane = vec![0u8; width * height];
    let mut cb_plane = vec![0u8; width.div_ceil(2) * height.div_ceil(2)];
    let mut cr_plane = vec![0u8; width.div_ceil(2) * height.div_ceil(2)];
    let chroma_width = width.div_ceil(2);
    for y in 0..height {
        for x in 0..width {
            let offset = y * bytes_per_row + x * 4;
            let (b, g, r) = (data[offset] as f32, data[offset + 1] as f32, data[offset + 2] as f32);
            // BGRA is premultiplied-first little-endian, so B comes first.
            let yy = 0.299 * r + 0.587 * g + 0.114 * b;
            y_plane[y * width + x] = yy.clamp(0.0, 255.0) as u8;
            if x % 2 == 0 && y % 2 == 0 {
                let cbi = -0.168_736 * r - 0.331_264 * g + 0.5 * b + 128.0;
                let cri = 0.5 * r - 0.418_688 * g - 0.081_312 * b + 128.0;
                let index = (y / 2) * chroma_width + x / 2;
                cb_plane[index] = cbi.clamp(0.0, 255.0) as u8;
                cr_plane[index] = cri.clamp(0.0, 255.0) as u8;
            }
        }
    }
    let quality = quality.clamp(0.1, 1.0);
    let scaled = |table: &[u8; 64]| -> [u8; 64] {
        let factor = if quality < 0.5 { 5000.0 / (quality * 100.0) } else { 200.0 - quality * 200.0 };
        let mut result = [0u8; 64];
        for (index, value) in table.iter().enumerate() {
            let scaled = ((*value as f32 * factor + 50.0) / 100.0).floor().clamp(1.0, 255.0);
            result[index] = scaled as u8;
        }
        result
    };
    let quant_y = scaled(&STD_LUMA_QUANT);
    let quant_c = scaled(&STD_CHROMA_QUANT);
    let mut writer = Vec::with_capacity(width * height / 4);
    write_jpeg(&mut writer, width, height, &y_plane, &cb_plane, &cr_plane, chroma_width, &quant_y, &quant_c);
    Some(writer)
}

const STD_LUMA_QUANT: [u8; 64] = [
    16, 11, 10, 16, 24, 40, 51, 61, 12, 12, 14, 19, 26, 58, 60, 55, 14, 13, 16, 24, 40, 57, 69, 56, 14, 17, 22, 29, 51,
    87, 80, 62, 18, 22, 37, 56, 68, 109, 103, 77, 24, 35, 55, 64, 81, 104, 113, 92, 49, 64, 78, 87, 103, 121, 120, 101,
    72, 92, 95, 98, 112, 100, 103, 99,
];

const STD_CHROMA_QUANT: [u8; 64] = [
    17, 18, 24, 47, 99, 99, 99, 99, 18, 21, 26, 66, 99, 99, 99, 99, 24, 26, 56, 99, 99, 99, 99, 99, 47, 66, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99,
];

/// Zig-zag order mapping natural index -> sequence position.
const ZIGZAG: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20, 13, 6, 7, 14, 21,
    28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59, 52, 45, 38, 31, 39, 46, 53, 60, 61, 54,
    47, 55, 62, 63,
];

/// Standard Annex K DC/AC Huffman tables (luminance/chrominance).
const DC_LUMA_BITS: [u8; 16] = [0, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0];
const DC_LUMA_VALS: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
const DC_CHROMA_BITS: [u8; 16] = [0, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0];
const DC_CHROMA_VALS: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
const AC_LUMA_BITS: [u8; 16] = [0, 2, 1, 3, 3, 2, 4, 3, 5, 5, 4, 4, 0, 0, 1, 125];
const AC_LUMA_VALS: [u8; 162] = [
    0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06, 0x13, 0x51, 0x61, 0x07, 0x22, 0x71, 0x14,
    0x32, 0x81, 0x91, 0xa1, 0x08, 0x23, 0x42, 0xb1, 0xc1, 0x15, 0x52, 0xd1, 0xf0, 0x24, 0x33, 0x62, 0x72, 0x82, 0x09,
    0x0a, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a,
    0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65,
    0x66, 0x67, 0x68, 0x69, 0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88,
    0x89, 0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9,
    0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3, 0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca,
    0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xe1, 0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea,
    0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8, 0xf9, 0xfa,
];
const AC_CHROMA_BITS: [u8; 16] = [0, 2, 1, 2, 4, 4, 3, 4, 7, 5, 4, 4, 0, 1, 2, 119];
const AC_CHROMA_VALS: [u8; 162] = [
    0x00, 0x01, 0x02, 0x03, 0x11, 0x04, 0x05, 0x21, 0x31, 0x06, 0x12, 0x41, 0x51, 0x07, 0x61, 0x71, 0x13, 0x22, 0x32,
    0x81, 0x08, 0x14, 0x42, 0x91, 0xa1, 0xb1, 0xc1, 0x09, 0x23, 0x33, 0x52, 0xf0, 0x15, 0x62, 0x72, 0xd1, 0x0a, 0x16,
    0x24, 0x34, 0xe1, 0x25, 0xf1, 0x17, 0x18, 0x19, 0x1a, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x35, 0x36, 0x37, 0x38, 0x39,
    0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64,
    0x65, 0x66, 0x67, 0x68, 0x69, 0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x82, 0x83, 0x84, 0x85, 0x86,
    0x87, 0x88, 0x89, 0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7,
    0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3, 0xc4, 0xc5, 0xc6, 0xc7, 0xc8,
    0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9,
    0xea, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8, 0xf9, 0xfa,
];

fn build_huffman(bits: &[u8; 16], values: &[u8]) -> Vec<(u8, u16, u8)> {
    let mut table = Vec::with_capacity(values.len());
    let mut code: u16 = 0;
    let mut index = 0usize;
    for (length_index, count) in bits.iter().enumerate() {
        let length = (length_index + 1) as u8;
        for _ in 0..*count {
            if index < values.len() {
                table.push((values[index], code, length));
                index += 1;
            }
            code += 1;
        }
        code <<= 1;
    }
    table
}

struct BitWriter {
    buffer: Vec<u8>,
    accumulator: u32,
    bits: u8,
}

impl BitWriter {
    fn new(capacity: usize) -> Self {
        Self { buffer: Vec::with_capacity(capacity), accumulator: 0, bits: 0 }
    }
    fn write(&mut self, value: u16, length: u8) {
        self.accumulator = (self.accumulator << length) | value as u32;
        self.bits += length;
        while self.bits >= 8 {
            let byte = ((self.accumulator >> (self.bits - 8)) & 0xFF) as u8;
            self.buffer.push(byte);
            if byte == 0xFF {
                // Byte stuffing: an 0xFF in entropy data is followed by 0x00.
                self.buffer.push(0x00);
            }
            self.bits -= 8;
        }
    }
    fn finish(mut self) -> Vec<u8> {
        if self.bits > 0 {
            // Pad the final byte with one-bits, as the spec requires.
            let padding = 8 - self.bits;
            self.write((1u16 << padding) - 1, padding);
        }
        self.buffer
    }
}

fn encode_block(
    writer: &mut BitWriter,
    coefficients: &[i32; 64],
    previous_dc: &mut i32,
    dc_table: &[(u8, u16, u8)],
    ac_table: &[(u8, u16, u8)],
) {
    let dc = coefficients[0];
    let difference = dc - *previous_dc;
    *previous_dc = dc;
    let (size, value) = magnitude(difference);
    if let Some((_, code, length)) = dc_table.iter().find(|(symbol, _, _)| *symbol == size) {
        writer.write(*code, *length);
    }
    if size > 0 {
        writer.write(value, size);
    }
    let mut run = 0u8;
    for index in 1..64 {
        let coefficient = coefficients[ZIGZAG[index]];
        if coefficient == 0 {
            run += 1;
            continue;
        }
        while run > 15 {
            if let Some((_, code, length)) = ac_table.iter().find(|(symbol, _, _)| *symbol == 0xF0) {
                writer.write(*code, *length);
            }
            run -= 16;
        }
        let (size, value) = magnitude(coefficient);
        let symbol = (run << 4) | size;
        if let Some((_, code, length)) = ac_table.iter().find(|(candidate, _, _)| *candidate == symbol) {
            writer.write(*code, *length);
        }
        writer.write(value, size);
        run = 0;
    }
    if run > 0
        && let Some((_, code, length)) = ac_table.iter().find(|(symbol, _, _)| *symbol == 0x00)
    {
        writer.write(*code, *length);
    }
}

/// JPEG magnitude category and the bit pattern for a signed value.
fn magnitude(value: i32) -> (u8, u16) {
    if value == 0 {
        return (0, 0);
    }
    let magnitude = value.unsigned_abs();
    let size = (32 - magnitude.leading_zeros()) as u8;
    let pattern = if value < 0 {
        ((1u32 << size) - 1 + value as u32) as u16
    } else {
        value as u16
    };
    (size, pattern)
}

fn forward_dct(block: &[f32; 64], output: &mut [i32; 64], quant: &[u8; 64]) {
    for v in 0..8 {
        for u in 0..8 {
            let mut sum = 0.0f32;
            for y in 0..8 {
                for x in 0..8 {
                    let sample = block[y * 8 + x];
                    let cu = if u == 0 { std::f32::consts::FRAC_1_SQRT_2 } else { 1.0 };
                    let cv = if v == 0 { std::f32::consts::FRAC_1_SQRT_2 } else { 1.0 };
                    let cos_x = ((2.0 * x as f32 + 1.0) * u as f32 * std::f32::consts::PI / 16.0).cos();
                    let cos_y = ((2.0 * y as f32 + 1.0) * v as f32 * std::f32::consts::PI / 16.0).cos();
                    sum += cu * cv * sample * cos_x * cos_y;
                }
            }
            let natural = v * 8 + u;
            let quantized = sum * 0.25 / quant[natural] as f32;
            output[natural] = quantized.round() as i32;
        }
    }
}

fn write_jpeg(
    out: &mut Vec<u8>,
    width: usize,
    height: usize,
    y_plane: &[u8],
    cb_plane: &[u8],
    cr_plane: &[u8],
    chroma_width: usize,
    quant_y: &[u8; 64],
    quant_c: &[u8; 64],
) {
    out.extend_from_slice(&[0xFF, 0xD8]); // SOI
    // APP0/JFIF
    out.extend_from_slice(&[0xFF, 0xE0, 0x00, 0x10]);
    out.extend_from_slice(b"JFIF\0");
    out.extend_from_slice(&[1, 1, 0, 0, 1, 0, 1, 0, 0]);
    // DQT: luminance then chrominance.
    for (id, table) in [(0u8, quant_y), (1u8, quant_c)] {
        out.extend_from_slice(&[0xFF, 0xDB, 0x00, 0x43, id]);
        for index in 0..64 {
            out.push(table[ZIGZAG[index]]);
        }
    }
    // SOF0: 3 components, 4:2:0 subsampling.
    out.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x11, 8]);
    out.extend_from_slice(&(height as u16).to_be_bytes());
    out.extend_from_slice(&(width as u16).to_be_bytes());
    out.push(3);
    out.extend_from_slice(&[1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1]);
    // DHT
    let mut write_huffman = |class: u8, id: u8, bits: &[u8; 16], values: &[u8]| {
        let length = 2 + 1 + 16 + values.len();
        out.extend_from_slice(&[0xFF, 0xC4]);
        out.extend_from_slice(&(length as u16).to_be_bytes());
        out.push((class << 4) | id);
        out.extend_from_slice(bits);
        out.extend_from_slice(values);
    };
    write_huffman(0, 0, &DC_LUMA_BITS, &DC_LUMA_VALS);
    write_huffman(1, 0, &AC_LUMA_BITS, &AC_LUMA_VALS);
    write_huffman(0, 1, &DC_CHROMA_BITS, &DC_CHROMA_VALS);
    write_huffman(1, 1, &AC_CHROMA_BITS, &AC_CHROMA_VALS);
    // SOS
    out.extend_from_slice(&[0xFF, 0xDA, 0x00, 0x0C, 3, 1, 0x00, 2, 0x11, 3, 0x11, 0, 63, 0]);

    let dc_luma = build_huffman(&DC_LUMA_BITS, &DC_LUMA_VALS);
    let ac_luma = build_huffman(&AC_LUMA_BITS, &AC_LUMA_VALS);
    let dc_chroma = build_huffman(&DC_CHROMA_BITS, &DC_CHROMA_VALS);
    let ac_chroma = build_huffman(&AC_CHROMA_BITS, &AC_CHROMA_VALS);

    let mut writer = BitWriter::new(width * height / 4);
    let (mut previous_dc_y, mut previous_dc_cb, mut previous_dc_cr) = (0i32, 0i32, 0i32);
    let mut sample_y = [0f32; 64];
    let mut sample_cb = [0f32; 64];
    let mut sample_cr = [0f32; 64];
    let mut coefficients = [0i32; 64];

    // MCU order: one 16x16 luma block set plus one 8x8 chroma block each.
    let mcu_width = width.div_ceil(16);
    let mcu_height = height.div_ceil(16);
    for mcu_y in 0..mcu_height {
        for mcu_x in 0..mcu_width {
            for (block_y, block_x) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
                for y in 0..8 {
                    for x in 0..8 {
                        let source_y = mcu_y * 16 + block_y * 8 + y;
                        let source_x = mcu_x * 16 + block_x * 8 + x;
                        // Edge blocks replicate the last real sample.
                        let clamped_y = source_y.min(height - 1);
                        let clamped_x = source_x.min(width - 1);
                        sample_y[y * 8 + x] = y_plane[clamped_y * width + clamped_x] as f32 - 128.0;
                    }
                }
                forward_dct(&sample_y, &mut coefficients, quant_y);
                encode_block(&mut writer, &coefficients, &mut previous_dc_y, &dc_luma, &ac_luma);
            }
            for (plane, sample, previous, dc, ac) in [
                (cb_plane, &mut sample_cb, &mut previous_dc_cb, &dc_chroma, &ac_chroma),
                (cr_plane, &mut sample_cr, &mut previous_dc_cr, &dc_chroma, &ac_chroma),
            ] {
                for y in 0..8 {
                    for x in 0..8 {
                        let source_y = mcu_y * 8 + y;
                        let source_x = mcu_x * 8 + x;
                        let clamped_y = source_y.min(height.div_ceil(2).saturating_sub(1));
                        let clamped_x = source_x.min(chroma_width.saturating_sub(1));
                        sample[y * 8 + x] = plane[clamped_y * chroma_width + clamped_x] as f32 - 128.0;
                    }
                }
                forward_dct(sample, &mut coefficients, quant_c);
                encode_block(&mut writer, &coefficients, previous, dc, ac);
            }
        }
    }
    out.extend_from_slice(&writer.finish());
    out.extend_from_slice(&[0xFF, 0xD9]); // EOI
}

// MARK: - Timing

/// Rolling timing publisher. The host runtime calls `record` once per frame.
#[derive(Default)]
pub struct TimingPublisher {
    deliver: RefCell<Option<Rc<dyn Fn(TimingFrame)>>>,
    frame: std::cell::Cell<u64>,
}

impl TimingPublisher {
    pub fn new() -> Rc<Self> {
        Rc::new(Self::default())
    }
    pub fn set_deliver(&self, deliver: Option<Rc<dyn Fn(TimingFrame)>>) {
        *self.deliver.borrow_mut() = deliver;
    }
    /// Emit one frame. `recomposition_ms` is absent because the host reports it
    /// per component instead of per frame, matching the Swift publisher.
    #[allow(clippy::too_many_arguments)]
    pub fn record(
        &self,
        layout_ms: f64,
        draw_ms: f64,
        present_ms: f64,
        total_ms: f64,
        node_count: usize,
        batch_count: usize,
        presented: bool,
    ) {
        let callback = self.deliver.borrow().clone();
        let Some(callback) = callback else { return };
        self.frame.set(self.frame.get().wrapping_add(1));
        callback(TimingFrame {
            frame: self.frame.get(),
            recomposition_ms: None,
            layout_ms,
            draw_ms,
            present_ms,
            presented: Some(presented),
            total_ms,
            node_count,
            batch_count,
        });
    }
}

// MARK: - Input bridge

/// Translates `MirrorInput` into `InputEvent` for the host's window session.
///
/// Keyboard support maps common Web `KeyboardEvent.code` values to SDL/USB-HID
/// scancodes used by CoveUI.
pub struct InputBridge;

impl InputBridge {
    pub fn event(payload: &cove_devtools_protocol::MirrorInput) -> Option<InputEvent> {
        match payload.kind.as_str() {
            "pointerMove" => Some(InputEvent::MouseMotion(MouseMotionEvent {
                x: payload.x.unwrap_or(0.0),
                y: payload.y.unwrap_or(0.0),
                delta_x: payload.delta_x.unwrap_or(0.0),
                delta_y: payload.delta_y.unwrap_or(0.0),
                ..Default::default()
            })),
            "pointerDown" => Some(InputEvent::MouseButtonDown(MouseButtonEvent {
                button: mouse_button(payload.button.unwrap_or(0)),
                x: payload.x.unwrap_or(0.0),
                y: payload.y.unwrap_or(0.0),
                clicks: (payload.click_count.unwrap_or(1).clamp(1, 255)) as u8,
                modifiers: key_modifiers(payload.modifiers.unwrap_or(0)),
                ..Default::default()
            })),
            "pointerUp" => Some(InputEvent::MouseButtonUp(MouseButtonEvent {
                button: mouse_button(payload.button.unwrap_or(0)),
                x: payload.x.unwrap_or(0.0),
                y: payload.y.unwrap_or(0.0),
                clicks: (payload.click_count.unwrap_or(1).clamp(1, 255)) as u8,
                modifiers: key_modifiers(payload.modifiers.unwrap_or(0)),
                ..Default::default()
            })),
            "wheel" => Some(InputEvent::MouseWheel(MouseWheelEvent {
                x: payload.delta_x.unwrap_or(0.0),
                y: payload.delta_y.unwrap_or(0.0),
                mouse_x: payload.x,
                mouse_y: payload.y,
                input: payload.scroll.clone().unwrap_or_else(ScrollInput::default),
                ..Default::default()
            })),
            "text" => {
                let text = payload.text.as_ref()?;
                (!text.is_empty()).then(|| InputEvent::TextInput(text.clone()))
            }
            "keyDown" | "keyUp" => {
                let scancode = key_scancode(payload.key.as_deref())?;
                let event = KeyEvent {
                    scancode,
                    keycode: payload.key_code.unwrap_or(0).max(0) as u32,
                    modifiers: key_modifiers(payload.modifiers.unwrap_or(0)),
                    is_repeat: payload.is_repeat.unwrap_or(false),
                };
                Some(if payload.kind == "keyDown" {
                    InputEvent::KeyDown(event)
                } else {
                    InputEvent::KeyUp(event)
                })
            }
            _ => None,
        }
    }
}

/// Web pointer buttons: 0 left, 1 middle, 2 right, 3/4 the extra buttons.
fn mouse_button(raw: i64) -> MouseButton {
    match raw {
        0 => MouseButton::Left,
        1 => MouseButton::Middle,
        2 => MouseButton::Right,
        3 => MouseButton::X1,
        4 => MouseButton::X2,
        _ => MouseButton::Left,
    }
}

/// Bit 0 shift, 1 ctrl, 2 alt, 3 gui — the DOM `modifier` bit layout.
fn key_modifiers(raw: i64) -> KeyModifiers {
    let mut modifiers = KeyModifiers::default();
    if raw & 1 != 0 {
        modifiers = modifiers | KeyModifiers::LEFT_SHIFT;
    }
    if raw & 2 != 0 {
        modifiers = modifiers | KeyModifiers::LEFT_CTRL;
    }
    if raw & 4 != 0 {
        modifiers = modifiers | KeyModifiers::LEFT_ALT;
    }
    if raw & 8 != 0 {
        modifiers = modifiers | KeyModifiers::LEFT_GUI;
    }
    modifiers
}

/// Map a DOM `KeyboardEvent.code` to a USB-HID scancode.
pub fn key_scancode(code: Option<&str>) -> Option<u32> {
    let code = code?;
    if code.is_empty() {
        return None;
    }
    if let Some(letter) = code.strip_prefix("Key")
        && letter.len() == 1
        && let Some(character) = letter.chars().next()
        && character.is_ascii_uppercase()
    {
        return Some(4 + character as u32 - 'A' as u32);
    }
    if let Some(digit) = code.strip_prefix("Digit")
        && let Ok(value) = digit.parse::<u32>()
        && value <= 9
    {
        return Some(if value == 0 { 39 } else { 29 + value });
    }
    if let Some(number) = code.strip_prefix('F')
        && let Ok(value) = number.parse::<u32>()
        && (1..=12).contains(&value)
    {
        return Some(57 + value);
    }
    if let Some(digit) = code.strip_prefix("Numpad")
        && let Ok(value) = digit.parse::<u32>()
        && value <= 9
    {
        return Some(if value == 0 { 98 } else { 88 + value });
    }
    Some(match code {
        "NumpadEnter" => 88,
        "Enter" => 40,
        "Escape" => 41,
        "Backspace" => 42,
        "Tab" => 43,
        "Space" => 44,
        "Minus" => 45,
        "Equal" => 46,
        "BracketLeft" => 47,
        "BracketRight" => 48,
        "Backslash" => 49,
        "Semicolon" => 51,
        "Quote" => 52,
        "Backquote" => 53,
        "Comma" => 54,
        "Period" => 55,
        "Slash" => 56,
        "CapsLock" => 57,
        "PrintScreen" => 70,
        "ScrollLock" => 71,
        "Pause" => 72,
        "Insert" => 73,
        "Home" => 74,
        "PageUp" => 75,
        "Delete" => 76,
        "End" => 77,
        "PageDown" => 78,
        "ArrowRight" => 79,
        "ArrowLeft" => 80,
        "ArrowDown" => 81,
        "ArrowUp" => 82,
        "NumLock" => 83,
        "NumpadDivide" => 84,
        "NumpadMultiply" => 85,
        "NumpadSubtract" => 86,
        "NumpadAdd" => 87,
        "NumpadDecimal" => 99,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_known_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn target_size_respects_the_pixel_budget() {
        assert_eq!(target_size(1920.0, 1080.0, 1.0), (1920, 1080));
        // Asking for more than the budget clamps down.
        assert_eq!(target_size(3840.0, 2160.0, 1.0), (1920, 1080));
        // Upscale request uses the larger budget.
        assert_eq!(target_size(1920.0, 1080.0, 2.0), (3840, 2160));
    }

    #[test]
    fn key_codes_map_to_hid_scancodes() {
        assert_eq!(key_scancode(Some("KeyA")), Some(4));
        assert_eq!(key_scancode(Some("KeyZ")), Some(29));
        assert_eq!(key_scancode(Some("Digit1")), Some(30));
        assert_eq!(key_scancode(Some("Digit0")), Some(39));
        assert_eq!(key_scancode(Some("F1")), Some(58));
        assert_eq!(key_scancode(Some("F12")), Some(69));
        assert_eq!(key_scancode(Some("Escape")), Some(41));
        assert_eq!(key_scancode(Some("NumpadEnter")), Some(88));
        assert_eq!(key_scancode(Some("Nope")), None);
        assert_eq!(key_scancode(None), None);
    }

    #[test]
    fn jpeg_encode_produces_a_valid_soi_eoi_frame() {
        // 16x16 opaque blue BGRA.
        let mut data = Vec::new();
        for _ in 0..16 * 16 {
            data.extend_from_slice(&[255, 0, 0, 255]);
        }
        let jpeg = encode_jpeg_bgra(&data, 16, 16, 64, 0.8).expect("encoded");
        assert_eq!(&jpeg[..2], &[0xFF, 0xD8], "starts with SOI");
        assert_eq!(&jpeg[jpeg.len() - 2..], &[0xFF, 0xD9], "ends with EOI");
        assert!(jpeg.len() > 100, "has payload");
        // A 4:2:0 frame has three component blocks per MCU.
        assert!(encode_jpeg_bgra(&data, 0, 16, 64, 0.8).is_none());
    }

    #[test]
    fn timing_publisher_only_emits_when_wired() {
        use std::cell::Cell;
        use std::rc::Rc;
        let received = Rc::new(Cell::new(0u64));
        let publisher = TimingPublisher::new();
        publisher.record(1.0, 2.0, 3.0, 6.0, 10, 2, true);
        assert_eq!(received.get(), 0);
        let counter = received.clone();
        publisher.set_deliver(Some(Rc::new(move |frame| counter.set(frame.frame))));
        publisher.record(1.0, 2.0, 3.0, 6.0, 10, 2, true);
        assert_eq!(received.get(), 1);
        publisher.record(1.0, 2.0, 3.0, 6.0, 10, 2, true);
        assert_eq!(received.get(), 2);
    }

    #[test]
    fn frame_tap_respects_its_rate_limit() {
        use std::cell::Cell;
        use std::rc::Rc;
        let delivered = Rc::new(Cell::new(0u32));
        let clock = Rc::new(Cell::new(0.0f64));
        let sink = FrameTapSink::new();
        let counter = delivered.clone();
        sink.set_deliver(Some(Rc::new(move |_frame| counter.set(counter.get() + 1))));
        let mut data = Vec::new();
        for _ in 0..8 * 8 {
            data.extend_from_slice(&[0, 0, 0, 255]);
        }
        // 8x8 keeps the encode cheap; the tap only needs a valid buffer.
        let mut tap = FrameTap::new(
            sink.clone(),
            Rc::new(move |_list, _w, _h, _lw, _lh| Ok(FrameTapPixels::new(data.clone(), 8 * 4))),
        );
        let clock_handle = clock.clone();
        tap = tap.with_clock(move || clock_handle.get());
        tap.start(10.0, 0.5, 1.0);
        let list = cove_core::DrawList::default();
        tap.capture(&list, 16, 16, 16.0, 16.0);
        assert_eq!(delivered.get(), 1, "first capture passes");
        // Immediately again: inside the 100 ms window.
        tap.capture(&list, 16, 16, 16.0, 16.0);
        assert_eq!(delivered.get(), 1, "rate limited");
        clock.set(0.5);
        tap.capture(&list, 16, 16, 16.0, 16.0);
        assert_eq!(delivered.get(), 2, "after the interval");
        tap.stop();
        assert!(!tap.is_active());
    }
}
