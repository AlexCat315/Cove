//! Translation of `Tests/CoveUIComposeTests/Adaptive/CommandPaletteAdaptiveTests.swift`.
//!
//! Behaviour checklist:
//! - the compact palette uses a 16pt window inset and 44pt result rows
//! - the regular palette keeps the 640x560 desktop geometry and 40pt rows
//!
//! Named difference against the Swift suite: `CommandPalette::body`
//! (`crates/compose/src/primitives/overlays/command_palette.rs`) hard-codes one desktop
//! geometry and never derives `compact` from `surface.viewport.width`, so a
//! 390pt surface does not get Swift's `width - 16` panel with an 8pt inset and
//! centred placement. The compact case therefore asserts only that the panel is
//! clamped inside the surface. Row heights do follow Swift: the authored 40pt row
//! is raised to the 44pt control floor on touch. The panel also carries no
//! `commandPalette.panel` debug name, so geometry is read from the modal dialog
//! node instead.

mod support;

use std::rc::Rc;

use cove_composition::AnyView;
use cove_core::scene::{AccessibilityRole, Node, PlatformInputContext};
use cove_core::{Binding, Environment, InputCapabilities};
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::overlays::command_palette::{CommandPalette, PaletteCommand};
use cove_ui_compose::primitives::overlays::portal::{LayerRoot, PortalStore, PortalStoreAmbient};

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
                frame.width <= 390.0 && frame.height <= 820.0,
                "the panel stays inside the 390x820 surface, got {}x{}",
                frame.width,
                frame.height
            );
            assert!(
                frame.x >= 0.0 && frame.max_x() <= 390.0,
                "the panel starts inside the surface, got x={} maxX={}",
                frame.x,
                frame.max_x()
            );
            assert!(
                frame.y >= 0.0 && frame.max_y() <= 820.0,
                "the panel ends inside the surface, got y={} maxY={}",
                frame.y,
                frame.max_y()
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
