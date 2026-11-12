//! Translation of `Tests/CoveUIComposeTests/Adaptive/CommandPaletteAdaptiveTests.swift`.
//!
//! Behaviour checklist:
//! - the compact palette uses a 16pt window inset and 44pt result rows
//! - the regular palette keeps the 640x560 desktop geometry and 40pt rows
//!
//! `CommandPalette::body` derives `compact` from the window width in its
//! `fitting_geometry` pass, matching Swift: a sub-600pt surface gets a centred
//! `width - 16` panel with an 8pt inset, while the desktop keeps 640x560 at a
//! 72pt top inset. Row heights follow Swift: the authored 40pt row is raised to
//! the 44pt control floor on touch. The panel carries no `commandPalette.panel`
//! debug name, so geometry is read from the modal dialog node instead.

use crate::support;

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::AnyView;
use cove_core::scene::{AccessibilityRole, Node, PlatformInputContext};
use cove_core::{Binding, Environment, InputCapabilities};
use cove_platform::{InputEvent, KeyEvent, KeyModifiers};
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::overlays::portal::{LayerRoot, PortalStore, PortalStoreAmbient};
use cove_ui_compose::primitives::overlays::{CommandPalette, PaletteCommand};
use cove_ui_compose::Scancode;

use support::GlobalTestLock;

const ROWS: [(&str, &str); 4] = [
    ("Toggle Theme", "View"),
    ("Go to File", "Navigate"),
    ("Go to Line", "Navigate"),
    ("Format Document", "Edit"),
];

fn nodes_of(root: &Node) -> Vec<Node> {
    let mut nodes = vec![root.clone()];
    for child in root.children() {
        nodes.extend(nodes_of(&child));
    }
    nodes
}

fn commands() -> Vec<PaletteCommand> {
    ROWS.iter()
        .map(|(title, group)| PaletteCommand::new(*title, || {}).group(*group))
        .collect()
}

/// Installs the palette the way the Swift `withPalette` helper does and hands
/// the settled node list to `body`.
fn with_palette(width: f32, height: f32, input: InputCapabilities, body: impl FnOnce(&[Node])) {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let context = PlatformInputContext::new();
            let store = Rc::new(PortalStore::new());
            context.add_scoped_ambient(Rc::new(PortalStoreAmbient::new(store)));
            context.with_current(|| {
                let mut environment = Environment::default();
                environment.viewport.width = width;
                environment.viewport.height = height;
                environment.input = input;

                let palette = CommandPalette::new(Binding::constant(true), commands());
                let layer = LayerRoot::new(palette);
                let sized = AnyView::new(layer).frame_size(width, height);
                let graph = support::graph();
                graph.install_view(
                    AnyView::new(AnyView::new(sized).surface_environment(environment)),
                    Default::default(),
                );
                for _ in 0..20 {
                    graph.recomposer.commit_all();
                    graph.compute_layout(width, height);
                    cove_core::scene::AnimatorScheduler::current().tick(0.1);
                }

                let root = graph.tree.borrow().root().expect("root node");
                body(&nodes_of(&root));
            });
        });
    });
}

