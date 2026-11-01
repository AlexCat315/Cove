//! Translation of `Tests/CoveUIComposeTests/OffscreenReuseTests.swift`.
//!
//! Behaviour checklist:
//! - the first render emits vertices and atlas uploads
//! - an unchanged render reuses the geometry and emits no atlas uploads
//! - changing the observed state rebuilds the text and changes vertex count
//! - after trimming memory, the rebuilt geometry keeps the same x positions

mod support;

use cove_core::State;
use cove_ui_compose::modifiers::layout::{FrameDimension, LayoutFrameExt};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::rendering::OffscreenView;
use cove_ui_compose::{AnyView, View};

use support::GlobalTestLock;

/// Mirror of the Swift `Content` view:
/// ```swift
/// struct Content: View {
///     @State var text: String
///     var body: some View { Text(text).frame(width: 100, height: 30) }
/// }
/// ```
///
/// `Content` is a plain `View` whose `@State` Swift auto-wires by reflection, so
/// mutating the state schedules a recompose. `impl_primitive_view!` provides no
/// `body` and no property discovery, so a primitive harness reading the state in
/// `children()` is never subscribed and `state.set(...)` silently invalidates
/// nothing — the text then renders identically and the vertex-count assertion
/// fails. Declaring the real `View`/`body` and publishing the state as a
/// dynamic property is what makes the third render actually rebuild the text.
#[derive(Clone)]
struct StateText {
    state: State<String>,
}

impl View for StateText {
    fn body(&self) -> AnyView {
        AnyView::new(Text::new(self.state.get()).frame(
            Some(FrameDimension::Points(100.0)),
            Some(FrameDimension::Points(30.0)),
            None,
            None,
            None,
            None,
        ))
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![cove_composition::DynamicField::state("text", &self.state)]
    }
}

#[test]
fn unchanged_snapshots_reuse_geometry_and_trim_rebuilds_it() {
    GlobalTestLock::locked(|| {
        let state = State::new("First".to_string());
        let mut view = OffscreenView::default();
        view.set_root_view(StateText {
            state: state.clone(),
        });

        let first = view.render(120, 50, 1.0).expect("first snapshot");
        assert!(
            !first.vertices.is_empty(),
            "the first render emits vertices"
        );
        assert!(
            !first.atlas_updates.is_empty(),
            "the first render uploads glyphs"
        );

        let second = view.render(120, 50, 1.0).expect("second snapshot");
        assert!(
            second.atlas_updates.is_empty(),
            "unchanged render has no atlas uploads"
        );
        let first_x: Vec<f32> = first.vertices.iter().map(|vertex| vertex.x).collect();
        let second_x: Vec<f32> = second.vertices.iter().map(|vertex| vertex.x).collect();
        assert_eq!(second_x, first_x, "unchanged geometry reuses x positions");

        state.set("Second value".to_string());
        let changed = view.render(120, 50, 1.0).expect("changed snapshot");
        assert_ne!(
            changed.vertices.len(),
            first.vertices.len(),
            "the rebuilt text changes the vertex count"
        );

        view.trim_memory();
        let restored = view.render(120, 50, 1.0).expect("restored snapshot");
        let changed_x: Vec<f32> = changed.vertices.iter().map(|vertex| vertex.x).collect();
        let restored_x: Vec<f32> = restored.vertices.iter().map(|vertex| vertex.x).collect();
        assert_eq!(
            restored_x, changed_x,
            "trimmed geometry keeps the rebuilt x positions"
        );
    });
}
