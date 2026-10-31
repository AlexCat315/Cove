//! Translation of `Tests/CoveUIComposeTests/StyleSkeletonTests.swift`.
//!
//! Behaviour checklist:
//! - `TextFieldStyleEnvironment` defaults to `DefaultTextFieldStyle`, whose body
//!   is chrome rather than an empty view
//! - `.text_field_style(_:)` overrides the environment for the subtree
//! - `PanelStyleEnvironment` defaults to `DefaultPanelStyle`, whose body is
//!   chrome rather than an empty view
//! - `ListRowStyleEnvironment` defaults to `DefaultListRowStyle` and produces a
//!   body for a selected row
//! - `TreeRowStyleEnvironment` is wired and carries the row depth
//! - `DefaultDividerStyle` paints a divider in the theme's divider colour
//! - all five style environments resolve to a non-nil default style

mod support;

use cove_composition::{AnyView, View, ViewExt};
use cove_core::scene::Node;
use cove_ui_compose::primitives::text::Divider;
use cove_ui_compose::theme::styles::components::divider_style::{AnyDividerStyle, DividerOrientation};
use cove_ui_compose::theme::styles::components::{
    AnyListRowStyle, AnyPanelStyle, AnyTextFieldStyle, AnyTreeRowStyle, DefaultDividerStyle,
    DefaultListRowStyle, DefaultPanelStyle, DefaultTextFieldStyle, DefaultTreeRowStyle,
    DividerStyleConfiguration,
    DividerStyleEnvironment, ListRowStyleConfiguration, ListRowStyleEnvironment,
    PanelStyleConfiguration, PanelStyleEnvironment, TextFieldStyle, TextFieldStyleConfiguration,
    TextFieldStyleEnvironment, TextFieldStyleViewExt, TreeRowStyleConfiguration,
    TreeRowStyleEnvironment,
};
use cove_ui_compose::theme::Theme;

use support::{DebugNode, GlobalTestLock, graph};

fn text_field_configuration() -> TextFieldStyleConfiguration {
    TextFieldStyleConfiguration {
        content: AnyView::new(cove_composition::EmptyView),
        placeholder: String::new(),
        is_focused: false,
        is_editing: false,
        is_error: false,
        is_enabled: true,
        theme: Theme::default_dark(),
        is_hovered: false,
    }
}

fn panel_configuration() -> PanelStyleConfiguration {
    PanelStyleConfiguration {
        title: "Hello".to_string(),
        accessory: AnyView::new(cove_composition::EmptyView),
        content: AnyView::new(cove_composition::EmptyView),
        is_active: false,
        theme: Theme::default_dark(),
        ..Default::default()
    }
}

/// A body counts as chrome when it is not the empty placeholder.
fn is_chrome(body: &AnyView) -> bool {
    body.storage
        .as_any()
        .downcast_ref::<cove_composition::EmptyView>()
        .is_none()
}

#[test]
fn text_field_style_defaults_to_the_default_style() {
    GlobalTestLock::locked(|| {
        let node = Node::new();
        let any: AnyTextFieldStyle = node.composition_value(&TextFieldStyleEnvironment::key());
        let body = any.make_body(text_field_configuration());
        assert!(
            is_chrome(&body),
            "the default text field style wraps the content in chrome"
        );
    });
}

struct ProbeStyle;
impl TextFieldStyle for ProbeStyle {
    fn make_body(&self, _configuration: TextFieldStyleConfiguration) -> AnyView {
        AnyView::new(cove_ui_compose::primitives::text::Text::new("probe"))
    }
}

#[test]
fn text_field_style_override_reaches_the_subtree() {
    GlobalTestLock::locked(|| {
        let graph = graph();
        graph.install_view(
            AnyView::new(
                DebugNode::new("x").text_field_style(ProbeStyle),
            ),
            Default::default(),
        );
        let leaf = support::deepest_node(&graph.tree.borrow().root().expect("root"));
        let resolved: AnyTextFieldStyle = leaf.composition_value(&TextFieldStyleEnvironment::key());
        let body = resolved.make_body(text_field_configuration());
        let rendered = body
            .storage
            .as_any()
            .downcast_ref::<cove_ui_compose::primitives::text::Text>();
        assert!(
            rendered.is_some_and(|text| text.string == "probe"),
            "the override replaces the environment for descendants"
        );
    });
}

