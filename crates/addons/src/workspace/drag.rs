//! Drag/drop resolution for workspace tabs.
//!
//! Swift resolves a drag against *live node geometry*: it finds nodes by their
//! debug names in the materialised tree and intersects frames. That is the only
//! way the preview matches what the user sees, since tab widths come from text
//! measurement. Rust keeps the same approach: this module only needs a way to
//! look up a node by debug name and the absolute frame of either it or its
//! ancestors.
use super::controller::{WorkspaceCommand, WorkspaceController};
use super::ids::{WorkspaceEdge, WorkspacePanelID, WorkspaceSlotID, WorkspaceTabGroupID};
use super::model::{WorkspaceDocument, WorkspaceTabGroup};
use super::theme::WorkspaceTheme;
use cove_composition::LayoutDebugAttachmentKey;
use cove_core::Rect;
use cove_core::scene::Node;

pub const DRAG_OVERLAY_ROOT_KEY: &str = "__workspace_drag_overlay_root";
pub const DRAG_OVERLAY_STATE_KEY: &str = "__workspace_tab_drag_overlay";

/// Distance a pointer must travel before a press becomes a drag.
pub const DRAG_THRESHOLD: f32 = cove_ui_compose::primitives::interaction::DRAG_THRESHOLD;

#[derive(Clone, Debug, PartialEq)]
pub struct WorkspaceReorderPreview {
    pub group_id: WorkspaceTabGroupID,
    pub target_index: usize,
    pub line_frame: Rect,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorkspaceDropPreview {
    pub target: super::controller::WorkspaceTarget,
    pub frame: Rect,
}

#[derive(Clone, Debug, PartialEq)]
pub enum WorkspaceDragResolution {
    Noop,
    Reorder(WorkspaceReorderPreview),
    Drop(WorkspaceDropPreview),
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorkspaceTabDragOverlayState {
    pub cursor_x: f32,
    pub cursor_y: f32,
    pub title: String,
    pub resolution: Option<WorkspaceDragResolution>,
}

/// Find the node a drag resolves against. The workspace root marks itself, so
/// a rename of the workspace debug name cannot break the lookup.
pub fn workspace_root(from: &Node) -> Option<Node> {
    let mut cursor = Some(from.clone());
    let mut fallback = None;
    while let Some(current) = cursor {
        if current.attachment::<bool>(DRAG_OVERLAY_ROOT_KEY) == Some(true) {
            return Some(current);
        }
        fallback = Some(current.clone());
        cursor = current.parent();
    }
    fallback
}

pub fn first_node(rooted_at: &Node, debug_name: &str) -> Option<Node> {
    if root_has_debug_name(rooted_at, debug_name) {
        return Some(rooted_at.clone());
    }
    for child in rooted_at.children() {
        if let Some(found) = first_node(&child, debug_name) {
            return Some(found);
        }
    }
    None
}

fn root_has_debug_name(node: &Node, debug_name: &str) -> bool {
    node.attachment::<String>(LayoutDebugAttachmentKey::DEBUG_NAME)
        .as_deref()
        == Some(debug_name)
}

/// Union of a group's tab frames widened by a forgiving band. A drop inside the
/// band is a reorder, not a split.
pub fn tab_band(group: &WorkspaceTabGroup, rooted_at: &Node) -> Option<Rect> {
    let frames = tab_frames(group, rooted_at);
    let (_, first) = frames.first()?;
    let mut union = *first;
    for (_, frame) in frames.iter().skip(1) {
        union = union.union(*frame);
    }
    Some(inset(union, -10.0, -8.0))
}

pub fn tab_frames(group: &WorkspaceTabGroup, rooted_at: &Node) -> Vec<(WorkspacePanelID, Rect)> {
    group
        .panels
        .iter()
        .filter_map(|panel_id| {
            let node = first_node(
                rooted_at,
                &format!("workspace-tab-{}", panel_id.raw_value()),
            )?;
            let frame = node.absolute_frame();
            (frame.width > 0.0 && frame.height > 0.0).then(|| (panel_id.clone(), frame))
        })
        .collect()
}

fn inset(rect: Rect, dx: f32, dy: f32) -> Rect {
    Rect::new(
        rect.x + dx,
        rect.y + dy,
        (rect.width - dx * 2.0).max(0.0),
        (rect.height - dy * 2.0).max(0.0),
    )
}

fn inset_by(rect: Rect, fraction_x: f32, fraction_y: f32) -> Rect {
    inset(rect, rect.width * fraction_x, rect.height * fraction_y)
}

pub fn resolve_drag(
    x: f32,
    y: f32,
    source_group_id: &WorkspaceTabGroupID,
    source_panel_id: &WorkspacePanelID,
    document: &WorkspaceDocument,
    from: &Node,
) -> Option<WorkspaceDragResolution> {
    if let Some(resolution) =
        resolve_tab_reorder(x, y, source_group_id, source_panel_id, document, from)
    {
        return Some(resolution);
    }
    resolve_drop_preview(x, y, source_group_id, source_panel_id, document, from)
        .map(WorkspaceDragResolution::Drop)
}

fn resolve_tab_reorder(
    x: f32,
    y: f32,
    source_group_id: &WorkspaceTabGroupID,
    source_panel_id: &WorkspacePanelID,
    document: &WorkspaceDocument,
    from: &Node,
) -> Option<WorkspaceDragResolution> {
    let root = workspace_root(from)?;
    let group = document.groups.get(source_group_id)?;
    let group_node = first_node(
        &root,
        &format!("workspace-group-{}", source_group_id.raw_value()),
    )?;
    if !group_node
        .absolute_frame()
        .contains(cove_core::Point { x, y })
    {
        return None;
    }
    let frames = tab_frames(group, &root);
    let (_, first) = frames.first()?;
    let mut band = *first;
    for (_, frame) in frames.iter().skip(1) {
        band = band.union(*frame);
    }
    let band = inset(band, -10.0, -8.0);
    if !band.contains(cove_core::Point { x, y }) {
        return None;
    }
    let old_index = group.panels.iter().position(|id| id == source_panel_id)?;
    if group.panels.len() <= 1 {
        return Some(WorkspaceDragResolution::Noop);
    }
    let target_index = frames
        .iter()
        .position(|(_, frame)| x < frame.x + frame.width * 0.5)
        .unwrap_or(group.panels.len());
    // Dropping into your own slot is a no-op, not a reorder.
    if target_index == old_index || target_index == old_index + 1 {
        return Some(WorkspaceDragResolution::Noop);
    }
    let line_x = if target_index == 0 {
        frames[0].1.x
    } else if target_index >= frames.len() {
        let last = frames[frames.len() - 1].1;
        last.x + last.width
    } else {
        frames[target_index].1.x
    };
    Some(WorkspaceDragResolution::Reorder(WorkspaceReorderPreview {
        group_id: source_group_id.clone(),
        target_index,
        line_frame: Rect::new(
            line_x - 1.5,
            band.y + 5.0,
            3.0,
            (band.height - 10.0).max(12.0),
        ),
    }))
}

fn resolve_drop_preview(
    x: f32,
    y: f32,
    source_group_id: &WorkspaceTabGroupID,
    source_panel_id: &WorkspacePanelID,
    document: &WorkspaceDocument,
    from: &Node,
) -> Option<WorkspaceDropPreview> {
    let root = workspace_root(from)?;
    let point = cove_core::Point { x, y };
    let visible_group_ids: Vec<WorkspaceTabGroupID> = document
        .slots
        .values()
        .flat_map(|slot| {
            slot.layout
                .as_ref()
                .map(|layout| layout.leaf_group_ids())
                .unwrap_or_default()
                .into_iter()
                .filter(|id| {
                    document
                        .groups
                        .get(id)
                        .is_some_and(|group| !group.is_collapsed)
                })
        })
        .collect();
    for group_id in visible_group_ids {
        let Some(group_node) =
            first_node(&root, &format!("workspace-group-{}", group_id.raw_value()))
        else {
            continue;
        };
        let frame = group_node.absolute_frame();
        if !frame.contains(point) {
            continue;
        }
        let Some(slot_id) = document.slot_containing(&group_id) else {
            continue;
        };
        if let Some(group) = document.groups.get(&group_id)
            && let Some(band) = tab_band(group, &root)
            && band.contains(point)
        {
            return Some(WorkspaceDropPreview {
                target: super::controller::WorkspaceTarget::tab_group(slot_id, group_id, None),
                frame: inset_by(frame, 0.24, 0.24),
            });
        }
        let mut preview = drop_preview(frame, point, slot_id.clone(), group_id.clone());
        if &group_id == source_group_id
            && preview.target.placement == super::ids::WorkspacePlacementKind::TabGroup
        {
            preview.target = super::controller::WorkspaceTarget::tab_group(slot_id, group_id, None);
        }
        return Some(preview);
    }
    for slot_id in document.slots.keys() {
        let draggable = document
            .panels
            .get(source_panel_id)
            .is_none_or(|panel| panel.is_draggable);
        if slot_id != &WorkspaceSlotID::center() && !draggable {
            continue;
        }
        let Some(slot_node) =
            first_node(&root, &format!("workspace-region-{}", slot_id.raw_value()))
        else {
            continue;
        };
        let frame = slot_node.absolute_frame();
        if frame.contains(point) {
            return Some(WorkspaceDropPreview {
                target: super::controller::WorkspaceTarget::slot_target(slot_id.clone(), None),
                frame,
            });
        }
    }
    None
}

fn drop_preview(
    frame: Rect,
    point: cove_core::Point,
    slot_id: WorkspaceSlotID,
    group_id: WorkspaceTabGroupID,
) -> WorkspaceDropPreview {
    let local_x = (point.x - frame.x) / frame.width.max(1.0);
    let local_y = (point.y - frame.y) / frame.height.max(1.0);
    let edge = 0.24_f32;
    let split = |edge_kind: WorkspaceEdge, preview_frame: Rect| WorkspaceDropPreview {
        target: super::controller::WorkspaceTarget::split(
            slot_id.clone(),
            group_id.clone(),
            edge_kind,
            0.5,
        ),
        frame: preview_frame,
    };
    if local_x < edge {
        return split(
            WorkspaceEdge::Leading,
            Rect::new(frame.x, frame.y, frame.width * edge, frame.height),
        );
    }
    if local_x > 1.0 - edge {
        let width = frame.width * edge;
        return split(
            WorkspaceEdge::Trailing,
            Rect::new(frame.x + frame.width - width, frame.y, width, frame.height),
        );
    }
    if local_y < edge {
        return split(
            WorkspaceEdge::Top,
            Rect::new(frame.x, frame.y, frame.width, frame.height * edge),
        );
    }
    if local_y > 1.0 - edge {
        let height = frame.height * edge;
        return split(
            WorkspaceEdge::Bottom,
            Rect::new(
                frame.x,
                frame.y + frame.height - height,
                frame.width,
                height,
            ),
        );
    }
    WorkspaceDropPreview {
        target: super::controller::WorkspaceTarget::tab_group(slot_id, group_id, None),
        frame: inset_by(frame, edge, edge),
    }
}

/// Persist the overlay on the workspace root so the root's draw hook can see it.
pub fn set_drag_overlay(state: Option<&WorkspaceTabDragOverlayState>, root: &Node) {
    match state {
        Some(state) => root.set_attachment(DRAG_OVERLAY_STATE_KEY, state.clone()),
        None => root.remove_attachment(DRAG_OVERLAY_STATE_KEY),
    }
    root.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
        "workspaceTabDragOverlay",
    ));
}

