//! Translation of Desktop/Sources/CoveUI/Diagnostics/FrameMetrics.swift.

use cove_logging::{Level, LogEvent, LogSystem};
use std::time::Instant;

/// Rolling one-second summary of frame timings, emitted when `COVE_FPS_LOG=1`.
pub struct FrameMetrics {
    pub fps_frames: u32,
    pub fps_window_start: Option<Instant>,
    pub fps_layout_accum: f64,
    pub fps_draw_accum: f64,
    pub fps_present_accum: f64,
    /// Time spent in the per-frame tick callback.
    pub fps_tick_accum: f64,
    /// Whole-frame wall time, from the first thing `handle_frame` does to the
    /// last. The three phase timers above cover only part of a frame, so a
    /// stall that happens outside them used to be invisible.
    pub fps_frame_accum: f64,
    /// Frames the render thread refused because every slot was in flight.
    pub fps_dropped: u32,
}

impl Default for FrameMetrics {
    fn default() -> Self {
        Self {
            fps_frames: 0,
            fps_window_start: None,
            fps_layout_accum: 0.0,
            fps_draw_accum: 0.0,
            fps_present_accum: 0.0,
            fps_tick_accum: 0.0,
            fps_frame_accum: 0.0,
            fps_dropped: 0,
        }
    }
}

impl FrameMetrics {
    /// Accumulates one frame's timing and emits a one-line summary roughly once
    /// per second. `present_ms` covers acquire + encode + submit + present, i.e.
    /// the part that blocks on vsync / GPU / cross-adapter copy. `frame_ms` is
    /// the whole frame, so `frame - (layout + draw)` is the time spent outside
    /// the measured phases. `dropped` counts frames the renderer refused during
    /// this window, which is how "not rendering" shows up in the log at all.
    pub fn record(
        &mut self,
        layout_ms: f64,
        draw_ms: f64,
        present_ms: f64,
        frame_ms: f64,
        dropped: u32,
    ) {
        let now = Instant::now();
        let start = *self.fps_window_start.get_or_insert(now);
        self.fps_frames += 1;
        self.fps_layout_accum += layout_ms;
        self.fps_draw_accum += draw_ms;
        self.fps_present_accum += present_ms;
        self.fps_frame_accum += frame_ms;
        self.fps_dropped += dropped;

        let elapsed = now.duration_since(start).as_secs_f64();
        if elapsed < 1.0 {
            return;
        }
        let count = f64::from(self.fps_frames);
        // Journal the summary once per second; individual frames remain in
        // timing streams. `dropped` is reported per window, not per frame.
        let line = format!(
            "[cove.fps] {:.1} fps | frame={:.2}ms | cpu: layout={:.2}ms draw={:.2}ms tick={:.2}ms | gpu/present={:.2}ms | dropped={} (n={})",
            count / elapsed,
            self.fps_frame_accum / count,
            self.fps_layout_accum / count,
            self.fps_draw_accum / count,
            self.fps_tick_accum / count,
            self.fps_present_accum / count,
            self.fps_dropped,
            self.fps_frames
        );
        LogSystem::logger("com.cove.ui.timing").log(&LogEvent::new(Level::Info, line));
        self.fps_frames = 0;
        self.fps_window_start = Some(now);
        self.fps_layout_accum = 0.0;
        self.fps_draw_accum = 0.0;
        self.fps_present_accum = 0.0;
        self.fps_tick_accum = 0.0;
        self.fps_frame_accum = 0.0;
        self.fps_dropped = 0;
    }
}