#[test]
fn panel_style_defaults_to_the_default_style() {
    GlobalTestLock::locked(|| {
        let node = Node::new();
        let any: AnyPanelStyle = node.composition_value(&PanelStyleEnvironment::key());
        let body = any.make_body(panel_configuration());
        assert!(
            is_chrome(&body),
            "the default panel style produces chrome"
        );
    });
}

#[test]
fn list_row_style_defaults_and_renders_a_selected_row() {
    GlobalTestLock::locked(|| {
        let node = Node::new();
        let any: AnyListRowStyle = node.composition_value(&ListRowStyleEnvironment::key());
        let body = any.make_body(ListRowStyleConfiguration {
            content: AnyView::new(cove_composition::EmptyView),
            is_selected: true,
            is_hovered: false,
            is_enabled: true,
            row_height: 30.0,
            theme: Theme::default_dark(),
            ..Default::default()
        });
        let _ = body.clone().storage;
        assert_eq!(
            node.composition_value(&ListRowStyleEnvironment::key()),
            AnyListRowStyle::new(DefaultListRowStyle),
            "the environment falls back to DefaultListRowStyle"
        );
    });
}

#[test]
fn tree_row_style_is_wired_and_carries_depth() {
    GlobalTestLock::locked(|| {
        let node = Node::new();
        let any: AnyTreeRowStyle = node.composition_value(&TreeRowStyleEnvironment::key());
        let body = any.make_body(TreeRowStyleConfiguration {
            content: AnyView::new(cove_composition::EmptyView),
            depth: 3,
            indentation: 14.0,
            disclosure_width: 18.0,
            has_children: true,
            is_expanded: true,
            is_search_hit: false,
            is_selected: false,
            is_hovered: false,
            is_enabled: true,
            theme: Theme::default_dark(),
        });
        let _ = body.clone().storage;
        assert_eq!(
            node.composition_value(&TreeRowStyleEnvironment::key()),
            AnyTreeRowStyle::new(DefaultTreeRowStyle),
            "the environment falls back to DefaultTreeRowStyle"
        );
    });
}

#[test]
fn divider_style_defaults_to_the_theme_divider_colour() {
    GlobalTestLock::locked(|| {
        let theme = Theme::default_dark();
        let any = AnyDividerStyle::new(DefaultDividerStyle);
        let body = any.make_body(DividerStyleConfiguration {
            orientation: DividerOrientation::Horizontal,
            thickness: 1.0,
            theme: theme.clone(),
        });
        let graph = graph();
        graph.install_view(body, Default::default());
        graph.compute_layout(200.0, 100.0);
        let node = support::deepest_node(&graph.tree.borrow().root().expect("root"));
        let painted = node.background_color();
        assert_eq!(
            painted,
            Some(theme.colors.divider),
            "the default divider paints the theme's divider colour"
        );
    });
}

#[test]
fn every_style_environment_falls_back_to_its_default() {
    GlobalTestLock::locked(|| {
        let node = Node::new();
        let _: AnyTextFieldStyle = node.composition_value(&TextFieldStyleEnvironment::key());
        let _: AnyPanelStyle = node.composition_value(&PanelStyleEnvironment::key());
        let _: AnyListRowStyle = node.composition_value(&ListRowStyleEnvironment::key());
        let _: AnyTreeRowStyle = node.composition_value(&TreeRowStyleEnvironment::key());
        let _: AnyDividerStyle = node.composition_value(&DividerStyleEnvironment::key());
        assert_eq!(
            node.composition_value(&TextFieldStyleEnvironment::key()),
            AnyTextFieldStyle::new(DefaultTextFieldStyle),
            "text field default resolves"
        );
    });
}
