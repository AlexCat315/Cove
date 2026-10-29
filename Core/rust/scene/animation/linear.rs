//! Translation of Animation/Curves/Linear.swift.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimingPoint { pub time: f32, pub progress: f32 }
#[derive(Clone, Debug, PartialEq)]
pub struct LinearTimingCurve { points: Vec<TimingPoint> }
impl LinearTimingCurve {
    pub fn new(points: Vec<TimingPoint>) -> Self {
        assert!(points.len() >= 2, "a linear timing curve requires at least two points");
        assert!(points.first() == Some(&TimingPoint { time: 0.0, progress: 0.0 }) && points.last() == Some(&TimingPoint { time: 1.0, progress: 1.0 }), "linear timing anchors must be (0,0) and (1,1)");
        assert!(points.iter().all(|p| p.time.is_finite() && p.progress.is_finite()) && points.windows(2).all(|p| p[0].time < p[1].time), "linear timing points must be finite and strictly ordered"); Self { points }
    }
    pub fn evaluate(&self, time: f32) -> f32 {
        if time <= 0.0 { return 0.0; } if time >= 1.0 { return 1.0; }
        let mut lower = 0; let mut upper = self.points.len() - 1;
        while upper - lower > 1 { let middle = lower + (upper - lower) / 2; if self.points[middle].time <= time { lower = middle; } else { upper = middle; } }
        let first = self.points[lower]; let last = self.points[upper]; let fraction = (time as f64 - first.time as f64) / (last.time as f64 - first.time as f64);
        (first.progress as f64 + (last.progress as f64 - first.progress as f64) * fraction) as f32
    }
}
