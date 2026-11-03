//! Workspace view: panes, tab bars, split dividers, rails and floating windows.
//!
//! Swift expressed this with `Box`/`Row`/`Text`/`ScrollView` widgets. Those
//! Compose primitives are still being ported, so each structural piece here is
//! a `PrimitiveView` that owns its node geometry directly. That keeps the
//! layout contract (flex weights, divider hairlines, negative-margin hit slop)
//! byte-for-byte with the Swift version while depending only on the stable
//! composition + scene layers.
use super::ids::{WorkspaceEdge, WorkspaceSlotID};
use super::model::{
    WorkspaceDocument, WorkspaceLayoutNode, WorkspaceSlot, WorkspaceSlotKind, WorkspaceTabGroup,
};
use super::theme::{WorkspaceRegionSizing, WorkspaceTheme};
use cove_composition::{
    AnyView, LayoutDebugAttachmentKey, PrimitiveView, View, impl_primitive_view,
};
use cove_core::Color;
use cove_core::scene::{
    Align, FlexDirection, FocusChainHolder, Justify, LayoutNode, Node, PointerCaptureHolder,
};
use cove_ui_compose::theme::tokens::SemanticColorRef;

pub(crate) const RESIZE_GEOMETRY_KEY: &str = "workspace.resize.geometry";
const RAIL_THICKNESS: f32 = 40.0;

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
        node.set_clips_to_bounds(true);
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
        let theme = super::theme::resolve_workspace_theme(node);
        let target = if self.is_visible { self.sizing.extent(&theme) } else { 0.0 };
        let previous = node.attachment::<f32>("workspace.region.extent");
        let old_visible = node.attachment::<bool>("workspace.region.visible");
        node.set_attachment("workspace.region.visible", self.is_visible);
        if self.is_visible && let Some(content) = &self.content {
            node.set_attachment("workspace.region.content", content.clone());
        }
        let revision = node.attachment::<cove_core::Signal<u64>>("workspace.region.revision").unwrap_or_else(|| {
            let signal = cove_core::Signal::detached(0);
            node.set_attachment("workspace.region.revision", signal);
            signal
        });
        let weak = node.downgrade();
        let horizontal = self.horizontal;
        let sizing = self.sizing;
        let visible = self.is_visible;
        let animate = theme.animates_layout && (old_visible.is_some_and(|old| old != visible) || node.has_animation_controller("workspace.region.extent"));
        if let (Some(extent), Some(layout)) = (previous, node.layout_node()) { apply_region_extent(&layout, sizing, horizontal, extent, &theme); }
        cove_core::scene::with_animation(animate.then(|| cove_ui_compose::theme::SemanticMotionRef::medium().resolve(&cove_ui_compose::theme::theme_for_node(node))), || {
            node.animatable_set_with("workspace.region.extent", previous.unwrap_or(target), target, move |extent| {
                let Some(owner) = Node::upgrade(&weak) else { return };
                owner.set_attachment("workspace.region.extent", extent);
                owner.set_opacity(if extent > 0.0001 { 1.0 } else { 0.0 });
                if let Some(layout) = owner.layout_node() {
                    apply_region_extent(&layout, sizing, horizontal, extent, &theme);
                }
                if !visible && extent <= 0.0001 { revision.notify(); }
            });
        });
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
    fn children(&self, node: &Node) -> Vec<AnyView> {
        if let Some(revision) = node.attachment::<cove_core::Signal<u64>>("workspace.region.revision") { revision.track(); }
        if self.is_visible { return self.content.iter().cloned().collect(); }
        if node.attachment::<f32>("workspace.region.extent").unwrap_or(0.0) > 0.0001 {
            node.attachment::<AnyView>("workspace.region.content").into_iter().collect()
        } else { Vec::new() }
    }
}

impl WorkspaceRegionTransition {
    fn apply_extent(&self, layout: &LayoutNode) {
        let target = if self.is_visible {
            self.sizing.extent(&WorkspaceTheme::default())
        } else {
            0.0
        };
        apply_region_extent(layout, self.sizing, self.horizontal, target, &WorkspaceTheme::default());
    }
}
fn apply_region_extent(layout: &LayoutNode, sizing: WorkspaceRegionSizing, horizontal: bool, target: f32, theme: &WorkspaceTheme) {
        match sizing {
            WorkspaceRegionSizing::Fraction(_) => {
                layout.set_flex_basis(0.0);
                layout.set_flex_grow(target);
            }
            WorkspaceRegionSizing::Points(_) => {
                layout.set_flex_grow(0.0);
                layout.set_flex_shrink(0.0);
                if horizontal {
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
                let slop = theme.split_divider_hit_slop;
                let span = target + 2.0 * slop;
                if horizontal {
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
    use super::super::ids::{WorkspacePanelID, WorkspaceTabGroupID};
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