/// The palette panel: the modal dialog surface the palette presents into.
fn panel(nodes: &[Node]) -> Node {
    nodes
        .iter()
        .filter(|node| {
            node.accessibility()
                .is_some_and(|semantics| semantics.role == AccessibilityRole::Dialog)
        })
        .max_by(|left, right| {
            (left.frame().width * left.frame().height)
                .partial_cmp(&(right.frame().width * right.frame().height))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .cloned()
        .expect("the palette panel materialises")
}

fn row(nodes: &[Node], label: &str) -> Option<Node> {
    nodes
        .iter()
        .find(|node| {
            node.accessibility().is_some_and(|semantics| {
                semantics.role == AccessibilityRole::Button && semantics.label == label
            })
        })
        .cloned()
}

#[test]
fn compact_palette_uses_the_surface_geometry() {
    with_palette(
        390.0,
        820.0,
        InputCapabilities::TOUCH | InputCapabilities::KEYBOARD,
        |nodes| {
            let panel = panel(nodes);
            let frame = panel.absolute_frame();
            assert!(
                (frame.width - 374.0).abs() < 1.0,
                "a 390pt surface gets Swift's width - 16 panel, got {}",
                frame.width
            );
            assert!(
                (frame.x - 8.0).abs() < 2.0,
                "the compact panel is centred with an 8pt margin, got x={}",
                frame.x
            );
            assert!(
                (frame.height - 560.0).abs() < 1.0,
                "the compact panel keeps the 560pt height cap, got {}",
                frame.height
            );

            let toggle = row(nodes, "Toggle Theme").expect("the first result row");
            assert_eq!(
                toggle.frame().height,
                44.0,
                "a touch surface raises the 40pt authored row to the 44pt control floor"
            );
            assert!(
                nodes.iter().any(|node| {
                    node.accessibility().is_some_and(|semantics| {
                        semantics.role == AccessibilityRole::StaticText && semantics.label == "View"
                    })
                }),
                "the group header materialises"
            );
            assert!(
                nodes.iter().any(|node| {
                    node.accessibility()
                        .is_some_and(|semantics| semantics.label == "Format Document")
                }),
                "every command is listed"
            );
        },
    );
}

#[test]
fn arrows_and_run_stay_within_the_filtered_results() {
    // A query that matches one command, an arrow that must not escape into a
    // filtered-out command, and Return must run only what the list shows.
    let ran = Rc::new(RefCell::new(Vec::<String>::new()));
    support::InteractiveScene::run(1280.0, 820.0, |scene| {
        let commands: Vec<PaletteCommand> = ROWS
            .iter()
            .map(|(title, group)| {
                let ran = ran.clone();
                let name = (*title).to_string();
                PaletteCommand::new(*title, move || ran.borrow_mut().push(name.clone())).group(*group)
            })
            .collect();
        scene.install(AnyView::new(LayerRoot::new(CommandPalette::new(
            Binding::constant(true),
            commands,
        ))));

        let field = scene
            .first(|node| {
                node.accessibility()
                    .is_some_and(|semantics| semantics.role == AccessibilityRole::TextField)
            })
            .expect("the palette mounts a search field");
        scene.focus.focus(Some(&field), true);
        let input = scene.dispatcher();
        input.dispatch(&InputEvent::TextInput("theme".into()));
        scene.settle(2, 0.1);
        // Only "Toggle Theme" survives the query, so an Arrow must stay on it.
        input.dispatch(&InputEvent::KeyDown(KeyEvent {
            scancode: Scancode::ARROW_DOWN,
            keycode: 0,
            modifiers: KeyModifiers::default(),
            is_repeat: false,
        }));
        scene.settle(2, 0.1);
        input.dispatch(&InputEvent::KeyDown(KeyEvent {
            scancode: Scancode::RETURN,
            keycode: 0,
            modifiers: KeyModifiers::default(),
            is_repeat: false,
        }));
        scene.settle(2, 0.1);

        assert_eq!(
            ran.borrow().as_slice(),
            ["Toggle Theme"],
            "Return runs the only visible match, never a query-hidden command"
        );
    });
}

#[test]
fn regular_palette_keeps_the_desktop_geometry() {
    with_palette(
        1280.0,
        820.0,
        InputCapabilities::POINTER | InputCapabilities::KEYBOARD,
        |nodes| {
            let panel = panel(nodes);
            assert!(
                (panel.frame().width - 640.0).abs() < 1.0,
                "the regular panel keeps the 640pt authored width, got {}",
                panel.frame().width
            );
            assert!(
                (panel.frame().height - 560.0).abs() < 1.0,
                "the regular panel keeps the 560pt authored height, got {}",
                panel.frame().height
            );

            let toggle = row(nodes, "Toggle Theme").expect("the first result row");
            assert_eq!(
                toggle.frame().height,
                CommandPalette::ROW_HEIGHT,
                "the pointer palette uses the 40pt authored row height"
            );
        },
    );
}
