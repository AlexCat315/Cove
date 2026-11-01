//! Translation of Primitives/Controls/FieldLabel.swift.
use crate::Scope;
use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::layout::{FlexExt, LayoutFrameExt};
use crate::modifiers::style::AppearanceExt;
use crate::theme::styles::buttons::{ButtonStyle, ButtonStyleConfiguration, ButtonStyleViewExt};
use crate::{AnyView, PrimitiveView, View, ViewKind};
use cove_core::scene::{FocusChainHolder, LayoutNode, Node};

#[derive(Clone, Debug, Default)]
pub struct FieldLabelOptions {
    pub is_required: bool,
    pub help: String,
    /// Matches within the nearest Form, or within this scene for standalone labels.
    pub target_id: Option<String>,
}

#[derive(Clone)]
pub struct FieldLabel {
    pub title: String,
    pub options: FieldLabelOptions,
}

impl FieldLabel {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            options: FieldLabelOptions::default(),
        }
    }
    pub fn configure(mut self, configure: impl FnOnce(&mut FieldLabelOptions)) -> Self {
        configure(&mut self.options);
        self
    }
}

impl View for FieldLabel {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(FieldLabelHost {
            title: self.title.clone(),
            options: self.options.clone(),
        })
    }
}

struct FieldLabelHost {
    title: String,
    options: FieldLabelOptions,
}

impl PrimitiveView for FieldLabelHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn make_layout_node(&self) -> Option<LayoutNode> {
        None
    }
    fn update_node(&self, _node: &Node) {}
    fn children(&self, node: &Node) -> Vec<AnyView> {
        use crate::modifiers::style::SemanticStyleViewExt;
        let title = self.title.clone() + if self.options.is_required { " *" } else { "" };
        let text = crate::primitives::text::Text::new(title)
            .semantic_font(crate::theme::SemanticFontRef::label())
            .semantic_foreground(crate::theme::SemanticColorRef::on_surface());
        let Some(target_id) = self.options.target_id.clone() else {
            return vec![AnyView::new(text)];
        };
        let weak_node = node.downgrade();
        let label = self.title.clone();
        let help = self.options.help.clone();
        let button = crate::primitives::controls::button::Button::new(text, move || {
            let Some(node) = Node::upgrade(&weak_node) else {
                return;
            };
            let mut root = node.clone();
            while root.attachment::<bool>(
                crate::primitives::controls::form::label_scope::FORM_LABEL_SCOPE_KEY,
            ) != Some(true)
            {
                let Some(parent) = root.parent() else { break };
                root = parent;
            }
            fn find(candidate: &Node, target_id: &str) -> Option<Node> {
                if candidate.is_focusable()
                    && candidate
                        .accessibility()
                        .is_some_and(|semantics| semantics.identifier == target_id)
                {
                    return Some(candidate.clone());
                }
                candidate
                    .children()
                    .iter()
                    .find_map(|child| find(child, target_id))
            }
            if let Some(target) = find(&root, &target_id)
                && let Some(chain) = FocusChainHolder::current()
            {
                chain.focus(Some(&target), true);
            }
        })
        .button_style(FieldLabelStyle)
        .accessibility(move |semantics| {
            semantics.label = label.clone();
            semantics.help = help.clone();
        });
        vec![AnyView::new(button)]
    }
}

impl View for FieldLabelHost {
    fn kind(&self) -> ViewKind<'_> {
        ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

#[derive(Clone, Copy)]
struct FieldLabelStyle;

impl ButtonStyle for FieldLabelStyle {
    fn make_body(&self, configuration: ButtonStyleConfiguration) -> AnyView {
        let label = configuration
            .label
            .flex(0.0)
            .shrink(1.0)
            .frame_min_width(0.0);
        let border = if configuration.is_focused {
            configuration.theme.colors.focus_ring
        } else {
            cove_core::Color::CLEAR
        };
        AnyView::new(
            // `Stretch` on the cross axis, not `Center`: a centred row
            // shrink-to-fits its child, so the label was measured unconstrained
            // (306pt intrinsic) inside a 220pt host and never wrapped. Swift's
            // row hands its definite width down, which is what makes the label
            // wrap. The glyphs stay centred because the label fills the row.
            // Swift puts `.flex(1, shrink: 1).frame(minWidth: 0)` on the *row*
            // (FieldLabel.swift:44), not only on the label. Without it the row
            // shrink-to-fits, so the label is measured unconstrained at its
            // 306pt intrinsic width inside a 220pt host and never wraps.
            crate::BoxView::new(
                cove_core::scene::FlexDirection::Row,
                cove_core::scene::Align::Center,
                cove_core::scene::Justify::FlexStart,
                cove_core::scene::Wrap::NoWrap,
                0.0,
                AnyView::new(label.flex(0.0).shrink(1.0).frame_min_width(0.0)),
            )
            .flex(1.0)
            .shrink(1.0)
            .frame_min_width(0.0)
            .border_color(border, if configuration.is_focused { 1.0 } else { 0.0 }),
        )
    }
}
