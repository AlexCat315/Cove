//! Workspace view: panes, tab bars, split dividers, rails and floating windows.
//!
//! Swift expressed this with `Box`/`Row`/`Text`/`ScrollView` widgets. Those
//! Compose primitives are still being ported, so each structural piece here is
//! a `PrimitiveView` that owns its node geometry directly. That keeps the
//! layout contract (flex weights, divider hairlines, negative-margin hit slop)
//! byte-for-byte with the Swift version while depending only on the stable
//! composition + scene layers.
use super::controller::{WorkspaceCommand, WorkspaceController};
use super::drag::{self, DRAG_THRESHOLD};
use super::ids::{
    WorkspaceEdge, WorkspacePanelID, WorkspaceSlotID, WorkspaceSplitAxis, WorkspaceTabGroupID,
};
use super::model::{
    WorkspaceDocument, WorkspaceLayoutNode, WorkspaceSlot, WorkspaceSlotKind, WorkspaceTabGroup,
};
use super::sizing::{WorkspaceLayoutMetrics, WorkspaceSizingResolver};
use super::theme::{WorkspaceRegionSizing, WorkspaceTheme};
use cove_composition::{
    AnyView, LayoutDebugAttachmentKey, PrimitiveView, View, impl_primitive_view,
};
use cove_core::Color;
use cove_core::scene::{
    Align, FlexDirection, FocusChainHolder, InputHandlerRoute, InputRoutingPriority, Justify,
    LayoutNode, Node, PointerCaptureHolder,
};
use cove_platform::MouseButton;
use cove_ui_compose::theme::tokens::SemanticColorRef;
use std::cell::RefCell;
use std::rc::Rc;

const TAB_PRESS_KEY: &str = "__workspace_tab_press";
pub(crate) const RESIZE_GEOMETRY_KEY: &str = "workspace.resize.geometry";
const RAIL_THICKNESS: f32 = 40.0;

/// Retained press state for a tab drag.
#[derive(Clone)]
struct TabPressState {
    down_x: f32,
    down_y: f32,
    last_x: f32,
    last_y: f32,
    did_drag: bool,
}

/// Shared geometry for a splitter: which container measured it and how far the
/// divider may travel.
#[derive(Clone, Debug)]
pub struct WorkspaceResizeGeometry {
    pub split_id: String,
    pub horizontal: bool,
    pub offset: f32,
    pub available: f32,
    pub range: (f32, f32),
    pub fraction: f32,
}

fn resolve_theme(node: &Node) -> WorkspaceTheme {
    node.attachment::<WorkspaceTheme>(WorkspaceTheme::COMPOSITION_LOCAL_KEY)
        .unwrap_or_default()
}

fn debug_name(node: &Node) -> Option<String> {
    node.attachment::<String>(LayoutDebugAttachmentKey::DEBUG_NAME)
}

// MARK: - Structural primitives

/// A container with an explicit flex direction and no hit testing of its own.
pub struct WorkspaceBox {
    pub direction: FlexDirection,
    pub align_items: Align,
    pub justify_content: Justify,
    pub spacing: f32,
    pub clips: bool,
    pub hit_testable: bool,
    pub children: Vec<AnyView>,
}

impl WorkspaceBox {
    pub fn new(direction: FlexDirection) -> Self {
        Self {
            direction,
            align_items: Align::Stretch,
            justify_content: Justify::FlexStart,
            spacing: 0.0,
            clips: false,
            hit_testable: false,
            children: Vec::new(),
        }
    }
    pub fn column() -> Self {
        Self::new(FlexDirection::Column)
    }
    pub fn row() -> Self {
        Self::new(FlexDirection::Row)
    }
    pub fn align(mut self, align: Align) -> Self {
        self.align_items = align;
        self
    }
    pub fn justify(mut self, justify: Justify) -> Self {
        self.justify_content = justify;
        self
    }
    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing;
        self
    }
    pub fn clipped(mut self) -> Self {
        self.clips = true;
        self
    }
    pub fn hit_testable(mut self) -> Self {
        self.hit_testable = true;
        self
    }
    pub fn child(mut self, child: impl View + 'static) -> Self {
        self.children.push(AnyView::new(child));
        self
    }
    pub fn any_child(mut self, child: AnyView) -> Self {
        self.children.push(child);
        self
    }
    pub fn children(mut self, children: Vec<AnyView>) -> Self {
        self.children.extend(children);
        self
    }
    pub fn boxed(self) -> AnyView {
        AnyView::new(self)
    }
}

impl PrimitiveView for WorkspaceBox {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(self.hit_testable);
        node.set_clips_to_bounds(self.clips);
        node
    }
    fn update_node(&self, node: &Node) {
        node.set_is_hit_testable(self.hit_testable);
        node.set_clips_to_bounds(self.clips);
    }
    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_flex_direction(self.direction);
        layout.set_align_items(self.align_items);
        layout.set_justify_content(self.justify_content);
        if self.spacing > 0.0 {
            layout.set_gap(self.spacing, cove_core::scene::Gutter::All);
        }
        Some(layout)
    }
    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_flex_direction(self.direction);
        layout.set_align_items(self.align_items);
        layout.set_justify_content(self.justify_content);
        layout.set_gap(self.spacing, cove_core::scene::Gutter::All);
    }
    fn children(&self, _node: &Node) -> Vec<AnyView> {
        self.children.clone()
    }
}
impl_primitive_view!(WorkspaceBox);

/// A pane background: rounded, clipped, no own hit area.
pub struct WorkspacePane {
    /// Explicit fill, used when no semantic slot is set.
    pub fill: Color,
    /// Theme-resolved fill. Takes precedence over [`Self::fill`], so a theme
    /// switch repaints the pane without rebuilding the view.
    pub semantic_fill: Option<SemanticColorRef>,
    pub children: Vec<AnyView>,
    debug_name: Option<String>,
    layout_role: Option<String>,
    semantic_role: Option<String>,
}

impl WorkspacePane {
    pub fn new(fill: Color) -> Self {
        Self {
            fill,
            semantic_fill: None,
            children: Vec::new(),
            debug_name: None,
            layout_role: None,
            semantic_role: None,
        }
    }

    /// A pane whose fill resolves against the active theme at paint time.
    pub fn semantic(fill: SemanticColorRef) -> Self {
        Self {
            semantic_fill: Some(fill),
            ..Self::new(Color::CLEAR)
        }
    }
    pub fn child(mut self, child: AnyView) -> Self {
        self.children.push(child);
        self
    }
    pub fn debug_name(mut self, name: impl Into<String>) -> Self {
        self.debug_name = Some(name.into());
        self
    }
    pub fn layout_role(mut self, role: impl Into<String>) -> Self {
        self.layout_role = Some(role.into());
        self
    }
    pub fn semantic_role(mut self, role: impl Into<String>) -> Self {
        self.semantic_role = Some(role.into());
        self
    }
    pub fn boxed(self) -> AnyView {
        AnyView::new(self)
    }
}

