//! Translation of CoveUIScene/Input/WheelRoutingAttachment.swift.
use crate::scene::{EventResult, Node};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WheelRoutingPriority {
    /// Prefer the focused editor while it contains the pointer, or when no
    /// pointer position is available. A phased stream retains its consumer.
    PreferFocused,
    /// A transient overlay under the pointer takes precedence over a text
    /// editor retaining focus behind it. May be attached to an ancestor.
    PreferHit,
}

/// Controls whether a wheel handler should consume an event after applying
/// scroll math.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollConsumePolicy {
    /// Consume any wheel delta routed to this node, even if clamping prevents a
    /// visible offset change.
    Always,
    /// Consume input for an overflowing axis, including at its boundary.
    WhenScrollable,
    /// Consume only when the effective scroll offset changed.
    WhenOffsetChanged,
    /// Never consume. Useful for diagnostics or explicit pass-through.
    Never,
}
impl ScrollConsumePolicy {
    pub fn result(self, did_scroll: bool, can_scroll: bool) -> EventResult {
        match self {
            Self::Always => EventResult::Handled,
            Self::WhenScrollable => {
                if can_scroll {
                    EventResult::Handled
                } else {
                    EventResult::Ignored
                }
            }
            Self::WhenOffsetChanged => {
                if did_scroll {
                    EventResult::Handled
                } else {
                    EventResult::Ignored
                }
            }
            Self::Never => EventResult::Ignored,
        }
    }
}

pub struct WheelRoutingAttachmentKey;
impl WheelRoutingAttachmentKey {
    /// `Node.attachments` entry carrying the wheel-routing priority for a node.
    pub const PRIORITY: &'static str = "__wheel_routing_priority";
}

impl Node {
    pub fn wheel_routing_priority(&self) -> Option<WheelRoutingPriority> {
        self.attachment(WheelRoutingAttachmentKey::PRIORITY)
    }
    pub fn set_wheel_routing_priority(&self, priority: Option<WheelRoutingPriority>) {
        match priority {
            Some(priority) => self.set_attachment(WheelRoutingAttachmentKey::PRIORITY, priority),
            None => self.remove_attachment(WheelRoutingAttachmentKey::PRIORITY),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consume_policy_maps_to_event_results() {
        assert_eq!(ScrollConsumePolicy::Always.result(false, false), EventResult::Handled);
        assert_eq!(ScrollConsumePolicy::WhenScrollable.result(true, false), EventResult::Ignored);
        assert_eq!(ScrollConsumePolicy::WhenOffsetChanged.result(true, true), EventResult::Handled);
        assert_eq!(ScrollConsumePolicy::Never.result(true, true), EventResult::Ignored);
    }

    #[test]
    fn priority_round_trips_through_attachments() {
        let node = Node::new();
        assert_eq!(node.wheel_routing_priority(), None);
        node.set_wheel_routing_priority(Some(WheelRoutingPriority::PreferHit));
        assert_eq!(node.wheel_routing_priority(), Some(WheelRoutingPriority::PreferHit));
        node.set_wheel_routing_priority(None);
        assert_eq!(node.wheel_routing_priority(), None);
    }
}