pub fn drag_overlay(root: &Node) -> Option<WorkspaceTabDragOverlayState> {
    root.attachment::<WorkspaceTabDragOverlayState>(DRAG_OVERLAY_STATE_KEY)
}

/// Apply a completed drag to the controller, matching Swift's `switch`.
pub fn apply_resolution(
    resolution: Option<WorkspaceDragResolution>,
    panel_id: &WorkspacePanelID,
    controller: &mut WorkspaceController,
) {
    let Some(resolution) = resolution else {
        return;
    };
    match resolution {
        WorkspaceDragResolution::Noop => {}
        WorkspaceDragResolution::Reorder(preview) => {
            controller.dispatch(WorkspaceCommand::ReorderPanel {
                panel_id: panel_id.clone(),
                group_id: preview.group_id,
                index: preview.target_index,
            });
        }
        WorkspaceDragResolution::Drop(preview) => {
            controller.dispatch(WorkspaceCommand::MovePanel {
                panel_id: panel_id.clone(),
                target: preview.target,
            });
        }
    }
}

/// Draw the drag preview. Mirrors Swift's overlay painting: accent-tinted
/// target box, an edge bar for split targets, a reorder caret, then the ghost.
pub fn draw_overlay(
    state: &WorkspaceTabDragOverlayState,
    root: &Node,
    theme: &WorkspaceTheme,
    accent: cove_core::Color,
    surface_floating: cove_core::Color,
    on_surface: cove_core::Color,
    list: &mut cove_core::DrawList,
) {
    match &state.resolution {
        Some(WorkspaceDragResolution::Reorder(preview)) => draw_reorder(preview, accent, list),
        Some(WorkspaceDragResolution::Drop(preview)) => draw_drop(preview, accent, list),
        Some(WorkspaceDragResolution::Noop) | None => {}
    }
    draw_ghost(
        state,
        root,
        theme,
        accent,
        surface_floating,
        on_surface,
        list,
    );
}