impl PrimitiveView for WorkspacePane {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn update_node(&self, node: &Node) {
        node.set_is_hit_testable(false);
        let fill = match &self.semantic_fill {
            Some(semantic) => semantic.resolve(&cove_ui_compose::theme::theme_for_node(node)),
            None => self.fill,
        };
        node.set_background_color(Some(fill));
        node.set_corner_radius(theme_radius_lg(node));
        if let Some(name) = &self.debug_name {
            node.set_attachment(LayoutDebugAttachmentKey::DEBUG_NAME, name.clone());
        }
        if let Some(role) = &self.layout_role {
            node.set_attachment(LayoutDebugAttachmentKey::LAYOUT_ROLE, role.clone());
        }
        if let Some(role) = &self.semantic_role {
            node.set_attachment(LayoutDebugAttachmentKey::SEMANTIC_ROLE, role.clone());
        }
    }
    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_flex_direction(FlexDirection::Column);
        layout.set_align_items(Align::Stretch);
        layout.set_min_width(Some(0.0));
        layout.set_min_height(Some(0.0));
        Some(layout)
    }
    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_flex_direction(FlexDirection::Column);
        layout.set_align_items(Align::Stretch);
    }
    fn children(&self, _node: &Node) -> Vec<AnyView> {
        self.children.clone()
    }
}
impl_primitive_view!(WorkspacePane);

fn theme_radius_lg(node: &Node) -> f32 {
    // Swift reads `theme.radius.lg`; the Rust scene stores the resolved radius
    // on the node so a theme switch only has to re-run `update_node`.
    node.attachment::<f32>("cove.workspace.pane.radius")
        .unwrap_or(8.0)
}

/// Collapse/expand region. Holds its extent as a flex weight so a disappearing
/// region collapses smoothly, and reserves only the hairline for a divider.
pub struct WorkspaceRegionTransition {
    pub is_visible: bool,
    pub horizontal: bool,
    pub sizing: WorkspaceRegionSizing,
    pub phase: Vec<bool>,
    pub content: Option<AnyView>,
    /// Snapshot of the authored document version when this region was built.
    /// A different phase with the same content means "animate".
    pub animates: bool,
}

impl WorkspaceRegionTransition {
    pub fn new(
        is_visible: bool,
        horizontal: bool,
        sizing: WorkspaceRegionSizing,
        content: Option<AnyView>,
    ) -> Self {
        Self {
            is_visible,
            horizontal,
            sizing,
            phase: vec![is_visible],
            content,
            animates: false,
        }
    }
    pub fn phase(mut self, phase: Vec<bool>) -> Self {
        self.phase = phase;
        self
    }
    pub fn boxed(self) -> AnyView {
        AnyView::new(self)
    }
    fn is_splitter(&self) -> bool {
        self.sizing.is_divider()
    }
}

impl PrimitiveView for WorkspaceRegionTransition {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn update_node(&self, node: &Node) {
        node.set_clips_to_bounds(!self.is_splitter());
        node.set_z_index(if self.is_splitter() { 1.0 } else { 0.0 });
        node.set_allows_hit_testing(self.is_visible);
        node.set_is_interaction_enabled(self.is_visible);
        if !self.is_visible {
            // A region that disappears must not keep focus or capture.
            if let Some(chain) = FocusChainHolder::current()
                && let Some(focused) = chain.focused()
                && !focused.accepts_subtree_input()
            {
                chain.clear();
            }
            if let Some(capture) = PointerCaptureHolder::current()
                && let Some(target) = capture.target()
                && !target.accepts_subtree_input()
            {
                capture.release();
            }
        }
        node.set_opacity(if self.is_visible { 1.0 } else { 0.0 });
    }
    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_flex_direction(FlexDirection::Column);
        layout.set_align_items(Align::Stretch);
        layout.set_min_width(Some(0.0));
        layout.set_min_height(Some(0.0));
        layout.set_flex_shrink(1.0);
        self.apply_extent(&layout);
        Some(layout)
    }
    fn update_layout(&self, layout: &LayoutNode) {
        self.apply_extent(layout);
    }
    fn children(&self, _node: &Node) -> Vec<AnyView> {
        if !self.is_visible {
            return Vec::new();
        }
        self.content.iter().cloned().collect()
    }
}

impl WorkspaceRegionTransition {
    fn apply_extent(&self, layout: &LayoutNode) {
        let target = if self.is_visible {
            self.sizing.extent(&WorkspaceTheme::default())
        } else {
            0.0
        };
        match self.sizing {
            WorkspaceRegionSizing::Fraction(_) => {
                layout.set_flex_basis(0.0);
                layout.set_flex_grow(target);
            }
            WorkspaceRegionSizing::Points(_) => {
                layout.set_flex_grow(0.0);
                layout.set_flex_shrink(0.0);
                if self.horizontal {
                    layout.set_width(Some(target));
                } else {
                    layout.set_height(Some(target));
                }
            }
            WorkspaceRegionSizing::Divider => {
                // The hairline separates two panes, so the region reserves only
                // it; the grab area is a negative margin that overhangs the
                // neighbouring cards instead of pushing them apart.
                layout.set_flex_grow(0.0);
                layout.set_flex_shrink(0.0);
                let slop = WorkspaceTheme::default().split_divider_hit_slop;
                let span = target + 2.0 * slop;
                if self.horizontal {
                    layout.set_width(Some(span));
                    layout.set_margin(-slop, cove_core::scene::Edge::Left);
                    layout.set_margin(-slop, cove_core::scene::Edge::Right);
                } else {
                    layout.set_height(Some(span));
                    layout.set_margin(-slop, cove_core::scene::Edge::Top);
                    layout.set_margin(-slop, cove_core::scene::Edge::Bottom);
                }
            }
        }
    }
}
impl_primitive_view!(WorkspaceRegionTransition);

/// Text label. The Compose `Text` primitive is unavailable, so this records the
/// string and the resolved color on the node for the host renderer.
pub struct WorkspaceLabel {
    pub text: String,
    pub color: Color,
    pub size: f32,
    pub line_limit: Option<usize>,
}

impl WorkspaceLabel {
    pub fn new(text: impl Into<String>, color: Color) -> Self {
        Self {
            text: text.into(),
            color,
            size: 12.0,
            line_limit: Some(1),
        }
    }
    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }
    pub fn line_limit(mut self, limit: Option<usize>) -> Self {
        self.line_limit = limit;
        self
    }
    pub fn boxed(self) -> AnyView {
        AnyView::new(self)
    }
}

impl PrimitiveView for WorkspaceLabel {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn update_node(&self, node: &Node) {
        node.set_attachment("cove.workspace.label.text", self.text.clone());
        node.set_attachment("cove.workspace.label.color", self.color);
        node.set_attachment("cove.workspace.label.size", self.size);
        node.set_attachment("cove.workspace.label.lineLimit", self.line_limit);
    }
    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_align_items(Align::Center);
        Some(layout)
    }
}
impl_primitive_view!(WorkspaceLabel);

