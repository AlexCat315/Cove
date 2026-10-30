//! Translation of Desktop/Sources/CoveUI/Diagnostics/PerformanceHUD.swift.

use cove_core::{Color, DrawList, Font, FontWeight, Point, Rect, TextAlignment};
use cove_text::{DrawListText, TextEnvironmentHolder};
use std::time::Instant;

/// Frame-rate overlay drawn into the application draw list.
pub struct PerformanceHud {
    sample_time: Instant,
    ticks: u32,
    frames: u32,
    label: String,
}

impl Default for PerformanceHud {
    fn default() -> Self {
        Self {
            sample_time: Instant::now(),
            ticks: 0,
            frames: 0,
            label: "Render FPS 0  Tick FPS 0".to_owned(),
        }
    }
}

impl PerformanceHud {
    pub fn tick(&mut self) {
        self.ticks += 1;
    }

    pub fn presented(&mut self) {
        self.frames += 1;
    }

    pub fn draw(&mut self, list: &mut DrawList) {
        let Some(environment) = TextEnvironmentHolder::current() else { return };
        let now = Instant::now();
        let elapsed = now.duration_since(self.sample_time).as_secs_f64();
        if elapsed >= 0.5 {
            self.label = format!(
                "Render FPS {}  Tick FPS {}",
                (f64::from(self.frames) / elapsed).round() as i64,
                (f64::from(self.ticks) / elapsed).round() as i64
            );
            self.ticks = 0;
            self.frames = 0;
            self.sample_time = now;
        }
        let font = environment.resolved_font(Some(&Font::system(13.0, FontWeight::Medium)));
        let layout = environment.cached_layout(
            &self.label,
            Some(&font),
            None,
            f32::INFINITY,
            TextAlignment::Leading,
            0.0,
        );
        let line_height = environment.resolved_line_height(&font, None);
        let _ = list.add_rounded_rect(
            Rect::new(14.0, 14.0, layout.total_width + 20.0, line_height + 16.0),
            10.0,
            Color { r: 0.08, g: 0.10, b: 0.13, a: 0.88 },
        );
        let _ = list.add_text(
            &layout,
            Point { x: 24.0, y: 22.0 },
            Color::WHITE,
            environment.atlas_texture_id,
            Some(&environment.atlas),
            None,
        );
    }
}