fn draw_reorder(
    preview: &WorkspaceReorderPreview,
    accent: cove_core::Color,
    list: &mut cove_core::DrawList,
) {
    let rect = preview.line_frame;
    if rect.width <= 0.0 || rect.height <= 0.0 {
        return;
    }
    let _ = list.add_rounded_rect(rect, 1.5, accent.multiplied_alpha(0.98));
    let _ = list.add_rounded_rect(
        Rect::new(rect.x - 3.0, rect.y, rect.width + 6.0, 3.0),
        1.5,
        accent,
    );
    let _ = list.add_rounded_rect(
        Rect::new(
            rect.x - 3.0,
            rect.y + rect.height - 3.0,
            rect.width + 6.0,
            3.0,
        ),
        1.5,
        accent,
    );
}

fn draw_drop(
    preview: &WorkspaceDropPreview,
    accent: cove_core::Color,
    list: &mut cove_core::DrawList,
) {
    let inset_amount = if preview.target.placement == super::ids::WorkspacePlacementKind::Slot {
        5.0
    } else {
        3.0
    };
    let rect = inset(preview.frame, inset_amount, inset_amount);
    if rect.width <= 2.0 || rect.height <= 2.0 {
        return;
    }
    let fill_alpha = if preview.target.placement == super::ids::WorkspacePlacementKind::TabGroup {
        0.10
    } else {
        0.16
    };
    let _ = list.add_rounded_rect(rect, 7.0, accent.multiplied_alpha(fill_alpha));
    add_border(rect, accent.multiplied_alpha(0.92), 2.0, list);
    if preview.target.placement != super::ids::WorkspacePlacementKind::Split {
        return;
    }
    let Some(edge) = preview.target.edge else {
        return;
    };
    let thickness = 5.0;
    let bar = match edge {
        WorkspaceEdge::Leading => Rect::new(rect.x, rect.y, thickness, rect.height),
        WorkspaceEdge::Trailing => Rect::new(
            rect.x + rect.width - thickness,
            rect.y,
            thickness,
            rect.height,
        ),
        WorkspaceEdge::Top => Rect::new(rect.x, rect.y, rect.width, thickness),
        WorkspaceEdge::Bottom => Rect::new(
            rect.x,
            rect.y + rect.height - thickness,
            rect.width,
            thickness,
        ),
    };
    let _ = list.add_rounded_rect(bar, 2.5, accent.multiplied_alpha(0.98));
}