/// Vertical character stack used by a collapsed side rail, with the Swift
/// short-Latin exception (a 3-char ASCII title stays horizontal).
pub struct WorkspaceVerticalTitle {
    pub title: String,
    pub color: Color,
}

impl WorkspaceVerticalTitle {
    pub fn new(title: impl Into<String>, color: Color) -> Self {
        Self {
            title: title.into(),
            color,
        }
    }
    pub fn boxed(self) -> AnyView {
        AnyView::new(self)
    }
    fn clean_title(&self) -> String {
        let clean = self.title.replace('\n', " ");
        let clean = clean.trim();
        if clean.is_empty() {
            "Panel".to_string()
        } else {
            clean.to_string()
        }
    }
    fn is_short_latin(&self) -> bool {
        let title = self.clean_title();
        title.chars().count() <= 3 && title.is_ascii()
    }
}

impl PrimitiveView for WorkspaceVerticalTitle {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn update_node(&self, node: &Node) {
        node.set_attachment("cove.workspace.label.text", self.clean_title());
        node.set_attachment("cove.workspace.label.color", self.color);
        node.set_attachment("cove.workspace.label.size", 11.0);
        node.set_attachment("cove.workspace.label.vertical", !self.is_short_latin());
    }
    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        if self.is_short_latin() {
            layout.set_align_items(Align::Center);
            layout.set_min_height(Some(28.0));
        } else {
            let count = self.clean_title().chars().count() as f32;
            layout.set_flex_direction(FlexDirection::Column);
            layout.set_align_items(Align::Center);
            layout.set_justify_content(Justify::Center);
            layout.set_width(Some(24.0));
            layout.set_min_height(Some((count * 13.0 + 12.0).max(48.0)));
        }
        Some(layout)
    }
    fn update_layout(&self, layout: &LayoutNode) {
        if self.is_short_latin() {
            layout.set_align_items(Align::Center);
        } else {
            layout.set_flex_direction(FlexDirection::Column);
            layout.set_width(Some(24.0));
        }
    }
}
impl_primitive_view!(WorkspaceVerticalTitle);

// MARK: - Tab bar

struct WorkspaceTabButton {
    group_id: WorkspaceTabGroupID,
    panel_id: WorkspacePanelID,
    title: String,
    is_active: bool,
    is_pinned: bool,
    document: WorkspaceDocument,
    controller: Rc<RefCell<WorkspaceController>>,
}

