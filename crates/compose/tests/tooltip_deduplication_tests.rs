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
//! Swift's suite tested a *derived* per-host predicate, `ButtonHost.tooltipAddsText(to:)`,
//! which compared a button's tooltip against the text it actually draws — including text
//! marked `accessibilityHidden`, the bug the suite was written for. Rust implements the
//! same predicate as `ButtonHost::tooltip_adds_text` and wires it into the tooltip session
//! as the `canShow` closure (`Button::update_tooltip`). The predicate is private, so these
//! tests observe it through the public `TooltipSession::is_active()`, whose final term *is*
//! that closure for an enabled, hovered button with the default options. The four Swift
//! cases are reproduced below — repeated label suppressed, hidden label counted as drawn
//! text, a distinct tooltip kept, a Select trigger staying quiet — plus the truncation
//! branch Rust adds: a clipped label presents its matching tooltip because the ellipsis
//! hides information the pointer user still needs.

use crate::support;

use std::cell::Cell;
use std::rc::Rc;

use cove_composition::{AnyView, PrimitiveView};
use cove_core::Binding;
use cove_core::scene::{AccessibilityRole, Node, NodeResource};
use cove_ui_compose::modifiers::interaction::AccessibilityExt;
use cove_ui_compose::primitives::controls::Button;
use cove_ui_compose::primitives::controls::fields::select::{Select, SelectOption};
use cove_ui_compose::primitives::overlays::portal::PortalResource;
use cove_ui_compose::primitives::overlays::tooltip::{
    PORTAL_ATTACHMENT, TEXT_TRUNCATED, TooltipArbitration, TooltipOptions, TooltipSession,
};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::styles::buttons::builtin_button_chrome::BUTTON_HOST_MARKER_KEY;

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

/// Mounts a button and returns its host node and tooltip session. The host carries
/// the button marker; `Button::update_tooltip` installs the session on that same node
/// and wires `tooltip_adds_text` as its `canShow` predicate.
fn mount_button(scene: &mut InteractiveScene, button: Button) -> (Node, Rc<TooltipSession>) {
    scene.install(AnyView::new(button));
    let host = scene
        .first(|node| node.attachment::<bool>(BUTTON_HOST_MARKER_KEY).is_some())
        .expect("the button host is mounted");
    let session = host
        .first_resource::<TooltipSession>()
        .expect("a button with a tooltip carries a session");
    (host, session)
}

#[test]
fn a_button_tooltip_repeating_its_label_is_suppressed() {
    InteractiveScene::run(320.0, 60.0, |scene| {
        let (_host, session) =
            mount_button(scene, Button::title("Medium", || {}).tooltip("Medium"));
        session.set_hovered(true);
        assert!(
            !session.is_active(),
            "a tooltip equal to the drawn label adds nothing, so it stays hidden"
        );
        scene.settle(2, 1.0);
        assert!(!session.is_presented(), "the redundant tip never presents");
        assert!(scene.portal.entries().is_empty(), "nothing is published");
    });
}

#[test]
fn a_hidden_label_still_counts_as_drawn_text() {
    InteractiveScene::run(320.0, 60.0, |scene| {
        // The bug Swift's suite was written for: a label hidden from accessibility is
        // still drawn, so a tooltip repeating it must stay hidden.
        let (_host, session) = mount_button(
            scene,
            Button::new(Text::new("Medium").accessibility_hidden(true), || {}).tooltip("Medium"),
        );
        session.set_hovered(true);
        assert!(
            !session.is_active(),
            "drawn text includes accessibility-hidden labels, so the tip is redundant"
        );
    });
}

#[test]
fn a_distinct_button_tooltip_is_kept() {
    InteractiveScene::run(320.0, 60.0, |scene| {
        let (_host, session) = mount_button(
            scene,
            Button::title("Medium", || {}).tooltip("Canvas text size"),
        );
        session.set_hovered(true);
        assert!(
            session.is_active(),
            "a tooltip that adds information survives the dedup predicate"
        );
        scene.settle(2, 1.0);
        assert!(
            session.is_presented(),
            "the distinct tip presents once its delay elapses"
        );
        let entries = scene.portal.entries();
        assert_eq!(entries.len(), 1, "exactly one tooltip chip appears");
        assert!(entries[0].is_tooltip, "the published entry is a tooltip");
    });
}

#[test]
fn a_truncated_label_presents_its_matching_tooltip() {
    InteractiveScene::run(320.0, 60.0, |scene| {
        let (_host, session) =
            mount_button(scene, Button::title("Medium", || {}).tooltip("Medium"));
        // The painter marks a clipped label while drawing; simulate that mark so the
        // predicate's truncation branch is observable without a paint pass.
        let label = scene
            .labelled("Medium", Some(AccessibilityRole::StaticText))
            .expect("the button draws its title");
        label.set_attachment(TEXT_TRUNCATED, true);
        session.set_hovered(true);
        assert!(
            session.is_active(),
            "an ellipsized label hides information, so its matching tooltip presents"
        );
    });
}

#[test]
fn a_select_trigger_does_not_repeat_its_selected_value() {
    InteractiveScene::run(200.0, 60.0, |scene| {
        scene.install(AnyView::new(
            Select::new(Binding::constant(1), vec![SelectOption::new(1, "Medium", true)])
                .width(160.0),
        ));
        let host = scene
            .first(|node| {
                node.accessibility()
                    .is_some_and(|semantics| semantics.role == AccessibilityRole::ComboBox)
            })
            .expect("the select trigger is mounted");
        // The selected value is the trigger's name and value, never help: repeating it
        // as help would make the system describe it twice.
        assert_eq!(host.accessibility().expect("semantic host").help, "");
        assert!(
            host.first_resource::<TooltipSession>().is_none(),
            "a select trigger installs no tooltip session"
        );
    });
}
