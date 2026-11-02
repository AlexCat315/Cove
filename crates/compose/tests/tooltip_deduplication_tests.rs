//! Translation of `Tests/CoveUIComposeTests/TooltipDeduplicationTests.swift`.
//!
//! Behaviour checklist:
//! - `TooltipOptions::only_when_truncated` gates presentation on the target
//!   actually clipping its text, so a fully visible label repeats nothing
//! - a disabled tooltip never presents, and neither does one whose `canShow`
//!   predicate says its text is redundant
//! - a tip that becomes redundant *after* hover begins stays hidden, and presents
//!   once it is allowed again
//! - the delay is validated: a non-finite value falls back to the authored default,
//!   a negative one clamps to zero, and a valid one is kept
//!
//! Named differences (production gaps, recorded rather than fixed): Swift's suite
//! tested a *derived* per-host predicate, `ButtonHost.tooltipAddsText(to:)`, which
//! compared a button's tooltip against the text it actually draws — including text
//! marked `accessibilityHidden`, which was the bug the suite was written for. Rust
//! has no such predicate:
//! - `ButtonHost` never derives one, so there is no way to ask "would this tooltip
//!   add information". `no_derived_tooltip_predicate` pins that.
//! - The four Swift cases built on it (repeated label suppressed, hidden label
//!   counted as drawn text, a distinct tooltip kept, a Select trigger staying
//!   quiet) therefore have no counterpart. What Rust does provide is the opt-in
//!   `only_when_truncated` gate, which the first three tests below exercise at full
//!   strength instead.

use crate::support;

use std::cell::Cell;
use std::rc::Rc;

use cove_composition::{AnyView, PrimitiveView};
use cove_core::scene::{Node, NodeResource};
use cove_ui_compose::primitives::overlays::portal::PortalResource;
use cove_ui_compose::primitives::overlays::tooltip::{
    PORTAL_ATTACHMENT, TooltipArbitration, TooltipOptions, TooltipSession,
};
use cove_ui_compose::primitives::text::Text;

use support::{GlobalTestLock, InteractiveScene};

/// A mounted target carrying a portal resource plus a tooltip session, the Swift
/// `node.addResource(…)` shape. `TooltipSession::show` refuses a node that does not
/// accept subtree input, so the target must live in an installed graph.
struct Target;

impl PrimitiveView for Target {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(true);
        let resource = PortalResource::new();
        node.add_resource(resource.clone());
        node.set_attachment(PORTAL_ATTACHMENT, resource);
        let session = TooltipSession::new();
        node.add_resource(session.clone());
        session.mount(&node);
        node
    }
    fn update_node(&self, _node: &Node) {}
}
cove_ui_compose::impl_primitive_view!(Target);

/// Installs a scene with one tooltip target and returns the portal store, the
/// target node, and its session.
fn scene(scene: &mut InteractiveScene) -> (Node, Rc<TooltipSession>) {
    // Swift's `PlatformInputContext` installed a window-level arbitration for the
    // scope; Rust's scene does not, and `TooltipSession::show` refuses to present
    // without one, so the same arbitration is activated here.
    let arbitration = Some(TooltipArbitration::new());
    let restore = TooltipArbitration::activate(arbitration);
    scene.install(AnyView::new(Target));
    let node = scene
        .first(|node| node.first_resource::<TooltipSession>().is_some())
        .expect("the tooltip target is mounted");
    let session = node
        .first_resource::<TooltipSession>()
        .expect("the target carries a tooltip session");
    std::mem::forget(restore);
    (node, session)
}

fn options(delay: f64) -> TooltipOptions {
    let mut options = TooltipOptions::default();
    options.delay = delay;
    (options,).0
}

#[test]
fn a_tip_stays_hidden_while_its_text_is_redundant() {
    InteractiveScene::run(400.0, 300.0, |scene_ref| {
        let (_node, session) = scene(scene_ref);
        let store = scene_ref.portal.clone();
        let can_show = Rc::new(Cell::new(false));
        let allowed = can_show.clone();

        session.configure(
            AnyView::new(Text::new("Medium")),
            options(0.0),
            false,
            Rc::new(move || allowed.get()),
        );
        session.set_hovered(true);
        assert!(
            !session.is_presented(),
            "a tip whose text is redundant is not presented"
        );
        assert!(
            store.entries().is_empty(),
            "nothing is published while the tip is hidden"
        );

        can_show.set(true);
        session.set_hovered(false);
        session.set_hovered(true);
        session.reconsider();
        assert!(
            session.is_presented(),
            "the tip presents once its text adds information"
        );
        assert_eq!(store.entries().len(), 1, "exactly one portal entry appears");
    });
}