impl PrimitiveView for WorkspaceTabButton {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(true);
        node.set_is_focusable(true);
        node
    }

    fn update_node(&self, node: &Node) {
        node.set_cursor(Some(cove_platform::SystemCursor::Pointer));
        let group_id = self.group_id.clone();
        let panel_id = self.panel_id.clone();
        let title = self.title.clone();
        let is_active = self.is_active;
        let is_pinned = self.is_pinned;
        let document = self.document.clone();
        let controller = self.controller.clone();
        let node_handle = node.clone();

        let Some(registry) = cove_core::scene::InteractionRegistryHolder::current() else {
            return;
        };
        let route = InputHandlerRoute::new(
            cove_core::scene::InputHandlerRole::Workspace,
            InputRoutingPriority::CAPTURE,
            "workspace.tab.drag",
        );

        let press_node = node.clone();
        let press_controller = controller.clone();
        let press_document = document.clone();
        let press_group = group_id.clone();
        let press_panel = panel_id.clone();
        registry.set_pointer(
            node,
            route.clone(),
            Rc::new(move |event, phase, _| {
                if event.button != MouseButton::Left {
                    return cove_core::scene::EventResult::Ignored;
                }
                match phase {
                    cove_core::scene::PointerPhase::Down => {
                        if let Some(chain) = FocusChainHolder::current() {
                            chain.focus(Some(&press_node), false);
                        }
                        press_node.set_attachment(
                            TAB_PRESS_KEY,
                            TabPressState {
                                down_x: event.x,
                                down_y: event.y,
                                last_x: event.x,
                                last_y: event.y,
                                did_drag: false,
                            },
                        );
                        if let Some(capture) = PointerCaptureHolder::current() {
                            capture.acquire(&press_node, false);
                        }
                        cove_core::scene::EventResult::Handled
                    }
                    cove_core::scene::PointerPhase::Up => {
                        let state = press_node.attachment::<TabPressState>(TAB_PRESS_KEY);
                        press_node.remove_attachment(TAB_PRESS_KEY);
                        press_node.set_cursor(Some(cove_platform::SystemCursor::Pointer));
                        press_node.set_opacity(1.0);
                        if let Some(root) = drag::workspace_root(&press_node) {
                            drag::set_drag_overlay(None, &root);
                        }
                        if let Some(capture) = PointerCaptureHolder::current()
                            && capture.target().as_ref() == Some(&press_node)
                        {
                            capture.release();
                        }
                        let Some(state) = state else {
                            return cove_core::scene::EventResult::Ignored;
                        };
                        if state.did_drag {
                            let resolution = drag::resolve_drag(
                                event.x,
                                event.y,
                                &press_group,
                                &press_panel,
                                &press_document,
                                &press_node,
                            );
                            drag::apply_resolution(
                                resolution,
                                &press_panel,
                                &mut press_controller.borrow_mut(),
                            );
                            return cove_core::scene::EventResult::Handled;
                        }
                        press_controller
                            .borrow_mut()
                            .dispatch(WorkspaceCommand::SetActivePanel {
                                group_id: press_group.clone(),
                                panel_id: press_panel.clone(),
                            });
                        cove_core::scene::EventResult::Handled
                    }
                }
            }),
        );

        let motion_node = node.clone();
        let _motion_controller = controller.clone();
        let motion_document = document.clone();
        let motion_group = group_id.clone();
        let motion_panel = panel_id.clone();
        let motion_title = title.clone();
        registry.set_motion(
            node,
            route,
            Rc::new(move |event, _| {
                if PointerCaptureHolder::current()
                    .and_then(|capture| capture.target())
                    .as_ref()
                    != Some(&motion_node)
                {
                    return cove_core::scene::EventResult::Ignored;
                }
                let Some(state) = motion_node.attachment::<TabPressState>(TAB_PRESS_KEY) else {
                    return cove_core::scene::EventResult::Ignored;
                };
                let dx = event.x - state.down_x;
                let dy = event.y - state.down_y;
                let draggable = motion_document
                    .panels
                    .get(&motion_panel)
                    .is_none_or(|panel| panel.is_draggable);
                let mut did_drag = state.did_drag;
                if !did_drag && draggable && dx.abs().max(dy.abs()) >= DRAG_THRESHOLD {
                    did_drag = true;
                    motion_node.set_cursor(Some(cove_platform::SystemCursor::Move));
                    motion_node.set_opacity(0.42);
                }
                motion_node.set_attachment(
                    TAB_PRESS_KEY,
                    TabPressState {
                        down_x: state.down_x,
                        down_y: state.down_y,
                        last_x: event.x,
                        last_y: event.y,
                        did_drag,
                    },
                );
                if did_drag && let Some(root) = drag::workspace_root(&motion_node) {
                    let resolution = drag::resolve_drag(
                        event.x,
                        event.y,
                        &motion_group,
                        &motion_panel,
                        &motion_document,
                        &motion_node,
                    );
                    let overlay = drag::WorkspaceTabDragOverlayState {
                        cursor_x: event.x,
                        cursor_y: event.y,
                        title: motion_title.clone(),
                        resolution,
                    };
                    drag::set_drag_overlay(Some(&overlay), &root);
                }
                if did_drag {
                    cove_core::scene::EventResult::Handled
                } else {
                    cove_core::scene::EventResult::Ignored
                }
            }),
        );

        let activate_controller = controller.clone();
        let activate_group = group_id.clone();
        let activate_panel = panel_id.clone();
        let style = node_handle;
        registry.set_key(
            node,
            InputHandlerRoute::new(
                cove_core::scene::InputHandlerRole::Workspace,
                InputRoutingPriority::NORMAL,
                "workspace.tab.key",
            ),
            Rc::new(move |event, phase| {
                if phase != cove_core::scene::EventPhase::Target {
                    return cove_core::scene::EventResult::Ignored;
                }
                if event.modifiers.has_ctrl()
                    || event.modifiers.has_gui()
                    || event.modifiers.has_alt()
                {
                    return cove_core::scene::EventResult::Ignored;
                }
                if event.scancode == cove_composition::Scancode::RETURN
                    || event.scancode == cove_composition::Scancode::SPACE
                {
                    activate_controller
                        .borrow_mut()
                        .dispatch(WorkspaceCommand::SetActivePanel {
                            group_id: activate_group.clone(),
                            panel_id: activate_panel.clone(),
                        });
                    return cove_core::scene::EventResult::Handled;
                }
                cove_core::scene::EventResult::Ignored
            }),
        );

        let mut accessibility =
            cove_core::scene::AccessibilitySemantics::new(cove_core::scene::AccessibilityRole::Tab);
        accessibility.label = title.clone();
        accessibility.value = if is_pinned {
            "Pinned".to_string()
        } else {
            String::new()
        };
        accessibility.identifier = panel_id.raw_value().to_string();
        accessibility.state.is_selected = is_active;
        accessibility.combines_children = true;
        style.set_accessibility(Some(accessibility.clone()));
        let mut actions = style.accessibility_actions();
        let click_controller = controller.clone();
        let click_group = group_id.clone();
        let click_panel = panel_id.clone();
        actions.activate = Some(Rc::new(move || {
            click_controller
                .borrow_mut()
                .dispatch(WorkspaceCommand::SetActivePanel {
                    group_id: click_group.clone(),
                    panel_id: click_panel.clone(),
                });
        }));
        style.set_accessibility_actions(actions);
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_flex_direction(FlexDirection::Row);
        layout.set_align_items(Align::Center);
        layout.set_justify_content(Justify::Center);
        layout.set_flex_shrink(0.0);
        Some(layout)
    }
    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_flex_direction(FlexDirection::Row);
        layout.set_align_items(Align::Center);
        layout.set_justify_content(Justify::Center);
        layout.set_flex_shrink(0.0);
    }
    fn children(&self, node: &Node) -> Vec<AnyView> {
        let height = node
            .attachment::<f32>("cove.workspace.tabbar.height")
            .unwrap_or(34.0);
        let title_color = node
            .attachment::<Color>("cove.workspace.tabbar.activeColor")
            .unwrap_or(Color::WHITE);
        let mut row = WorkspaceBox::row().align(Align::Center).spacing(4.0);
        if self.is_pinned {
            row = row.child(WorkspaceLabel::new("•", title_color).size(6.0).boxed());
        }
        row = row.child(
            WorkspaceLabel::new(self.title.clone(), title_color)
                .size(11.0)
                .boxed(),
        );
        let mut children = vec![row.boxed()];
        // Keep the recorded height available to the host renderer.
        let _ = height;
        children.push(WorkspaceLabel::new("", title_color).boxed());
        children
    }
}
impl_primitive_view!(WorkspaceTabButton);

/// One tab bar: horizontally scrolling tab row plus the overflow affordance.
struct WorkspaceTabBar {
    group: WorkspaceTabGroup,
    document: WorkspaceDocument,
    controller: Rc<RefCell<WorkspaceController>>,
}

impl PrimitiveView for WorkspaceTabBar {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn update_node(&self, node: &Node) {
        let theme = resolve_theme(node);
        node.set_attachment(
            "cove.workspace.tabbar.height",
            theme.tab_bar_height.max(26.0),
        );
        node.set_attachment(
            "cove.workspace.tabbar.activeColor",
            Color::from_rgba8(235, 235, 240, 255),
        );
    }
    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_flex_direction(FlexDirection::Row);
        layout.set_align_items(Align::Center);
        layout.set_height(Some(34.0));
        layout.set_flex_shrink(0.0);
        Some(layout)
    }
    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_flex_direction(FlexDirection::Row);
        layout.set_align_items(Align::Center);
        layout.set_height(Some(34.0));
        layout.set_flex_shrink(0.0);
    }
    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let active = self
            .group
            .active_panel_id
            .clone()
            .or_else(|| self.group.panels.first().cloned());
        let mut tabs = Vec::new();
        for panel_id in &self.group.panels {
            let Some(panel) = self.document.panels.get(panel_id) else {
                continue;
            };
            let is_active = active.as_ref() == Some(panel_id);
            let button = WorkspaceTabButton {
                group_id: self.group.id.clone(),
                panel_id: panel_id.clone(),
                title: panel.title.clone(),
                is_active,
                is_pinned: self.group.is_pinned(panel_id),
                document: self.document.clone(),
                controller: self.controller.clone(),
            };
            let mut view = AnyView::new(button);
            let _ = &mut view;
            tabs.push(view);
        }
        vec![
            WorkspaceBox::row()
                .align(Align::Center)
                .children(tabs)
                .boxed(),
        ]
    }
}
impl_primitive_view!(WorkspaceTabBar);

// MARK: - Tab group

struct WorkspaceTabGroupBody {
    group: WorkspaceTabGroup,
    document: WorkspaceDocument,
    controller: Rc<RefCell<WorkspaceController>>,
    content: Rc<dyn Fn(WorkspacePanelID) -> AnyView>,
}

