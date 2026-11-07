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
//! - all five style environments resolve to their `Default*` implementation

use crate::support;

use cove_composition::{AnyView, EmptyView};
use cove_core::InputCapabilities;
use cove_core::scene::Node;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::Theme;
use cove_ui_compose::theme::styles::components::default_divider_style::DefaultDividerStyle;
use cove_ui_compose::theme::styles::components::divider_style::{
    AnyDividerStyle, DividerOrientation, DividerStyle, DividerStyleConfiguration,
    DividerStyleEnvironment,
};
use cove_ui_compose::theme::styles::components::{
    AnyListRowStyle, AnyPanelStyle, AnyTextFieldStyle, AnyTreeRowStyle, DefaultListRowStyle,
    DefaultPanelStyle, DefaultTextFieldStyle, DefaultTreeRowStyle, ListRowStyleConfiguration,
    ListRowStyleEnvironment, PanelStyleConfiguration, PanelStyleEnvironment, TextFieldStyle,
    TextFieldStyleConfiguration, TextFieldStyleEnvironment, TextFieldStyleViewExt,
    TreeRowStyleConfiguration, TreeRowStyleEnvironment, TreeRowStyleScope,
};

use support::{DebugNode, GlobalTestLock, graph};

fn text_field_configuration() -> TextFieldStyleConfiguration {
    TextFieldStyleConfiguration {
        content: AnyView::new(EmptyView),
        placeholder: String::new(),
        is_focused: false,
        is_editing: false,
        is_error: false,
        is_enabled: true,
        theme: Theme::default_dark(),
        is_hovered: false,
    }
}

/// A body counts as chrome when it is not the empty placeholder.
fn is_chrome(body: &AnyView) -> bool {
    body.storage.as_any().downcast_ref::<EmptyView>().is_none()
}

#[test]
fn text_field_style_defaults_to_the_default_style() {
    GlobalTestLock::locked(|| {
        let node = Node::new();
        let any: AnyTextFieldStyle = node.composition_value(&TextFieldStyleEnvironment::key());
        assert!(
            any == AnyTextFieldStyle::new(DefaultTextFieldStyle),
            "the environment falls back to DefaultTextFieldStyle"
        );
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
        AnyView::new(Text::new("probe"))
    }
}

#[test]
fn text_field_style_override_reaches_the_subtree() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let graph = graph();
            graph.install_view(
                AnyView::new(DebugNode::new("x").text_field_style(ProbeStyle)),
                Default::default(),
            );
            let leaf = support::deepest_node(&graph.tree.borrow().root().expect("root"));
            let resolved: AnyTextFieldStyle =
                leaf.composition_value(&TextFieldStyleEnvironment::key());
            assert!(
                resolved == AnyTextFieldStyle::new(ProbeStyle),
                "the override replaces the environment for descendants"
            );
            let body = resolved.make_body(text_field_configuration());
            let rendered = body.storage.as_any().downcast_ref::<Text>();
            assert!(
                rendered.is_some_and(|text| text.plain_text() == "probe"),
                "the override's body is the one that renders"
            );
        });
    });
}

#[test]
fn panel_style_defaults_to_the_default_style() {
    GlobalTestLock::locked(|| {
        let node = Node::new();
        let any: AnyPanelStyle = node.composition_value(&PanelStyleEnvironment::key());
        assert!(
            any == AnyPanelStyle::new(DefaultPanelStyle),
            "the environment falls back to DefaultPanelStyle"
        );
        let body = any.make_body(PanelStyleConfiguration {
            title: "Hello".to_string(),
            accessory: AnyView::new(EmptyView),
            content: AnyView::new(EmptyView),
            is_active: false,
            theme: Theme::default_dark(),
        });
        assert!(is_chrome(&body), "the default panel style produces chrome");
    });
}

#[test]
fn list_row_style_defaults_and_renders_a_selected_row() {
    GlobalTestLock::locked(|| {
        let node = Node::new();
        let any: AnyListRowStyle = node.composition_value(&ListRowStyleEnvironment::key());
        assert!(
            any == AnyListRowStyle::new(DefaultListRowStyle),
            "the environment falls back to DefaultListRowStyle"
        );
        let body = any.make_body(ListRowStyleConfiguration {
            content: AnyView::new(EmptyView),
            is_selected: true,
            is_emphasized: true,
            is_hovered: false,
            is_enabled: true,
            row_height: 30.0,
            shows_separator: true,
            theme: Theme::default_dark(),
            input: InputCapabilities::POINTER | InputCapabilities::KEYBOARD,
        });
        assert!(
            is_chrome(&body),
            "a selected row still produces a styled body"
        );
    });
}

#[test]
fn tree_row_style_is_wired_and_carries_depth() {
    GlobalTestLock::locked(|| {
        let node = Node::new();
        let any: AnyTreeRowStyle = node.composition_value(&TreeRowStyleEnvironment::key());
        assert!(
            any == AnyTreeRowStyle::new(DefaultTreeRowStyle, TreeRowStyleScope::Content),
            "the environment falls back to DefaultTreeRowStyle"
        );
        let body = any.make_body(TreeRowStyleConfiguration {
            content: AnyView::new(EmptyView),
            depth: 3,
            indentation: 14.0,
            disclosure_width: 18.0,
            has_children: true,
            is_expanded: true,
            is_search_hit: false,
            is_selected: false,
            is_hovered: false,
            is_checked: false,
            is_half_checked: false,
            is_current: false,
            is_enabled: true,
            fills_width: true,
            theme: Theme::default_dark(),
        });
        assert!(is_chrome(&body), "a deep expanded row produces a body");
    });
}

#[test]
fn divider_style_defaults_to_the_theme_divider_colour() {
    GlobalTestLock::locked(|| {
        let theme = Theme::default_dark();
        let node = Node::new();
        let any: AnyDividerStyle = node.composition_value(&DividerStyleEnvironment::key());
        assert!(
            any == AnyDividerStyle::new(DefaultDividerStyle),
            "the environment falls back to DefaultDividerStyle"
        );
        let body = any.make_body(DividerStyleConfiguration {
            orientation: DividerOrientation::Horizontal,
            thickness: 1.0,
            theme: theme.clone(),
        });
        let graph = graph();
        graph.install_view(body, Default::default());
        graph.compute_layout(200.0, 100.0);
        let painted = support::deepest_node(&graph.tree.borrow().root().expect("root"));
        assert_eq!(
            painted.background_color(),
            Some(theme.colors.divider),
            "the default divider paints the theme's divider colour"
        );
    });
}

#[test]
fn every_style_environment_falls_back_to_its_default() {
    GlobalTestLock::locked(|| {
        let node = Node::new();
        assert!(
            node.composition_value(&TextFieldStyleEnvironment::key())
                == AnyTextFieldStyle::new(DefaultTextFieldStyle)
        );
        assert!(
            node.composition_value(&PanelStyleEnvironment::key())
                == AnyPanelStyle::new(DefaultPanelStyle)
        );
        assert!(
            node.composition_value(&ListRowStyleEnvironment::key())
                == AnyListRowStyle::new(DefaultListRowStyle)
        );
        assert!(
            node.composition_value(&TreeRowStyleEnvironment::key())
                == AnyTreeRowStyle::new(DefaultTreeRowStyle, TreeRowStyleScope::Content)
        );
        assert!(
            node.composition_value(&DividerStyleEnvironment::key())
                == AnyDividerStyle::new(DefaultDividerStyle)
        );
    });
}