fn add_border(rect: Rect, color: cove_core::Color, thickness: f32, list: &mut cove_core::DrawList) {
    let t = thickness.max(1.0);
    let _ = list.add_rect(Rect::new(rect.x, rect.y, rect.width, t), color);
    let _ = list.add_rect(
        Rect::new(rect.x, rect.y + rect.height - t, rect.width, t),
        color,
    );
    let _ = list.add_rect(Rect::new(rect.x, rect.y, t, rect.height), color);
    let _ = list.add_rect(
        Rect::new(rect.x + rect.width - t, rect.y, t, rect.height),
        color,
    );
}

fn cleaned_title(title: &str) -> String {
    let clean = title.replace('\n', " ");
    let clean = clean.trim();
    if clean.is_empty() {
        "Panel".to_string()
    } else {
        clean.to_string()
    }
}

fn draw_ghost(
    state: &WorkspaceTabDragOverlayState,
    root: &Node,
    _theme: &WorkspaceTheme,
    accent: cove_core::Color,
    surface_floating: cove_core::Color,
    on_surface: cove_core::Color,
    list: &mut cove_core::DrawList,
) {
    let title = cleaned_title(&state.title);
    // Swift measures the real text and clamps to 170pt. The estimated advance
    // matches its fallback when no text environment is available, which is the
    // only case the overlay itself is drawn without glyphs.
    let text_width = (title.chars().count() as f32 * 7.0).clamp(42.0, 170.0);
    let width = (text_width + 34.0).clamp(112.0, 220.0);
    let height = 28.0;
    let bounds = root.absolute_frame();
    let min_x = bounds.x + 6.0;
    let min_y = bounds.y + 6.0;
    let max_x = (bounds.x + bounds.width - width - 6.0).max(min_x);
    let max_y = (bounds.y + bounds.height - height - 6.0).max(min_y);
    let x = (state.cursor_x + 14.0).clamp(min_x, max_x);
    let y = (state.cursor_y + 10.0).clamp(min_y, max_y);
    let rect = Rect::new(x, y, width, height);

    let _ = list.add_rounded_rect(rect, 6.0, surface_floating.multiplied_alpha(0.96));
    add_border(rect, accent.multiplied_alpha(0.55), 1.0, list);
    let _ = list.add_rounded_rect(
        Rect::new(rect.x + 7.0, rect.y + 6.0, 3.0, rect.height - 12.0),
        1.5,
        accent.multiplied_alpha(0.95),
    );
    let _ = on_surface;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drag_threshold_matches_the_shared_activation_rule() {
        assert_eq!(
            DRAG_THRESHOLD,
            cove_ui_compose::primitives::interaction::DRAG_THRESHOLD
        );
    }

    #[test]
    fn title_cleanup_replaces_newlines_and_falls_back() {
        assert_eq!(cleaned_title("a\nb"), "a b");
        assert_eq!(cleaned_title("   "), "Panel");
    }

    #[test]
    fn drop_preview_prefers_the_leading_edge_band() {
        let frame = Rect::new(0.0, 0.0, 100.0, 100.0);
        let preview = drop_preview(
            frame,
            cove_core::Point { x: 2.0, y: 50.0 },
            WorkspaceSlotID::leading(),
            WorkspaceTabGroupID::new("g"),
        );
        assert_eq!(preview.target.edge, Some(WorkspaceEdge::Leading));
        assert_eq!(preview.frame.width, 24.0);
    }
}