impl PrimitiveView for WorkspaceTabGroupBody {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn update_node(&self, _node: &Node) {}
    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_flex_direction(FlexDirection::Column);
        layout.set_align_items(Align::Stretch);
        layout.set_min_width(Some(0.0));
        layout.set_min_height(Some(0.0));
        Some(layout)
    }
    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_flex_direction(FlexDirection::Column);
        layout.set_align_items(Align::Stretch);
    }
    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let mut children = Vec::new();
        let hides_tab_bar = self.group.panels.len() == 1
            && self
                .group
                .panels
                .first()
                .and_then(|id| self.document.panels.get(id))
                .is_some_and(|panel| panel.hides_tab_bar_when_alone);
        if !hides_tab_bar {
            children.push(AnyView::new(WorkspaceTabBar {
                group: self.group.clone(),
                document: self.document.clone(),
                controller: self.controller.clone(),
            }));
        }
        if let Some(active) = self
            .group
            .active_panel_id
            .clone()
            .or_else(|| self.group.panels.first().cloned())
        {
            let panel = (self.content)(active.clone());
            let node_view = Node::new();
            node_view.set_attachment(
                LayoutDebugAttachmentKey::DEBUG_NAME,
                format!("workspace-panel-{}", active.raw_value()),
            );
            let _ = node_view;
            // Swift wraps the panel in a vertical scroll view; the panel itself
            // is laid out at the region height and only overflow scrolls.
            children.push(WorkspaceBox::column().child(panel).boxed());
        }
        children
    }
}
impl_primitive_view!(WorkspaceTabGroupBody);

fn tab_group_view(
    group: WorkspaceTabGroup,
    document: WorkspaceDocument,
    controller: Rc<RefCell<WorkspaceController>>,
    content: Rc<dyn Fn(WorkspacePanelID) -> AnyView>,
    surface: Color,
) -> AnyView {
    let body = WorkspaceTabGroupBody {
        group: group.clone(),
        document,
        controller,
        content,
    };
    let pane = WorkspacePane::new(surface)
        .debug_name(format!("workspace-group-{}", group.id.raw_value()))
        .layout_role("workspace-tab-group")
        .semantic_role("workspace.group");
    AnyView::new(pane.child(AnyView::new(body)))
}

// MARK: - Layout node recursion

struct WorkspaceLayoutNodeView {
    node: WorkspaceLayoutNode,
    slot_id: WorkspaceSlotID,
    document: WorkspaceDocument,
    controller: Rc<RefCell<WorkspaceController>>,
    content: Rc<dyn Fn(WorkspacePanelID) -> AnyView>,
    surface: Color,
}

impl PrimitiveView for WorkspaceLayoutNodeView {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn update_node(&self, _node: &Node) {}
    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_flex_direction(FlexDirection::Row);
        layout.set_align_items(Align::Stretch);
        layout.set_min_width(Some(0.0));
        layout.set_min_height(Some(0.0));
        Some(layout)
    }
    fn update_layout(&self, layout: &LayoutNode) {
        if matches!(self.node, WorkspaceLayoutNode::Split { axis, .. } if axis == WorkspaceSplitAxis::Vertical)
        {
            layout.set_flex_direction(FlexDirection::Column);
        } else {
            layout.set_flex_direction(FlexDirection::Row);
        }
    }
    fn children(&self, _node: &Node) -> Vec<AnyView> {
        self.build()
    }
}
impl_primitive_view!(WorkspaceLayoutNodeView);

impl WorkspaceLayoutNodeView {
    fn build(&self) -> Vec<AnyView> {
        match &self.node {
            WorkspaceLayoutNode::Group(group_id) => {
                let Some(group) = self.document.groups.get(group_id) else {
                    return vec![WorkspaceBox::row().boxed()];
                };
                vec![tab_group_view(
                    group.clone(),
                    self.document.clone(),
                    self.controller.clone(),
                    self.content.clone(),
                    self.surface,
                )]
            }
            WorkspaceLayoutNode::Split {
                axis,
                fraction,
                first,
                second,
            } => {
                let first_leaf = first.leaf_group_ids().into_iter().next();
                let second_leaf = second.leaf_group_ids().into_iter().next();
                let (Some(first_leaf), Some(second_leaf)) = (first_leaf, second_leaf) else {
                    return Vec::new();
                };
                let address = super::model::WorkspaceLayoutSplit::new(
                    self.slot_id.clone(),
                    first_leaf,
                    second_leaf,
                );
                let metrics = WorkspaceLayoutMetrics::default();
                let resolver = WorkspaceSizingResolver::with_metrics(&self.document, metrics);
                let first_size = resolver.layout(first, *axis);
                let second_size = resolver.layout(second, *axis);
                let splitter = WorkspaceSplitDivider::new(
                    format!(
                        "workspace-layout-split-{}-{}-{}",
                        self.slot_id.raw_value(),
                        address.first_group.raw_value(),
                        address.second_group.raw_value()
                    ),
                    *axis == WorkspaceSplitAxis::Vertical,
                    self.controller.clone(),
                    Some(address),
                    metrics.divider,
                );
                let first_view = AnyView::new(Self {
                    node: (**first).clone(),
                    slot_id: self.slot_id.clone(),
                    document: self.document.clone(),
                    controller: self.controller.clone(),
                    content: self.content.clone(),
                    surface: self.surface,
                });
                let second_view = AnyView::new(Self {
                    node: (**second).clone(),
                    slot_id: self.slot_id.clone(),
                    document: self.document.clone(),
                    controller: self.controller.clone(),
                    content: self.content.clone(),
                    surface: self.surface,
                });
                let _ = (first_size, second_size, *fraction);
                let container = match axis {
                    WorkspaceSplitAxis::Vertical => WorkspaceBox::column(),
                    WorkspaceSplitAxis::Horizontal => WorkspaceBox::row(),
                };
                vec![
                    container
                        .child(first_view)
                        .child(AnyView::new(splitter))
                        .child(second_view)
                        .boxed(),
                ]
            }
        }
    }
}

// MARK: - Split divider

struct WorkspaceSplitDivider {
    debug_name: String,
    /// `true` when the split is vertical, i.e. the divider itself is a vertical
    /// hairline the user drags horizontally.
    is_vertical_split: bool,
    controller: Rc<RefCell<WorkspaceController>>,
    /// Content-addressed split for an in-slot split, or `None` for one of the
    /// four named region splits.
    address: Option<super::model::WorkspaceLayoutSplit>,
    split_id: Option<super::ids::WorkspaceSplitID>,
    hairline: f32,
}

impl WorkspaceSplitDivider {
    fn new(
        debug_name: String,
        is_vertical_split: bool,
        controller: Rc<RefCell<WorkspaceController>>,
        address: Option<super::model::WorkspaceLayoutSplit>,
        hairline: f32,
    ) -> Self {
        Self {
            debug_name,
            is_vertical_split,
            controller,
            address,
            split_id: None,
            hairline,
        }
    }

