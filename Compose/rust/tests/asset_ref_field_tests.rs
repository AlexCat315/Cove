//! Translation of `Tests/CoveUIComposeTests/AssetRefFieldTests.swift`.
//!
//! Behaviour checklist:
//! - kinds without a preview map to a glyph by keyword, case-insensitively
//! - the thumbnail slot draws a glyph, never the kind as truncated text
//! - `AssetDropRegistry` dispatches a matching payload to the target
//! - `AssetDropRegistry` rejects unsupported asset kinds
//!
//! Name mapping: `AssetKindTile.icon(for:)` is [`icon_for`] and
//! `AssetDropRegistry.drop(_:atX:y:)` is [`AssetDropRegistry::drop`] taking a
//! [`Point`]; both are straight renames.

mod support;

use std::collections::BTreeSet;
use std::rc::Rc;

use cove_composition::AnyView;
use cove_core::scene::Node;
use cove_core::{Binding, Point, State};
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::controls::fields::asset_ref_field::icon_resource;
use cove_ui_compose::primitives::controls::fields::icon_for;
use cove_ui_compose::primitives::controls::fields::{AssetRef, AssetRefField};
use cove_ui_compose::primitives::media::ui_icon_catalog::{editor, file, media};
use cove_ui_compose::primitives::overlays::asset_drop_target::{
    AssetDropPayload, AssetDropRegistry, AssetDropRegistryHolder,
};

use support::GlobalTestLock;

fn kinds(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn kind_glyphs() {
    assert_eq!(
        icon_resource(icon_for(Some("image"))),
        media::image(),
        "an image kind maps to the image glyph"
    );
    assert_eq!(
        icon_resource(icon_for(Some("Texture"))),
        media::image(),
        "keyword matching is case-insensitive"
    );
    assert_eq!(
        icon_resource(icon_for(Some("Static Mesh"))),
        editor::box_(),
        "a mesh kind maps to the box glyph"
    );
    assert_eq!(
        icon_resource(icon_for(Some("material"))),
        editor::brush(),
        "a material kind maps to the brush glyph"
    );
    assert_eq!(
        icon_resource(icon_for(Some("weird"))),
        file::file(),
        "an unrecognised kind falls back to the file glyph"
    );
    assert_eq!(
        icon_resource(icon_for(None)),
        file::file(),
        "a missing kind falls back to the file glyph"
    );
}

fn text_inputs(node: &Node, into: &mut Vec<String>) {
    if let Some(inputs) = node.layout_node().and_then(|layout| layout.text_inputs()) {
        into.push(inputs.text);
    }
    for child in node.children() {
        text_inputs(&child, into);
    }
}

fn texts(root: &Node) -> Vec<String> {
    let mut collected = Vec::new();
    text_inputs(root, &mut collected);
    collected
}

#[test]
fn thumbnail_is_a_glyph() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let graph = support::graph();
            for value in [
                None,
                Some(AssetRef::new("cam", "Camera", "image")),
            ] {
                graph.install_view(
                    AnyView::new(
                        AnyView::new(
                            AssetRefField::new(Binding::constant(value))
                                .accepted_kinds(kinds(&["image"])),
                        )
                        .frame_width(220.0),
                    ),
                    Default::default(),
                );
                graph.compute_layout(220.0, 80.0);
                let root = graph.tree.borrow().root().expect("root");
                let rendered = texts(&root);
                assert!(
                    !rendered.iter().any(|text| text == "image"),
                    "the kind is drawn as a glyph, never as text (saw {rendered:?})"
                );
            }
        });
    });
}

#[test]
fn registry_drop_dispatches_to_target() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let registry = Rc::new(AssetDropRegistry::new());
            AssetDropRegistryHolder::set_current(Some(registry.clone()));

            let value = State::new(None::<AssetRef>);
            let active = State::new(Some(
                AssetDropPayload::new("asset-1", "SM_Ruin_Arch_01", "mesh")
                    .subtitle("Content/Environment/Ruins")
                    .preview_path("/tmp/preview.png"),
            ));
            let binding = value.binding();
            let active_binding = active.binding();

            let graph = support::graph();
            graph.install_view(
                AnyView::new(
                    AnyView::new(
                        AssetRefField::new(binding)
                            .active_payload(active_binding)
                            .accepted_kinds(kinds(&["mesh"])),
                    )
                    .frame_size(220.0, 32.0),
                ),
                Default::default(),
            );
            graph.compute_layout(220.0, 32.0);

            let payload = active.get().expect("a payload is in flight");
            assert!(
                AssetDropRegistry::drop(&registry, &payload, Point::new(12.0, 12.0)),
                "a matching payload is accepted by the zone"
            );
            let stored = value.get().expect("the field stored a value");
            assert_eq!(stored.id, "asset-1", "the dropped id is stored");
            assert_eq!(stored.kind, "mesh", "the dropped kind is stored");
            assert_eq!(
                stored.preview_path.as_deref(),
                Some("/tmp/preview.png"),
                "the dropped preview path is stored"
            );

            AssetDropRegistryHolder::set_current(None);
        });
    });
}

#[test]
fn registry_rejects_unsupported_kind() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let registry = Rc::new(AssetDropRegistry::new());
            AssetDropRegistryHolder::set_current(Some(registry.clone()));

            let value = State::new(None::<AssetRef>);
            let payload = AssetDropPayload::new("mat-1", "M_Ruin", "material");
            let binding = value.binding();

            let graph = support::graph();
            graph.install_view(
                AnyView::new(
                    AnyView::new(
                        AssetRefField::new(binding)
                            .active_payload(Binding::constant(Some(payload.clone())))
                            .accepted_kinds(kinds(&["mesh"])),
                    )
                    .frame_size(220.0, 32.0),
                ),
                Default::default(),
            );
            graph.compute_layout(220.0, 32.0);

            assert!(
                !AssetDropRegistry::drop(&registry, &payload, Point::new(12.0, 12.0)),
                "a material payload is rejected by a mesh-only zone"
            );
            assert!(value.get().is_none(), "a rejected drop stores no value");

            AssetDropRegistryHolder::set_current(None);
        });
    });
}