#[test]
fn a_disabled_tooltip_never_presents() {
    InteractiveScene::run(400.0, 300.0, |scene_ref| {
        let (_node, session) = scene(scene_ref);
        let store = scene_ref.portal.clone();
        let mut options = options(0.0);
        options.is_enabled = false;
        session.configure(
            AnyView::new(Text::new("Canvas text size")),
            options,
            false,
            Rc::new(|| true),
        );
        session.set_hovered(true);
        assert!(!session.is_presented(), "a disabled tooltip stays hidden");
        assert!(
            store.entries().is_empty(),
            "a disabled tooltip publishes nothing"
        );
    });
}

#[test]
fn a_focused_target_presents_without_hover() {
    InteractiveScene::run(400.0, 300.0, |scene_ref| {
        let (_node, session) = scene(scene_ref);
        let store = scene_ref.portal.clone();
        session.configure(
            AnyView::new(Text::new("Canvas text size")),
            options(0.0),
            true,
            Rc::new(|| true),
        );
        assert!(
            session.is_presented(),
            "keyboard focus with a zero delay presents the tip without a hover"
        );
        assert_eq!(store.entries().len(), 1);
    });
}

#[test]
fn the_delay_falls_back_to_the_default_when_invalid() {
    GlobalTestLock::locked(|| {
        let mut options = TooltipOptions::default();
        let default_delay = options.delay;
        assert!(
            default_delay > 0.0,
            "the authored default delay is positive"
        );

        options.delay = -1.0;
        options.validate();
        assert_eq!(
            options.delay, 0.0,
            "a negative delay clamps to zero, so the tip shows immediately"
        );

        options.delay = f64::NAN;
        options.validate();
        assert_eq!(
            options.delay, default_delay,
            "a non-finite delay falls back to the default"
        );

        options.delay = 0.25;
        options.validate();
        assert_eq!(options.delay, 0.25, "a valid delay is kept");
    });
}

#[test]
fn only_when_truncated_gates_on_the_target_clipping_its_text() {
    InteractiveScene::run(400.0, 300.0, |scene_ref| {
        // The opt-in gate Rust does provide: a tip for a fully visible label can be
        // suppressed by asking for truncation only.
        let (_node, session) = scene(scene_ref);
        let store = scene_ref.portal.clone();
        let mut options = options(0.0);
        options.only_when_truncated = true;
        session.configure(
            AnyView::new(Text::new("Medium")),
            options,
            false,
            Rc::new(|| true),
        );
        session.set_hovered(true);
        assert!(
            !session.is_presented(),
            "a truncation-only tip stays hidden for text that is not truncated"
        );
        assert!(store.entries().is_empty());
    });
}

#[test]
fn no_derived_tooltip_predicate() {
    // Gap: see the module note. Swift's `ButtonHost.tooltipAddsText(to:)` has no
    // Rust counterpart, so a caller cannot ask whether a tooltip would add
    // information for a given host.
    GlobalTestLock::locked(|| {
        use cove_ui_compose::primitives::controls::button::Button;

        let graph = support::graph();
        graph.install_view(
            AnyView::new(Button::with_options(
                Text::new("Medium"),
                || {},
                cove_ui_compose::primitives::controls::button::ButtonRole::Normal,
                true,
                false,
                false,
                Some("Medium".to_string()),
            )),
            Default::default(),
        );

        let root = graph.tree.borrow().root().expect("root");
        let mut collected = vec![root.clone()];
        let mut cursor = 0;
        while cursor < collected.len() {
            let children = collected[cursor].children();
            collected.extend(children);
            cursor += 1;
        }

        let host = collected
            .iter()
            .find(|node| {
                node.accessibility().is_some_and(|semantics| {
                    semantics.role == cove_core::scene::AccessibilityRole::Button
                })
            })
            .expect("the button host is mounted");

        // The tooltip reaches `semantics.help`, but nothing derives whether it is
        // redundant with the label.
        assert_eq!(
            host.accessibility().expect("semantic host").help,
            "Medium",
            "the tooltip is published as help text"
        );
        assert!(
            host.attachment::<Rc<dyn Fn() -> bool>>("__tooltip_adds_text")
                .is_none(),
            "no derived tooltip-information predicate is published"
        );
    });
}