    fn region(
        split_id: &str,
        is_vertical_split: bool,
        controller: Rc<RefCell<WorkspaceController>>,
        hairline: f32,
    ) -> Self {
        Self {
            debug_name: format!("workspace-split-{split_id}"),
            is_vertical_split,
            controller,
            address: None,
            split_id: Some(super::ids::WorkspaceSplitID::new(split_id)),
            hairline,
        }
    }

    fn write(&self, value: f32) {
        if !value.is_finite() {
            return;
        }
        let clamped = super::sizing::WorkspaceSplitFractions::clamp(value);
        let mut controller = self.controller.borrow_mut();
        match &self.address {
            Some(address) => {
                controller.dispatch(WorkspaceCommand::ResizeLayoutSplit {
                    address: address.clone(),
                    fraction: clamped,
                });
            }
            None => {
                if let Some(split_id) = &self.split_id {
                    controller.dispatch(WorkspaceCommand::ResizeSplit {
                        split_id: split_id.clone(),
                        fraction: clamped,
                    });
                }
            }
        }
    }
}

impl PrimitiveView for WorkspaceSplitDivider {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(true);
        node.set_is_focusable(true);
        node
    }
    fn update_node(&self, node: &Node) {
        let theme = resolve_theme(node);
        let hit_size = (theme.gutter + theme.split_divider_hit_slop * 2.0).max(1.0);
        node.set_cursor(Some(if self.is_vertical_split {
            cove_platform::SystemCursor::ResizeHorizontal
        } else {
            cove_platform::SystemCursor::ResizeVertical
        }));
        node.set_attachment("cove.workspace.split.hitSize", hit_size);
        node.set_attachment(
            LayoutDebugAttachmentKey::DEBUG_NAME,
            self.debug_name.clone(),
        );
        node.set_accessibility(Some({
            let mut semantics = cove_core::scene::AccessibilitySemantics::new(
                cove_core::scene::AccessibilityRole::Slider,
            );
            semantics.label = format!(
                "Resize {} workspace region",
                self.split_id
                    .as_ref()
                    .map(|id| id.raw_value().to_string())
                    .unwrap_or_else(|| "split".into())
            );
            semantics
        }));

        let Some(registry) = cove_core::scene::InteractionRegistryHolder::current() else {
            return;
        };
        let route = InputHandlerRoute::new(
            cove_core::scene::InputHandlerRole::Workspace,
            InputRoutingPriority::CAPTURE,
            "workspace.split",
        );
        let pointer_node = node.clone();
        registry.set_pointer(
            node,
            route.clone(),
            Rc::new(move |event, phase, _| {
                if event.button != MouseButton::Left {
                    return cove_core::scene::EventResult::Ignored;
                }
                match phase {
                    cove_core::scene::PointerPhase::Down => {
                        if let Some(chain) = FocusChainHolder::current() {
                            chain.focus(Some(&pointer_node), false);
                        }
                        if let Some(capture) = PointerCaptureHolder::current() {
                            capture.acquire(&pointer_node, false);
                        }
                    }
                    cove_core::scene::PointerPhase::Up => {
                        if let Some(capture) = PointerCaptureHolder::current()
                            && capture.target().as_ref() == Some(&pointer_node)
                        {
                            capture.release();
                        }
                    }
                }
                cove_core::scene::EventResult::Handled
            }),
        );

        let horizontal = self.is_vertical_split;
        let hairline = self.hairline;
        let controller = self.controller.clone();
        let address = self.address.clone();
        let split_id = self.split_id.clone();
        let motion_node = node.clone();
        registry.set_motion(
            node,
            route,
            Rc::new(move |event, _| {
                if PointerCaptureHolder::current()
                    .and_then(|capture| capture.target())
                    .as_ref()
                    != Some(&motion_node)
                {
                    return cove_core::scene::EventResult::Ignored;
                }
                let Some(geometry) =
                    motion_node.attachment::<Vec<WorkspaceResizeGeometry>>(RESIZE_GEOMETRY_KEY)
                else {
                    return cove_core::scene::EventResult::Ignored;
                };
                let wanted = split_id
                    .as_ref()
                    .map(|id| id.raw_value().to_string())
                    .or_else(|| {
                        address.as_ref().map(|address| {
                            format!(
                                "layout-{}-{}",
                                address.first_group.raw_value(),
                                address.second_group.raw_value()
                            )
                        })
                    });
                let Some(wanted) = wanted else {
                    return cove_core::scene::EventResult::Ignored;
                };
                let Some(geometry) = geometry
                    .iter()
                    .find(|candidate| candidate.split_id == wanted)
                else {
                    return cove_core::scene::EventResult::Ignored;
                };
                if geometry.available <= 0.0 {
                    return cove_core::scene::EventResult::Ignored;
                }
                let position = if geometry.horizontal {
                    event.x
                } else {
                    event.y
                };
                let value = (position - geometry.offset) / geometry.available;
                let clamped = geometry.range.0.max(geometry.range.1.min(value));
                let mut borrowed = controller.borrow_mut();
                match &address {
                    Some(address) => {
                        borrowed.dispatch(WorkspaceCommand::ResizeLayoutSplit {
                            address: address.clone(),
                            fraction: clamped,
                        });
                    }
                    None => {
                        if let Some(split_id) = &split_id {
                            borrowed.dispatch(WorkspaceCommand::ResizeSplit {
                                split_id: split_id.clone(),
                                fraction: clamped,
                            });
                        }
                    }
                }
                let _ = horizontal;
                let _ = hairline;
                cove_core::scene::EventResult::Handled
            }),
        );
    }
    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        if self.is_vertical_split {
            layout.set_flex_direction(FlexDirection::Row);
            layout.set_align_items(Align::Stretch);
            layout.set_justify_content(Justify::Center);
            layout.set_width(Some(1.0));
        } else {
            layout.set_flex_direction(FlexDirection::Column);
            layout.set_align_items(Align::Stretch);
            layout.set_justify_content(Justify::Center);
            layout.set_height(Some(1.0));
        }
        Some(layout)
    }
    fn update_layout(&self, layout: &LayoutNode) {
        let hit = 1.0;
        if self.is_vertical_split {
            layout.set_flex_direction(FlexDirection::Row);
            layout.set_width(Some(hit));
        } else {
            layout.set_flex_direction(FlexDirection::Column);
            layout.set_height(Some(hit));
        }
    }
    fn children(&self, _node: &Node) -> Vec<AnyView> {
        Vec::new()
    }
}
impl_primitive_view!(WorkspaceSplitDivider);

// MARK: - Rail

struct WorkspaceRailButton {
    group_id: WorkspaceTabGroupID,
    panel_id: WorkspacePanelID,
    title: String,
    controller: Rc<RefCell<WorkspaceController>>,
    horizontal: bool,
}

