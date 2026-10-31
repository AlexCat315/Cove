//! Declarative sprite and motion model for the software pet.
use cove_core::Point;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PetPose {
    Idle,
    Windup,
    Rise,
    Apex,
    Fall,
    Land,
    Sit,
}
impl PetPose {
    pub const ALL: [Self; 7] = [
        Self::Idle,
        Self::Windup,
        Self::Rise,
        Self::Apex,
        Self::Fall,
        Self::Land,
        Self::Sit,
    ];

    fn fallback_chain(self) -> &'static [Self] {
        match self {
            Self::Rise | Self::Fall => &[Self::Apex, Self::Idle],
            Self::Windup | Self::Land => &[Self::Sit, Self::Idle],
            Self::Apex => &[Self::Idle],
            Self::Idle | Self::Sit => &[Self::Idle],
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PetSpriteSheet {
    pub display_size: f32,
    pub frames: HashMap<PetPose, String>,
}
impl PetSpriteSheet {
    pub fn new(display_size: f32, frames: HashMap<PetPose, String>) -> Self {
        Self {
            display_size,
            frames,
        }
    }
    pub fn path(&self, pose: PetPose) -> Option<&str> {
        self.frames.get(&pose).map(String::as_str)
    }
    pub fn resolved_path(&self, pose: PetPose) -> Option<&str> {
        self.path(pose).or_else(|| {
            pose.fallback_chain()
                .iter()
                .find_map(|candidate| self.path(*candidate))
        })
    }
}

/// A ballistic parabola in window space, with peak height measured above the
/// straight chord from the starting perch to the destination.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JumpArc {
    pub start: Point,
    pub end: Point,
    pub peak_height: f32,
}
impl JumpArc {
    pub fn new(start: Point, end: Point, peak_height: f32) -> Self {
        Self {
            start,
            end,
            peak_height,
        }
    }
    pub fn position(self, progress: f32) -> Point {
        let progress = clamp01(progress);
        Point::new(
            self.start.x + (self.end.x - self.start.x) * progress,
            self.start.y + (self.end.y - self.start.y) * progress
                - 4.0 * self.peak_height * progress * (1.0 - progress),
        )
    }
    /// Vertical velocity with respect to normalized air progress. Negative is
    /// ascending in the window coordinate system; positive is descending.
    pub fn vertical_velocity(self, progress: f32) -> f32 {
        (self.end.y - self.start.y) - 4.0 * self.peak_height * (1.0 - 2.0 * progress)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PetTrip {
    pub from_anchor: String,
    pub to_anchor: String,
    pub arc: JumpArc,
    pub total_duration: f64,
    pub facing: f32,
}
impl PetTrip {
    pub const WINDUP_END: f32 = 0.16;
    pub const AIRBORNE_END: f32 = 0.72;
    pub const LAND_END: f32 = 0.84;

    pub fn new(
        from_anchor: impl Into<String>,
        to_anchor: impl Into<String>,
        arc: JumpArc,
        total_duration: f64,
        facing: f32,
    ) -> Self {
        Self {
            from_anchor: from_anchor.into(),
            to_anchor: to_anchor.into(),
            arc,
            total_duration,
            facing,
        }
    }
    pub fn air_progress(&self, time: f32) -> Option<f32> {
        if !(Self::WINDUP_END..=Self::AIRBORNE_END).contains(&time) {
            return None;
        }
        Some(clamp01(
            (time - Self::WINDUP_END) / (Self::AIRBORNE_END - Self::WINDUP_END),
        ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PetRenderState {
    pub position: Point,
    pub pose: PetPose,
    pub scale_x: f32,
    pub scale_y: f32,
    pub facing: f32,
    pub presence: f32,
}
impl PetRenderState {
    pub fn new(
        position: Point,
        pose: PetPose,
        scale_x: f32,
        scale_y: f32,
        facing: f32,
        presence: f32,
    ) -> Self {
        Self {
            position,
            pose,
            scale_x,
            scale_y,
            facing,
            presence,
        }
    }
}

#[inline]
pub fn clamp01(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}
#[inline]
pub fn lerp(start: f32, end: f32, progress: f32) -> f32 {
    start + (end - start) * progress
}
