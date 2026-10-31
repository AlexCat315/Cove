//! Composable insertion and removal effects from Animation/Transition.swift.
use cove_core::Point;
use cove_core::scene::NodeTransform;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransitionEdge {
    Top,
    Bottom,
    Leading,
    Trailing,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TransitionEffect {
    pub opacity: f32,
    pub x: f32,
    pub y: f32,
    pub scale: NodeTransform,
    pub collapse: bool,
}

impl Default for TransitionEffect {
    fn default() -> Self {
        Self {
            opacity: 1.0,
            x: 0.0,
            y: 0.0,
            scale: NodeTransform::IDENTITY,
            collapse: false,
        }
    }
}

impl TransitionEffect {
    fn combined(self, other: Self) -> Self {
        assert!(
            self.scale.is_identity()
                || other.scale.is_identity()
                || self.scale.anchor == other.scale.anchor,
            "combine scale transitions with the same anchor; nest visibility containers for different anchors",
        );
        let scale = if self.scale.is_identity() {
            other.scale
        } else if other.scale.is_identity() {
            self.scale
        } else {
            NodeTransform::new(self.scale.scale * other.scale.scale, self.scale.anchor)
                .with_scales_shadow(false)
        };
        Self {
            opacity: self.opacity * other.opacity,
            x: self.x + other.x,
            y: self.y + other.y,
            scale,
            collapse: self.collapse || other.collapse,
        }
    }

    pub(crate) fn interpolating(self, goal: Self, amount: f32) -> Self {
        if amount == 0.0 {
            return self;
        }
        if amount == 1.0 {
            return goal;
        }
        let anchor = if goal.scale.is_identity() {
            self.scale.anchor
        } else if self.scale.is_identity() {
            goal.scale.anchor
        } else {
            Point::new(
                self.scale.anchor.x + (goal.scale.anchor.x - self.scale.anchor.x) * amount,
                self.scale.anchor.y + (goal.scale.anchor.y - self.scale.anchor.y) * amount,
            )
        };
        Self {
            opacity: self.opacity + (goal.opacity - self.opacity) * amount,
            x: self.x + (goal.x - self.x) * amount,
            y: self.y + (goal.y - self.y) * amount,
            scale: NodeTransform::new(
                self.scale.scale + (goal.scale.scale - self.scale.scale) * amount,
                anchor,
            )
            .with_scales_shadow(false),
            collapse: goal.collapse,
        }
    }
}

/// A pair of effects: one for insertion and one for removal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transition {
    pub(crate) insertion: TransitionEffect,
    pub(crate) removal: TransitionEffect,
}

impl Default for Transition {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Transition {
    pub const IDENTITY: Self = Self {
        insertion: TransitionEffect {
            opacity: 1.0,
            x: 0.0,
            y: 0.0,
            scale: NodeTransform::IDENTITY,
            collapse: false,
        },
        removal: TransitionEffect {
            opacity: 1.0,
            x: 0.0,
            y: 0.0,
            scale: NodeTransform::IDENTITY,
            collapse: false,
        },
    };

    pub const fn identity() -> Self {
        Self::IDENTITY
    }

    pub fn opacity() -> Self {
        Self::from_effect(TransitionEffect {
            opacity: 0.0,
            ..TransitionEffect::default()
        })
    }

    pub fn collapse() -> Self {
        Self::from_effect(TransitionEffect {
            collapse: true,
            ..TransitionEffect::default()
        })
    }

    pub fn offset(x: f32, y: f32) -> Self {
        Self::from_effect(TransitionEffect {
            x,
            y,
            ..TransitionEffect::default()
        })
    }

    pub fn move_edge(edge: TransitionEdge, distance: f32) -> Self {
        let distance = if distance.is_finite() { distance } else { 0.0 };
        Self::from_effect(match edge {
            TransitionEdge::Top => TransitionEffect {
                y: -distance,
                ..TransitionEffect::default()
            },
            TransitionEdge::Bottom => TransitionEffect {
                y: distance,
                ..TransitionEffect::default()
            },
            TransitionEdge::Leading => TransitionEffect {
                x: -distance,
                ..TransitionEffect::default()
            },
            TransitionEdge::Trailing => TransitionEffect {
                x: distance,
                ..TransitionEffect::default()
            },
        })
    }

    pub fn move_from_edge(edge: TransitionEdge) -> Self {
        Self::move_edge(edge, 16.0)
    }

    pub fn navigation(distance: f32, forward: bool) -> Self {
        let distance = if distance.is_finite() {
            distance.max(0.0)
        } else {
            0.0
        };
        let insertion = TransitionEffect {
            opacity: 0.0,
            x: if forward { distance } else { -distance },
            ..TransitionEffect::default()
        };
        let removal = TransitionEffect {
            opacity: 0.0,
            x: if forward { -distance } else { distance },
            ..TransitionEffect::default()
        };
        Self { insertion, removal }
    }

    pub fn scale(factor: f32, anchor: Point) -> Self {
        let factor = if factor.is_finite() {
            factor.max(0.0)
        } else {
            1.0
        };
        let anchor = Point::new(
            if anchor.x.is_finite() { anchor.x } else { 0.0 },
            if anchor.y.is_finite() { anchor.y } else { 0.0 },
        );
        Self::from_effect(TransitionEffect {
            scale: NodeTransform::new(factor, anchor).with_scales_shadow(false),
            ..TransitionEffect::default()
        })
    }

    pub fn combined(self, other: Self) -> Self {
        Self {
            insertion: self.insertion.combined(other.insertion),
            removal: self.removal.combined(other.removal),
        }
    }

    pub fn asymmetric(insertion: Self, removal: Self) -> Self {
        Self {
            insertion: insertion.insertion,
            removal: removal.removal,
        }
    }

    fn from_effect(effect: TransitionEffect) -> Self {
        Self {
            insertion: effect,
            removal: effect,
        }
    }
}