impl PrimitiveView for WorkspaceRailButton {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(true);
        node.set_is_focusable(true);
        node
    }
    fn update_node(&self, node: &Node) {
        node.set_cursor(Some(cove_platform::SystemCursor::Pointer));
        let group_id = self.group_id.clone();
        let panel_id = self.panel_id.clone();
        let controller = self.controller.clone();
        let mut actions = node.accessibility_actions();
        actions.activate = Some(Rc::new(move || {
            let mut borrowed = controller.borrow_mut();
            borrowed.dispatch(WorkspaceCommand::SetActivePanel {
                group_id: group_id.clone(),
                panel_id: panel_id.clone(),
            });
            borrowed.dispatch(WorkspaceCommand::Expand(group_id.clone()));
        }));
        node.set_accessibility_actions(actions);
        let mut semantics = cove_core::scene::AccessibilitySemantics::new(
            cove_core::scene::AccessibilityRole::Button,
        );
        semantics.label = self.title.clone();
        node.set_accessibility(Some(semantics));
    }
    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_flex_direction(FlexDirection::Column);
        layout.set_align_items(Align::Center);
        layout.set_justify_content(Justify::Center);
        layout.set_flex_shrink(0.0);
        if self.horizontal {
            layout.set_flex_direction(FlexDirection::Row);
        }
        Some(layout)
    }
    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_flex_direction(if self.horizontal {
            FlexDirection::Row
        } else {
            FlexDirection::Column
        });
        layout.set_align_items(Align::Center);
        layout.set_justify_content(Justify::Center);
    }
    fn children(&self, node: &Node) -> Vec<AnyView> {
        let color = node
            .attachment::<Color>("cove.workspace.rail.labelColor")
            .unwrap_or(Color::from_rgba8(160, 160, 168, 255));
        if self.horizontal {
            vec![
                WorkspaceLabel::new(self.title.clone(), color)
                    .size(11.0)
                    .boxed(),
            ]
        } else {
            vec![WorkspaceVerticalTitle::new(self.title.clone(), color).boxed()]
        }
    }
}
impl_primitive_view!(WorkspaceRailButton);

struct WorkspaceRail {
    slot_id: WorkspaceSlotID,
    edge_override: Option<WorkspaceEdge>,
    thickness: f32,
    groups_override: Option<Vec<WorkspaceTabGroup>>,
    document: WorkspaceDocument,
    controller: Rc<RefCell<WorkspaceController>>,
}

impl PrimitiveView for WorkspaceRail {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn update_node(&self, node: &Node) {
        node.set_background_color(Some(Color::from_rgba8(24, 24, 27, 255)));
        node.set_attachment(
            "cove.workspace.rail.labelColor",
            Color::from_rgba8(160, 160, 168, 255),
        );
    }
    fn make_layout_node(&self) -> Option<LayoutNode> {
        let edge = self.edge();
        let horizontal = matches!(edge, WorkspaceEdge::Top | WorkspaceEdge::Bottom);
        let layout = LayoutNode::new();
        layout.set_flex_direction(if horizontal {
            FlexDirection::Row
        } else {
            FlexDirection::Column
        });
        layout.set_align_items(if horizontal {
            Align::Center
        } else {
            Align::Center
        });
        layout.set_gap(
            if horizontal { 6.0 } else { 4.0 },
            cove_core::scene::Gutter::All,
        );
        if horizontal {
            layout.set_height(Some(self.thickness));
            layout.set_width_percent(100.0);
        } else {
            layout.set_width(Some(self.thickness));
        }
        Some(layout)
    }
    fn update_layout(&self, layout: &LayoutNode) {
        let horizontal = matches!(self.edge(), WorkspaceEdge::Top | WorkspaceEdge::Bottom);
        layout.set_flex_direction(if horizontal {
            FlexDirection::Row
        } else {
            FlexDirection::Column
        });
        if horizontal {
            layout.set_height(Some(self.thickness));
        } else {
            layout.set_width(Some(self.thickness));
        }
    }
    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let groups = self
            .groups_override
            .clone()
            .unwrap_or_else(|| rail_groups(&self.slot_id, self.edge_override, &self.document));
        let horizontal = matches!(self.edge(), WorkspaceEdge::Top | WorkspaceEdge::Bottom);
        let mut buttons = Vec::new();
        for group in groups {
            for panel_id in &group.panels {
                let Some(panel) = self.document.panels.get(panel_id) else {
                    continue;
                };
                buttons.push(AnyView::new(WorkspaceRailButton {
                    group_id: group.id.clone(),
                    panel_id: panel_id.clone(),
                    title: panel.title.clone(),
                    controller: self.controller.clone(),
                    horizontal,
                }));
            }
        }
        vec![
            WorkspaceBox::column()
                .align(Align::Center)
                .children(buttons)
                .boxed(),
        ]
    }
}
impl_primitive_view!(WorkspaceRail);

impl WorkspaceRail {
    fn edge(&self) -> WorkspaceEdge {
        self.edge_override
            .unwrap_or_else(|| rail_edge(&self.slot_id, &self.document))
    }
}

// MARK: - Document queries (Swift free functions)

pub fn visible_groups(
    slot_id: &WorkspaceSlotID,
    document: &WorkspaceDocument,
) -> Vec<WorkspaceTabGroup> {
    document
        .slot(slot_id)
        .layout
        .as_ref()
        .map(|layout| layout.leaf_group_ids())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|id| document.groups.get(&id).cloned())
        .filter(|group| !group.is_collapsed)
        .collect()
}

/// Drop collapsed, empty and missing groups so the renderer never sees a
/// branch with nothing to draw.
pub fn compact_layout(
    node: &WorkspaceLayoutNode,
    document: &WorkspaceDocument,
) -> Option<WorkspaceLayoutNode> {
    match node {
        WorkspaceLayoutNode::Group(group_id) => {
            let group = document.groups.get(group_id)?;
            (!group.is_collapsed && !group.panels.is_empty())
                .then(|| WorkspaceLayoutNode::Group(group_id.clone()))
        }
        WorkspaceLayoutNode::Split {
            axis,
            fraction,
            first,
            second,
        } => {
            match (
                compact_layout(first, document),
                compact_layout(second, document),
            ) {
                (None, None) => None,
                (Some(remaining), None) | (None, Some(remaining)) => Some(remaining),
                (Some(next_first), Some(next_second)) => Some(WorkspaceLayoutNode::split(
                    *axis,
                    *fraction,
                    next_first,
                    next_second,
                )),
            }
        }
    }
}

pub fn renderable_layout(
    slot_id: &WorkspaceSlotID,
    document: &WorkspaceDocument,
) -> Option<WorkspaceLayoutNode> {
    document
        .slot(slot_id)
        .layout
        .as_ref()
        .and_then(|layout| compact_layout(layout, document))
}

pub fn collapsed_groups_on(
    edge: WorkspaceEdge,
    document: &WorkspaceDocument,
) -> Vec<WorkspaceTabGroup> {
    document
        .collapsed
        .iter()
        .filter(|item| item.edge == edge)
        .filter_map(|item| document.groups.get(&item.group_id).cloned())
        .filter(|group| group.is_collapsed)
        .collect()
}

