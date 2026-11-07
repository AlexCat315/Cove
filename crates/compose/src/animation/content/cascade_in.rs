//! Surfaces that arrive one after another.
//!
//! Every surface mounted in the same burst takes the next slot in a short
//! cascade: it fades in while settling from a hair below its size, on the
//! entrance ease-out. Mount order is tree order, so a page fills top to bottom
//! and each row left to right without measuring anything.
use std::cell::Cell;
use std::time::{Duration, Instant};

use cove_core::Point;
use cove_core::scene::{ActiveAnimationContext, Node};

use crate::theme::{SemanticMotionRef, theme_for_node};
use crate::{ModifiedContent, View, ViewExt, ViewModifier};

const STARTED: &str = "__cascade_in.started";
const PROGRESS_KEY: &str = "cascadeIn.progress";
/// Mounts closer together than this belong to one cascade.
const BURST_WINDOW: Duration = Duration::from_millis(160);
const STEP: f64 = 0.03;
const MAX_DELAY: f64 = 0.18;
const START_SCALE: f32 = 0.985;

thread_local! {
    static BURST: Cell<Option<(Instant, u32)>> = const { Cell::new(None) };
}

fn next_slot() -> u32 {
    BURST.with(|burst| {
        let now = Instant::now();
        let slot = match burst.get() {
            Some((last, slot)) if now.duration_since(last) < BURST_WINDOW => slot + 1,
            _ => 0,
        };
        burst.set(Some((now, slot)));
        slot
    })
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CascadeIn;

impl ViewModifier for CascadeIn {
    fn apply_node(&self, node: &Node) {
        if node.attachment::<bool>(STARTED).is_some() {
            return;
        }
        node.set_attachment(STARTED, true);
        if node.prefers_reduced_motion() {
            return;
        }
        let delay = (next_slot() as f64 * STEP).min(MAX_DELAY);
        apply(node, 0.0);
        let mut motion = SemanticMotionRef::entering().resolve(&theme_for_node(node));
        motion.delay = delay;
        let owner = node.downgrade();
        ActiveAnimationContext::with(Some(motion), None, None, || {
            node.animatable_set_with(PROGRESS_KEY, 0.0_f32, 1.0_f32, move |value| {
                if let Some(owner) = Node::upgrade(&owner) {
                    apply(&owner, value);
                }
            });
        });
    }
}

fn apply(node: &Node, progress: f32) {
    node.set_opacity(progress.clamp(0.0, 1.0));
    let mut transform = node.transform();
    transform.anchor = Point::new(0.5, 1.0);
    transform.scales_shadow = false;
    transform.scale = START_SCALE + (1.0 - START_SCALE) * progress;
    node.set_transform(transform);
}

pub trait CascadeInExt: View + Sized + 'static {
    /// Join the current mount cascade.
    fn cascade_in(self) -> ModifiedContent {
        self.modifier(CascadeIn)
    }
}
impl<T: View + Sized + 'static> CascadeInExt for T {}
