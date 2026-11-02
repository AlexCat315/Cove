//! Translation of CoveAndroidContract/FrameClock.swift.

/// Uses Choreographer's monotonic display timestamp, including idle callbacks.
/// A new surface starts a fresh clock, so background time never advances a
/// scene.
///
/// Kept in the platform-neutral contract module so the Android app and host
/// tests exercise one implementation instead of a copied assertion target.
#[derive(Clone, Debug, Default)]
pub struct DisplayFrameClock {
    previous: Option<u64>,
}

impl DisplayFrameClock {
    pub fn new() -> Self {
        Self { previous: None }
    }

    pub fn advance(&mut self, timestamp: u64) -> f64 {
        let Some(previous) = self.previous else {
            self.previous = Some(timestamp);
            return 1.0 / 60.0;
        };
        if timestamp <= previous {
            return 0.0;
        }
        self.previous = Some(timestamp);
        ((timestamp - previous) as f64 / 1_000_000_000.0).min(0.1)
    }

    /// The most recent display timestamp observed by this clock.
    pub fn previous(&self) -> Option<u64> {
        self.previous
    }
}

/// Caps rendered frames while advancing the display clock on idle callbacks.
#[derive(Clone, Debug, Default)]
pub struct DisplayFramePacer {
    clock: DisplayFrameClock,
    last_frame: Option<u64>,
    accumulated_delta: f64,
}

impl DisplayFramePacer {
    pub fn advance(
        &mut self,
        timestamp: u64,
        needs_frame: bool,
        target_rate: Option<f64>,
    ) -> Option<f64> {
        let delta = self.clock.advance(timestamp);
        if !needs_frame {
            self.accumulated_delta = 0.0;
            return None;
        }
        self.accumulated_delta = (self.accumulated_delta + delta).min(0.1);
        if let (Some(previous), Some(rate)) = (
            self.last_frame,
            target_rate.filter(|rate| rate.is_finite() && *rate > 0.0),
        ) && timestamp.saturating_sub(previous) as f64 + 1.0
            < 1_000_000_000.0 / rate.clamp(1.0, 240.0)
        {
            return None;
        }
        self.last_frame = Some(timestamp);
        Some(std::mem::take(&mut self.accumulated_delta))
    }
}