pub fn rail_groups(
    slot_id: &WorkspaceSlotID,
    edge_override: Option<WorkspaceEdge>,
    document: &WorkspaceDocument,
) -> Vec<WorkspaceTabGroup> {
    if let Some(edge) = edge_override {
        return collapsed_groups_on(edge, document);
    }
    let slot = document.slot(slot_id);
    if let WorkspaceSlotKind::Chrome { edge, .. } = slot.kind {
        return collapsed_groups_on(edge, document);
    }
    document
        .collapsed
        .iter()
        .filter(|item| &item.slot_id == slot_id)
        .filter_map(|item| document.groups.get(&item.group_id).cloned())
        .filter(|group| group.is_collapsed)
        .collect()
}

pub fn rail_edge(slot_id: &WorkspaceSlotID, document: &WorkspaceDocument) -> WorkspaceEdge {
    if let WorkspaceSlotKind::Chrome { edge, .. } = document.slot(slot_id).kind {
        return edge;
    }
    if slot_id == &WorkspaceSlotID::leading() {
        WorkspaceEdge::Leading
    } else if slot_id == &WorkspaceSlotID::trailing() {
        WorkspaceEdge::Trailing
    } else if slot_id == &WorkspaceSlotID::top() {
        WorkspaceEdge::Top
    } else {
        WorkspaceEdge::Bottom
    }
}

pub fn chrome_thickness(slot: &WorkspaceSlot) -> f32 {
    if let WorkspaceSlotKind::Chrome { size, .. } = slot.kind
        && let Some(fixed) = size.fixed_value()
    {
        return fixed;
    }
    RAIL_THICKNESS
}

pub fn chrome_rail_slots(
    edge: WorkspaceEdge,
    document: &WorkspaceDocument,
    shows_rails: bool,
) -> Vec<(WorkspaceSlotID, WorkspaceEdge, f32)> {
    let collapsed_on_edge = shows_rails && !collapsed_groups_on(edge, document).is_empty();
    let mut slots: Vec<&WorkspaceSlot> = document
        .slots
        .values()
        .filter(|slot| match slot.kind {
            WorkspaceSlotKind::Chrome {
                edge: slot_edge, ..
            } => slot_edge == edge && (slot.layout.is_some() || collapsed_on_edge),
            _ => false,
        })
        .collect();
    let preferred = WorkspaceSlotID::chrome_rail(edge);
    slots.sort_by(|lhs, rhs| {
        if lhs.id == preferred {
            return std::cmp::Ordering::Less;
        }
        if rhs.id == preferred {
            return std::cmp::Ordering::Greater;
        }
        lhs.id.raw_value().cmp(rhs.id.raw_value())
    });
    let mapped: Vec<_> = slots
        .into_iter()
        .filter_map(|slot| match slot.kind {
            WorkspaceSlotKind::Chrome { edge, .. } => {
                Some((slot.id.clone(), edge, chrome_thickness(slot)))
            }
            _ => None,
        })
        .collect();
    if collapsed_on_edge {
        mapped.into_iter().take(1).collect()
    } else {
        mapped
    }
}

pub fn visible_chrome_rail_slots(
    edge: WorkspaceEdge,
    document: &WorkspaceDocument,
) -> Vec<(WorkspaceSlotID, WorkspaceEdge, f32)> {
    let mut slots: Vec<&WorkspaceSlot> = document
        .slots
        .values()
        .filter(|slot| match slot.kind {
            WorkspaceSlotKind::Chrome {
                edge: slot_edge, ..
            } => slot_edge == edge && slot.layout.is_some(),
            _ => false,
        })
        .collect();
    slots.sort_by(|lhs, rhs| lhs.id.raw_value().cmp(rhs.id.raw_value()));
    slots
        .into_iter()
        .filter_map(|slot| match slot.kind {
            WorkspaceSlotKind::Chrome { edge, .. } => {
                Some((slot.id.clone(), edge, chrome_thickness(slot)))
            }
            _ => None,
        })
        .collect()
}

pub fn rail_plan(
    edge: WorkspaceEdge,
    document: &WorkspaceDocument,
    shows_rails: bool,
) -> Option<(WorkspaceSlotID, WorkspaceEdge, f32, Vec<WorkspaceTabGroup>)> {
    if !shows_rails {
        return None;
    }
    let groups = collapsed_groups_on(edge, document);
    if groups.is_empty() {
        return None;
    }
    let slot = chrome_rail_slots(edge, document, shows_rails)
        .into_iter()
        .next()
        .unwrap_or((WorkspaceSlotID::chrome_rail(edge), edge, RAIL_THICKNESS));
    Some((slot.0, slot.1, slot.2, groups))
}

#[cfg(test)]
mod tests {
    use super::super::model::{WorkspacePanel, WorkspaceSlot};
    use super::*;

    fn document() -> WorkspaceDocument {
        let mut panels = std::collections::BTreeMap::new();
        let panel = WorkspacePanel::new(WorkspacePanelID::new("viewport"), "Viewport");
        panels.insert(panel.id.clone(), panel);
        let group = WorkspaceTabGroup::new(
            WorkspaceTabGroupID::new("center"),
            vec![WorkspacePanelID::new("viewport")],
        );
        let mut groups = std::collections::BTreeMap::new();
        groups.insert(group.id.clone(), group);
        let mut slots = std::collections::BTreeMap::new();
        slots.insert(
            WorkspaceSlotID::center(),
            WorkspaceSlot::new(
                WorkspaceSlotID::center(),
                WorkspaceSlotKind::Content,
                Some(WorkspaceLayoutNode::Group(WorkspaceTabGroupID::new(
                    "center",
                ))),
            ),
        );
        WorkspaceDocument::new(panels, groups, slots)
    }

    #[test]
    fn collapsed_and_empty_groups_are_compacted_out() {
        let mut document = document();
        if let Some(group) = document.groups.get_mut(&WorkspaceTabGroupID::new("center")) {
            group.is_collapsed = true;
        }
        let layout = WorkspaceLayoutNode::Group(WorkspaceTabGroupID::new("center"));
        assert!(compact_layout(&layout, &document).is_none());
        assert!(visible_groups(&WorkspaceSlotID::center(), &document).is_empty());
    }

    #[test]
    fn center_slot_renders_its_group() {
        let document = document();
        let layout = renderable_layout(&WorkspaceSlotID::center(), &document).unwrap();
        assert_eq!(
            layout.leaf_group_ids(),
            vec![WorkspaceTabGroupID::new("center")]
        );
    }

    #[test]
    fn chrome_rail_thickness_falls_back_for_non_chrome_slots() {
        let document = document();
        assert_eq!(
            chrome_thickness(&document.slot(&WorkspaceSlotID::center())),
            RAIL_THICKNESS
        );
    }
}
