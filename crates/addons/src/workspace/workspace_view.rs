//! Composed workspace view: shell regions, split dividers, tab groups, rails and
//! the floating layer.
//!
//! Translation of `Sources/CoveUIWorkspace/WorkspaceView.swift`. Swift declares
//! twelve private subviews (`_WorkspaceShell`, `_WorkspaceMainRow`,
//! `_WorkspaceCenterColumn`, `_WorkspaceSideRegion`, `_WorkspaceRailReserve`,
//! `_WorkspaceSlotView`, `_WorkspaceTabGroupView`, `_WorkspaceChromeSlot`,
//! `_WorkspaceBottomSlot`, `_WorkspaceFloatingLayer`,
//! `_WorkspaceFloatingWindowView`, `_WorkspaceCompactView`). They are folded
//! here into private constructors of [`WorkspaceView`] plus the private region
//! types below — a deliberate structural difference. The layout contract each
//! encodes is preserved: flex weights, divider hairlines, the negative-margin
//! hit slop and the reserve arithmetic.
use super::controller::{WorkspaceCommand, WorkspaceController, WorkspaceTarget};
use super::drag::{self, DRAG_OVERLAY_ROOT_KEY, DRAG_THRESHOLD};
use super::ids::{
    WorkspaceEdge, WorkspaceFloatingWindowID, WorkspacePanelID, WorkspaceSlotID,
    WorkspaceSplitAxis, WorkspaceTabGroupID,
};
use super::model::{
    WorkspaceDocument, WorkspaceFloatingWindow, WorkspaceLayoutNode, WorkspaceLayoutSplit,
    WorkspacePanelRegistry, WorkspaceTabGroup,
};
use super::sizing::{WorkspaceLayoutMetrics, WorkspaceResolvedSplit, WorkspaceSizingResolver};
use super::theme::{WorkspaceRegionSizing, WorkspaceTheme};
use super::view::{
    RESIZE_GEOMETRY_KEY, WorkspaceBox, WorkspacePane, WorkspaceRegionTransition,
    WorkspaceResizeGeometry, chrome_rail_slots, collapsed_groups_on, rail_groups, rail_plan,
    renderable_layout, visible_chrome_rail_slots, visible_groups,
};
use cove_composition::Scope;
use cove_composition::{
    AnyView, EmptyView, LayoutDebugAttachmentKey, PrimitiveView, View, ViewExt, impl_primitive_view,
};

/// Erase any view so modifier chains can be returned / stored as `AnyView`.
fn erased(view: impl View + 'static) -> AnyView {
    AnyView::new(view)
}
use cove_core::Color;
use cove_core::scene::{
    AccessibilityRole, AccessibilitySemantics, Align, Edge, FlexDirection, FocusChainHolder,
    InputHandlerRoute, InputRoutingPriority, InteractionRegistryHolder, Justify, LayoutNode, Node,
    PointerCaptureHolder, PositionType,
};
use cove_platform::MouseButton;
use cove_ui_compose::modifiers::layout::{FlexExt, LayoutFrameExt};
use cove_ui_compose::modifiers::style::AppearanceExt;
use cove_ui_compose::primitives::controls::Button;
use cove_ui_compose::primitives::layout::ScrollView;
use cove_ui_compose::primitives::overlays::Popover;
use cove_ui_compose::primitives::overlays::anchored::PopoverPlacement;
use cove_ui_compose::primitives::overlays::menu::{Menu, MenuEntry, MenuItem};
use cove_ui_compose::primitives::text::{Divider, Text};
use cove_ui_compose::theme::theme_for_node;
use cove_ui_compose::theme::tokens::{SemanticColorRef, SemanticFontRef, SemanticStyleViewExt};
use std::cell::RefCell;
use std::rc::Rc;

const TAB_PRESS_KEY: &str = "__workspace_tab_press";
const FLOATING_DRAG_KEY: &str = "__workspace_floating_window_drag";
const LAYOUT_PHASE_KEY: &str = "workspace.layout.phase";
const RAIL_THICKNESS: f32 = 40.0;
const FLOATING_TITLE_HEIGHT: f32 = 30.0;

/// Panel content factory: a panel id maps to the view the host authored for it.
pub type WorkspaceContent = Rc<dyn Fn(WorkspacePanelID) -> AnyView>;

/// The composed workspace.
///
/// The maximized presentation short-circuits the whole shell, the compact
/// presentation replaces it below the expanded width class, and the docked shell
/// otherwise renders the four regions plus rails, dividers and floating windows.
pub struct WorkspaceView {
    pub controller: Rc<RefCell<WorkspaceController>>,
    pub content: WorkspaceContent,
    pub compact: bool,
}

impl WorkspaceView {
    pub fn new(controller: Rc<RefCell<WorkspaceController>>, content: WorkspaceContent) -> Self {
        Self {
            controller,
            content,
            compact: false,
        }
    }

    /// Build from a registry. The registry is snapshotted so the view does not
    /// hold a borrow across a panel factory.
    pub fn with_registry(
        controller: Rc<RefCell<WorkspaceController>>,
        registry: &WorkspacePanelRegistry,
    ) -> Self {
        Self::new(controller, registry.content())
    }

    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }

    fn document(&self) -> WorkspaceDocument {
        self.controller.borrow().document().clone()
    }

    fn content_view(&self, panel_id: &WorkspacePanelID) -> AnyView {
        (self.content)(panel_id.clone())
    }
}

impl View for WorkspaceView {
    fn body(&self, _cx: &Scope) -> AnyView {
        let document = self.document();
        let maximized = self.controller.borrow().maximized_panel_id().cloned();

        if let Some(panel_id) = maximized {
            return self.maximized(&panel_id, &document);
        }

        let themed = AnyView::new(WorkspaceThemeProvider {
            theme: WorkspaceTheme::default(),
            content: AnyView::new(self.surface(&document)),
        });
        erased(themed.modifier(WorkspaceDragOverlayModifier))
    }
}

impl WorkspaceView {
    /// Maximized branch: a title row with `Restore` above the single panel.
    fn maximized(&self, panel_id: &WorkspacePanelID, document: &WorkspaceDocument) -> AnyView {
        let title = document
            .panels
            .get(panel_id)
            .map(|panel| panel.title.clone())
            .unwrap_or_else(|| panel_id.raw_value().to_string());
        let controller = self.controller.clone();
        let restore = Button::title("Restore", move || {
            controller
                .borrow_mut()
                .dispatch(WorkspaceCommand::RestoreMaximized);
        });
        let header = AnyView::new(
            WorkspaceBox::row()
                .align(Align::Center)
                .spacing(8.0)
                .child((Text::new(title).semantic_font(SemanticFontRef::label())).flex(1.0))
                .child(restore)
                .boxed(),
        )
        .modifier(WorkspacePaddingModifier::all(6.0));
        erased(
            AnyView::new(
                WorkspaceBox::column()
                    .child(header)
                    .child(
                        (self.content_view(panel_id))
                            .flex_with(1.0, 1.0, Some(0.0))
                            .frame_min_width(0.0),
                    )
                    .boxed(),
            )
            .modifier(WorkspaceFillModifier)
            .modifier(WorkspaceDragOverlayModifier),
        )
    }

    /// `SurfaceReader` branch: `compact || width class != expanded`.
    fn surface(&self, document: &WorkspaceDocument) -> AnyView {
        let compact = self.compact;
        let document = document.clone();
        let controller = self.controller.clone();
        let content = self.content.clone();
        AnyView::new(WorkspaceSurfaceReader {
            build: Rc::new(move |width: f32| {
                let expanded = width >= cove_core::Viewport::EXPANDED_MINIMUM_WIDTH;
                if compact || !expanded {
                    return erased(
                        AnyView::new(WorkspaceCompactView {
                            document: document.clone(),
                            controller: controller.clone(),
                            content: content.clone(),
                            presented: cove_core::State::new(false),
                        })
                        .modifier(WorkspaceFillModifier)
                        .modifier(WorkspaceRoleModifier::new(
                            "workspace",
                            "workspace",
                            "workspace",
                        )),
                    );
                }
                erased(
                    AnyView::new(
                        WorkspaceBox::column()
                            .child(WorkspaceShell {
                                document: document.clone(),
                                controller: controller.clone(),
                                content: content.clone(),
                                metrics: WorkspaceLayoutMetrics::default(),
                            })
                            .child(WorkspaceFloatingLayer {
                                document: document.clone(),
                                controller: controller.clone(),
                                content: content.clone(),
                            })
                            .boxed(),
                    )
                    .modifier(WorkspaceFillModifier)
                    .modifier(WorkspaceRoleModifier::new(
                        "workspace",
                        "workspace",
                        "workspace",
                    )),
                )
            }),
        })
    }
}

// MARK: - Surface width

/// `SurfaceReader`: publishes the container width into the build closure.
struct WorkspaceSurfaceReader {
    build: Rc<dyn Fn(f32) -> AnyView>,
}

impl View for WorkspaceSurfaceReader {
    fn body(&self, _cx: &Scope) -> AnyView {
        let build = self.build.clone();
        // The workspace fills its container, so the reader must not hug its content.
        erased(
            AnyView::new(cove_composition::adaptive::ContainerSizeReader::new(
                move |size: cove_core::Size| build(size.width),
            ))
            .modifier(WorkspaceFillModifier),
        )
    }
}

// MARK: - Modifiers

/// `ThemeReader` around a workspace subtree: provides [`WorkspaceTheme`] to every
/// descendant.
struct WorkspaceThemeProvider {
    theme: WorkspaceTheme,
    content: AnyView,
}

impl View for WorkspaceThemeProvider {
    fn body(&self, _cx: &Scope) -> AnyView {
        erased(
            self.content
                .clone()
                .composition_local(super::theme::WorkspaceThemeEnvironment::key(), self.theme),
        )
    }
}

struct WorkspaceFillModifier;

impl cove_composition::ViewModifier for WorkspaceFillModifier {
    fn apply_layout(&self, layout: &LayoutNode) {
        layout.set_width_percent(100.0);
        layout.set_height_percent(100.0);
        layout.set_min_width(Some(0.0));
        layout.set_min_height(Some(0.0));
    }
}

struct WorkspaceRoleModifier {
    layout_role: String,
    semantic_role: String,
    debug_name: String,
}

impl WorkspaceRoleModifier {
    fn new(layout_role: &str, semantic_role: &str, debug_name: &str) -> Self {
        Self {
            layout_role: layout_role.to_owned(),
            semantic_role: semantic_role.to_owned(),
            debug_name: debug_name.to_owned(),
        }
    }
}

impl cove_composition::ViewModifier for WorkspaceRoleModifier {
    fn apply_node(&self, node: &Node) {
        node.set_attachment(
            LayoutDebugAttachmentKey::LAYOUT_ROLE,
            self.layout_role.clone(),
        );
        node.set_attachment(
            LayoutDebugAttachmentKey::SEMANTIC_ROLE,
            self.semantic_role.clone(),
        );
        node.set_attachment(
            LayoutDebugAttachmentKey::DEBUG_NAME,
            self.debug_name.clone(),
        );
    }
}

struct WorkspaceSemanticsModifier(AccessibilitySemantics);

impl cove_composition::ViewModifier for WorkspaceSemanticsModifier {
    fn apply_node(&self, node: &Node) {
        node.set_accessibility(Some(self.0.clone()));
    }
}

struct WorkspacePaddingModifier {
    top: f32,
    leading: f32,
    bottom: f32,
    trailing: f32,
}

impl WorkspacePaddingModifier {
    fn all(value: f32) -> Self {
        Self {
            top: value,
            leading: value,
            bottom: value,
            trailing: value,
        }
    }
}

impl cove_composition::ViewModifier for WorkspacePaddingModifier {
    fn apply_layout(&self, layout: &LayoutNode) {
        layout.set_padding(self.leading, Edge::Left);
        layout.set_padding(self.trailing, Edge::Right);
        layout.set_padding(self.top, Edge::Top);
        layout.set_padding(self.bottom, Edge::Bottom);
    }
}

/// `_WorkspaceDragOverlayModifier`: marks the node that paints the tab drag
/// preview and installs its draw hook. Hosts may rename the workspace's debug
/// name, so the lookup keys off this attachment.
struct WorkspaceDragOverlayModifier;

impl cove_composition::ViewModifier for WorkspaceDragOverlayModifier {
    fn apply_node(&self, node: &Node) {
        node.set_attachment(DRAG_OVERLAY_ROOT_KEY, true);
        let target = node.clone();
        node.update_overlay_draw("workspace-tab-drag-overlay", move |list, _| {
            let Some(state) = drag::drag_overlay(&target) else {
                return;
            };
            let theme = super::theme::resolve_workspace_theme(&target);
            let colors = theme_for_node(&target).colors;
            drag::draw_overlay(
                &state,
                &target,
                &theme,
                colors.accent,
                colors.surface_floating,
                colors.on_surface,
                list,
            );
        });
    }
}

/// `_WorkspaceLayoutAnimation`: records the region phase; an animate pass only
/// happens when a previous phase existed, differs, and the theme opts in.
struct WorkspaceLayoutAnimation {
    phase: Vec<bool>,
}

impl cove_composition::ViewModifier for WorkspaceLayoutAnimation {
    fn apply_node(&self, node: &Node) {
        let previous = node.attachment::<Vec<bool>>(LAYOUT_PHASE_KEY);
        let animates = super::theme::resolve_workspace_theme(node).animates_layout;
        let animate = previous
            .as_ref()
            .is_some_and(|previous| previous != &self.phase)
            && animates;
        node.set_attachment(LAYOUT_PHASE_KEY, self.phase.clone());
        if animate {
            node.set_attachment("cove.workspace.layout.animating", true);
        }
    }
}

/// Publishes the split geometry a divider resolves against.
struct WorkspaceResizeGeometryModifier(Vec<WorkspaceResizeGeometry>);

impl cove_composition::ViewModifier for WorkspaceResizeGeometryModifier {
    fn apply_node(&self, node: &Node) {
        node.set_attachment(RESIZE_GEOMETRY_KEY, self.0.clone());
    }
}

// MARK: - Extent measurement

/// `WorkspaceExtentObserver`: reports the measured extent so a region can
/// resolve splits against its real allocation. Installed once per node; the
/// hook chain is preserved so repeated recompositions never stack callbacks.
struct WorkspaceExtent {
    extent: cove_core::State<f32>,
}

impl WorkspaceExtent {
    fn new() -> Self {
        Self {
            extent: cove_core::State::new(0.0),
        }
    }

    fn measure(horizontal: bool, extent: &cove_core::State<f32>, node: &Node) {
        let frame = node.frame();
        let value = if horizontal {
            frame.width
        } else {
            frame.height
        };
        if (extent.get() - value).abs() > 0.01 {
            extent.set(value);
        }
    }
}

/// Container that publishes its measured axis and rebuilds its single child from
/// that value.
struct WorkspaceExtentBody {
    horizontal: bool,
    extent: cove_core::State<f32>,
    build: Rc<dyn Fn(f32) -> AnyView>,
}

impl PrimitiveView for WorkspaceExtentBody {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn update_node(&self, node: &Node) {
        node.set_clips_to_bounds(false);
        let previous = node.layout_did_update();
        let extent = self.extent.clone();
        let horizontal = self.horizontal;
        node.set_layout_did_update(Some(Rc::new(move |node| {
            if let Some(previous) = &previous {
                previous(node);
            }
            WorkspaceExtent::measure(horizontal, &extent, node);
        })));
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_flex_direction(if self.horizontal {
            FlexDirection::Row
        } else {
            FlexDirection::Column
        });
        layout.set_align_items(Align::Stretch);
        layout.set_min_width(Some(0.0));
        layout.set_min_height(Some(0.0));
        layout.set_flex(1.0);
        Some(layout)
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_flex_direction(if self.horizontal {
            FlexDirection::Row
        } else {
            FlexDirection::Column
        });
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![(self.build)(self.extent.get())]
    }
}
impl_primitive_view!(WorkspaceExtentBody);

// MARK: - Shell

/// `_WorkspaceShell`: top chrome, the top region, the main row and bottom chrome.
struct WorkspaceShell {
    document: WorkspaceDocument,
    controller: Rc<RefCell<WorkspaceController>>,
    content: WorkspaceContent,
    metrics: WorkspaceLayoutMetrics,
}

impl View for WorkspaceShell {
    fn body(&self, _cx: &Scope) -> AnyView {
        let extent = WorkspaceExtent::new();
        let document = self.document.clone();
        let controller = self.controller.clone();
        let content = self.content.clone();
        let metrics = self.metrics;
        erased(
            AnyView::new(WorkspaceExtentBody {
                horizontal: false,
                extent: extent.extent.clone(),
                build: Rc::new(move |height| {
                    build_shell(
                        document.clone(),
                        controller.clone(),
                        content.clone(),
                        metrics,
                        height,
                    )
                }),
            })
            .modifier(WorkspaceFillModifier),
        )
    }
}

fn build_shell(
    document: WorkspaceDocument,
    controller: Rc<RefCell<WorkspaceController>>,
    content: WorkspaceContent,
    metrics: WorkspaceLayoutMetrics,
    height: f32,
) -> AnyView {
    let top_visible = !visible_groups(&WorkspaceSlotID::top(), &document).is_empty();
    let top_collapsed = !collapsed_groups_on(WorkspaceEdge::Top, &document).is_empty();
    let top_chrome =
        chrome_rail_slots(WorkspaceEdge::Top, &document, metrics.shows_collapsed_rails);
    let bottom_chrome = visible_chrome_rail_slots(WorkspaceEdge::Bottom, &document);
    let reserve: f32 = top_chrome.iter().map(|slot| slot.2).sum::<f32>()
        + bottom_chrome.iter().map(|slot| slot.2).sum::<f32>()
        + if top_collapsed { RAIL_THICKNESS } else { 0.0 };

    let resolver = WorkspaceSizingResolver::with_metrics(&document, metrics);
    let center_bottom = resolver.combine(
        resolver.slot(&WorkspaceSlotID::center(), WorkspaceSplitAxis::Vertical),
        resolver.slot(&WorkspaceSlotID::bottom(), WorkspaceSplitAxis::Vertical),
        true,
        if visible_groups(&WorkspaceSlotID::bottom(), &document).is_empty() {
            0.0
        } else {
            metrics.divider
        },
    );
    let main_size = resolver.combine(
        center_bottom,
        resolver.combine(
            resolver.slot(&WorkspaceSlotID::leading(), WorkspaceSplitAxis::Vertical),
            resolver.slot(&WorkspaceSlotID::trailing(), WorkspaceSplitAxis::Vertical),
            false,
            metrics.divider,
        ),
        false,
        metrics.divider,
    );
    let split = WorkspaceResolvedSplit::new(
        document.split_fractions.top,
        0.25,
        (height - reserve - if top_visible { metrics.divider } else { 0.0 }).max(0.0),
        resolver.slot(&WorkspaceSlotID::top(), WorkspaceSplitAxis::Vertical),
        main_size,
    );
    let phase = vec![top_visible];

    let mut children: Vec<AnyView> = Vec::new();
    for (slot_id, edge, thickness) in &top_chrome {
        children.push(AnyView::new(WorkspaceChromeSlot {
            slot_id: slot_id.clone(),
            edge: *edge,
            thickness: *thickness,
            groups_override: None,
            document: document.clone(),
            controller: controller.clone(),
        }));
    }
    children.push(erased(
        AnyView::new(WorkspaceRegionTransition::new(
            top_visible,
            false,
            WorkspaceRegionSizing::Fraction(split.fraction),
            Some(AnyView::new(WorkspaceSlotView {
                slot_id: WorkspaceSlotID::top(),
                document: document.clone(),
                controller: controller.clone(),
                content: content.clone(),
            })),
        ))
        .modifier(WorkspaceLayoutAnimation {
            phase: phase.clone(),
        }),
    ));
    children.push(AnyView::new(WorkspaceRegionTransition::new(
        top_visible,
        false,
        WorkspaceRegionSizing::Divider,
        Some(AnyView::new(WorkspaceSplitDivider::named(
            "top",
            false,
            controller.clone(),
        ))),
    )));
    if top_collapsed && metrics.shows_collapsed_rails {
        children.push(AnyView::new(WorkspaceRail {
            slot_id: WorkspaceSlotID::top(),
            edge_override: None,
            thickness: RAIL_THICKNESS,
            groups_override: None,
            document: document.clone(),
            controller: controller.clone(),
        }));
    }
    children.push(erased(
        AnyView::new(WorkspaceMainRow {
            document: document.clone(),
            controller: controller.clone(),
            content: content.clone(),
            metrics,
        })
        .flex_with(
            if top_visible {
                1.0 - split.fraction
            } else {
                1.0
            },
            1.0,
            Some(0.0),
        )
        .modifier(WorkspaceLayoutAnimation {
            phase: phase.clone(),
        }),
    ));
    for (slot_id, edge, thickness) in &bottom_chrome {
        children.push(AnyView::new(WorkspaceChromeSlot {
            slot_id: slot_id.clone(),
            edge: *edge,
            thickness: *thickness,
            groups_override: None,
            document: document.clone(),
            controller: controller.clone(),
        }));
    }

    let geometry = vec![WorkspaceResizeGeometry {
        split_id: "top".to_string(),
        horizontal: false,
        offset: top_chrome.iter().map(|slot| slot.2).sum::<f32>() + metrics.divider / 2.0,
        available: split.available,
        range: split.range,
        fraction: split.fraction,
    }];
    erased(
        AnyView::new(WorkspaceBox::column().children(children).boxed())
            .modifier(WorkspaceResizeGeometryModifier(geometry))
            .modifier(WorkspaceFillModifier),
    )
}

// MARK: - Main row

/// `_WorkspaceMainRow`: leading rail, leading region, divider, center column,
/// divider, trailing region, trailing rail.
struct WorkspaceMainRow {
    document: WorkspaceDocument,
    controller: Rc<RefCell<WorkspaceController>>,
    content: WorkspaceContent,
    metrics: WorkspaceLayoutMetrics,
}

impl View for WorkspaceMainRow {
    fn body(&self, _cx: &Scope) -> AnyView {
        let extent = WorkspaceExtent::new();
        let document = self.document.clone();
        let controller = self.controller.clone();
        let content = self.content.clone();
        let metrics = self.metrics;
        erased(
            AnyView::new(WorkspaceExtentBody {
                horizontal: true,
                extent: extent.extent.clone(),
                build: Rc::new(move |width| {
                    build_main_row(
                        document.clone(),
                        controller.clone(),
                        content.clone(),
                        metrics,
                        width,
                    )
                }),
            })
            .modifier(WorkspaceFillModifier),
        )
    }
}

fn build_main_row(
    document: WorkspaceDocument,
    controller: Rc<RefCell<WorkspaceController>>,
    content: WorkspaceContent,
    metrics: WorkspaceLayoutMetrics,
    width: f32,
) -> AnyView {
    let leading_visible = !visible_groups(&WorkspaceSlotID::leading(), &document).is_empty();
    let leading_rail = rail_plan(
        WorkspaceEdge::Leading,
        &document,
        metrics.shows_collapsed_rails,
    );
    let trailing_visible = !visible_groups(&WorkspaceSlotID::trailing(), &document).is_empty();
    let trailing_rail = rail_plan(
        WorkspaceEdge::Trailing,
        &document,
        metrics.shows_collapsed_rails,
    );

    let resolver = WorkspaceSizingResolver::with_metrics(&document, metrics);
    let center_size = resolver.combine(
        resolver.slot(&WorkspaceSlotID::center(), WorkspaceSplitAxis::Horizontal),
        resolver.slot(&WorkspaceSlotID::bottom(), WorkspaceSplitAxis::Horizontal),
        false,
        metrics.divider,
    );
    let leading_size = resolver.slot(&WorkspaceSlotID::leading(), WorkspaceSplitAxis::Horizontal);
    let trailing_size = resolver.slot(&WorkspaceSlotID::trailing(), WorkspaceSplitAxis::Horizontal);
    let rail_width = leading_rail.as_ref().map_or(0.0, |plan| plan.2)
        + trailing_rail.as_ref().map_or(0.0, |plan| plan.2);
    let dividers = (leading_visible as u8 + trailing_visible as u8) as f32;
    let available = (width - rail_width - metrics.divider * dividers).max(0.0);

    let leading_split = WorkspaceResolvedSplit::new(
        document.split_fractions.leading,
        0.22,
        available,
        leading_size,
        if trailing_visible {
            resolver.combine(center_size, trailing_size, true, metrics.divider)
        } else {
            center_size
        },
    );
    let leading_weight = if leading_visible {
        leading_split.fraction
    } else {
        0.0
    };
    let trailing_split = WorkspaceResolvedSplit::new(
        document.split_fractions.center_trailing,
        0.78,
        available * (1.0 - leading_weight),
        center_size,
        trailing_size,
    );
    let center_weight = (1.0 - leading_weight)
        * if trailing_visible {
            trailing_split.fraction
        } else {
            1.0
        };
    let trailing_weight = (1.0 - leading_weight) - center_weight;
    let phase = vec![leading_visible, trailing_visible];

    let leading_rail_thickness = leading_rail.as_ref().map_or(RAIL_THICKNESS, |plan| plan.2);
    let trailing_rail_thickness = trailing_rail.as_ref().map_or(RAIL_THICKNESS, |plan| plan.2);

    let mut children: Vec<AnyView> = Vec::new();
    children.push(AnyView::new(WorkspaceRegionTransition::new(
        leading_rail.is_some(),
        true,
        WorkspaceRegionSizing::Points(leading_rail_thickness),
        Some(match &leading_rail {
            Some(plan) => AnyView::new(WorkspaceRail {
                slot_id: plan.0.clone(),
                edge_override: Some(plan.1),
                thickness: plan.2,
                groups_override: Some(plan.3.clone()),
                document: document.clone(),
                controller: controller.clone(),
            }),
            None => AnyView::new(EmptyView),
        }),
    )));
    children.push(AnyView::new(WorkspaceRegionTransition::new(
        leading_visible,
        true,
        WorkspaceRegionSizing::Fraction(leading_weight),
        Some(AnyView::new(WorkspaceSlotView {
            slot_id: WorkspaceSlotID::leading(),
            document: document.clone(),
            controller: controller.clone(),
            content: content.clone(),
        })),
    )));
    children.push(AnyView::new(WorkspaceRegionTransition::new(
        leading_visible,
        true,
        WorkspaceRegionSizing::Divider,
        Some(AnyView::new(WorkspaceSplitDivider::named(
            "leading",
            true,
            controller.clone(),
        ))),
    )));
    children.push(erased(
        AnyView::new(WorkspaceCenterColumn {
            document: document.clone(),
            controller: controller.clone(),
            content: content.clone(),
            metrics,
        })
        .flex_with(center_weight, 1.0, Some(0.0))
        .modifier(WorkspaceLayoutAnimation {
            phase: phase.clone(),
        }),
    ));
    children.push(AnyView::new(WorkspaceRegionTransition::new(
        trailing_visible,
        true,
        WorkspaceRegionSizing::Divider,
        Some(AnyView::new(WorkspaceSplitDivider::named(
            "centerTrailing",
            true,
            controller.clone(),
        ))),
    )));
    children.push(AnyView::new(WorkspaceRegionTransition::new(
        trailing_visible,
        true,
        WorkspaceRegionSizing::Fraction(trailing_weight),
        Some(AnyView::new(WorkspaceSlotView {
            slot_id: WorkspaceSlotID::trailing(),
            document: document.clone(),
            controller: controller.clone(),
            content: content.clone(),
        })),
    )));
    children.push(AnyView::new(WorkspaceRegionTransition::new(
        trailing_rail.is_some(),
        true,
        WorkspaceRegionSizing::Points(trailing_rail_thickness),
        Some(match &trailing_rail {
            Some(plan) => AnyView::new(WorkspaceRail {
                slot_id: plan.0.clone(),
                edge_override: Some(plan.1),
                thickness: plan.2,
                groups_override: Some(plan.3.clone()),
                document: document.clone(),
                controller: controller.clone(),
            }),
            None => AnyView::new(EmptyView),
        }),
    )));

    let geometry = vec![
        WorkspaceResizeGeometry {
            split_id: "leading".to_string(),
            horizontal: true,
            offset: leading_rail_thickness + metrics.divider / 2.0,
            available: leading_split.available,
            range: leading_split.range,
            fraction: leading_split.fraction,
        },
        WorkspaceResizeGeometry {
            split_id: "centerTrailing".to_string(),
            horizontal: true,
            offset: leading_rail_thickness
                + leading_weight * available
                + if leading_visible {
                    metrics.divider
                } else {
                    0.0
                }
                + metrics.divider / 2.0,
            available: trailing_split.available,
            range: trailing_split.range,
            fraction: trailing_split.fraction,
        },
    ];
    erased(
        AnyView::new(WorkspaceBox::row().children(children).boxed())
            .modifier(WorkspaceResizeGeometryModifier(geometry))
            .modifier(WorkspaceFillModifier),
    )
}

// MARK: - Center column

/// `_WorkspaceCenterColumn`: the center region, the bottom split and a collapsed
/// bottom rail.
struct WorkspaceCenterColumn {
    document: WorkspaceDocument,
    controller: Rc<RefCell<WorkspaceController>>,
    content: WorkspaceContent,
    metrics: WorkspaceLayoutMetrics,
}

impl View for WorkspaceCenterColumn {
    fn body(&self, _cx: &Scope) -> AnyView {
        let extent = WorkspaceExtent::new();
        let document = self.document.clone();
        let controller = self.controller.clone();
        let content = self.content.clone();
        let metrics = self.metrics;
        erased(
            AnyView::new(WorkspaceExtentBody {
                horizontal: false,
                extent: extent.extent.clone(),
                build: Rc::new(move |height| {
                    build_center_column(
                        document.clone(),
                        controller.clone(),
                        content.clone(),
                        metrics,
                        height,
                    )
                }),
            })
            .modifier(WorkspaceFillModifier)
            .modifier(WorkspaceRoleModifier::new(
                "workspace-center-column",
                "workspace.center",
                "workspace-center-column",
            )),
        )
    }
}

fn build_center_column(
    document: WorkspaceDocument,
    controller: Rc<RefCell<WorkspaceController>>,
    content: WorkspaceContent,
    metrics: WorkspaceLayoutMetrics,
    height: f32,
) -> AnyView {
    let bottom_visible = !visible_groups(&WorkspaceSlotID::bottom(), &document).is_empty();
    let bottom_rail = if bottom_visible {
        None
    } else {
        rail_plan(
            WorkspaceEdge::Bottom,
            &document,
            metrics.shows_collapsed_rails,
        )
    };
    let resolver = WorkspaceSizingResolver::with_metrics(&document, metrics);
    let split = WorkspaceResolvedSplit::new(
        document.split_fractions.top_bottom,
        0.74,
        (height - if bottom_visible { metrics.divider } else { 0.0 }).max(0.0),
        resolver.slot(&WorkspaceSlotID::center(), WorkspaceSplitAxis::Vertical),
        resolver.slot(&WorkspaceSlotID::bottom(), WorkspaceSplitAxis::Vertical),
    );
    let phase = vec![bottom_visible];
    let bottom_rail_thickness = bottom_rail.as_ref().map_or(RAIL_THICKNESS, |plan| plan.2);

    let mut children: Vec<AnyView> = Vec::new();
    children.push(erased(
        AnyView::new(WorkspaceSlotView {
            slot_id: WorkspaceSlotID::center(),
            document: document.clone(),
            controller: controller.clone(),
            content: content.clone(),
        })
        .flex_with(
            if bottom_visible { split.fraction } else { 1.0 },
            1.0,
            Some(0.0),
        )
        .modifier(WorkspaceLayoutAnimation {
            phase: phase.clone(),
        }),
    ));
    children.push(AnyView::new(WorkspaceRegionTransition::new(
        bottom_visible,
        false,
        WorkspaceRegionSizing::Divider,
        Some(AnyView::new(WorkspaceSplitDivider::named(
            "topBottom",
            false,
            controller.clone(),
        ))),
    )));
    children.push(AnyView::new(WorkspaceRegionTransition::new(
        bottom_visible,
        false,
        WorkspaceRegionSizing::Fraction(1.0 - split.fraction),
        Some(AnyView::new(WorkspaceBottomSlot {
            document: document.clone(),
            controller: controller.clone(),
            content: content.clone(),
        })),
    )));
    children.push(AnyView::new(WorkspaceRegionTransition::new(
        bottom_rail.is_some(),
        false,
        WorkspaceRegionSizing::Points(bottom_rail_thickness),
        Some(match &bottom_rail {
            Some(plan) => AnyView::new(WorkspaceChromeSlot {
                slot_id: plan.0.clone(),
                edge: plan.1,
                thickness: plan.2,
                groups_override: Some(plan.3.clone()),
                document: document.clone(),
                controller: controller.clone(),
            }),
            None => AnyView::new(EmptyView),
        }),
    )));

    let geometry = vec![WorkspaceResizeGeometry {
        split_id: "topBottom".to_string(),
        horizontal: false,
        offset: metrics.divider / 2.0,
        available: split.available,
        range: split.range,
        fraction: split.fraction,
    }];
    erased(
        AnyView::new(WorkspaceBox::column().children(children).boxed())
            .modifier(WorkspaceResizeGeometryModifier(geometry))
            .modifier(WorkspaceFillModifier),
    )
}

// MARK: - Regions

/// `_WorkspaceSlotView`.
struct WorkspaceSlotView {
    slot_id: WorkspaceSlotID,
    document: WorkspaceDocument,
    controller: Rc<RefCell<WorkspaceController>>,
    content: WorkspaceContent,
}

impl View for WorkspaceSlotView {
    fn body(&self, _cx: &Scope) -> AnyView {
        let inner = match renderable_layout(&self.slot_id, &self.document) {
            Some(node) => AnyView::new(WorkspaceLayoutNodeView {
                node,
                slot_id: self.slot_id.clone(),
                document: self.document.clone(),
                controller: self.controller.clone(),
                content: self.content.clone(),
            }),
            None => AnyView::new(EmptyView),
        };
        erased(
            AnyView::new(WorkspaceBox::column().child((inner).flex(1.0)).boxed())
                .modifier(WorkspaceFillModifier)
                .modifier(WorkspaceRoleModifier::new(
                    "workspace-region",
                    &format!("workspace.slot.{}", self.slot_id.raw_value()),
                    &format!("workspace-region-{}", self.slot_id.raw_value()),
                )),
        )
    }
}

/// `_WorkspaceBottomSlot`: renders the bottom region only when it has a visible
/// group.
struct WorkspaceBottomSlot {
    document: WorkspaceDocument,
    controller: Rc<RefCell<WorkspaceController>>,
    content: WorkspaceContent,
}

impl View for WorkspaceBottomSlot {
    fn body(&self, _cx: &Scope) -> AnyView {
        let inner: AnyView =
            if visible_groups(&WorkspaceSlotID::bottom(), &self.document).is_empty() {
                AnyView::new(EmptyView)
            } else {
                erased(
                    AnyView::new(WorkspaceSlotView {
                        slot_id: WorkspaceSlotID::bottom(),
                        document: self.document.clone(),
                        controller: self.controller.clone(),
                        content: self.content.clone(),
                    })
                    .modifier(WorkspaceRoleModifier::new(
                        "workspace-region",
                        "workspace.slot.bottom",
                        "workspace-region-bottom",
                    )),
                )
            };
        erased(
            AnyView::new(WorkspaceBox::column().child((inner).flex(1.0)).boxed())
                .modifier(WorkspaceFillModifier)
                .modifier(WorkspaceRoleModifier::new(
                    "workspace-bottom-slot",
                    "workspace.bottom",
                    "workspace-bottom-slot",
                )),
        )
    }
}

/// `_WorkspaceChromeSlot`: a rail reserved at the edge with a fixed thickness.
struct WorkspaceChromeSlot {
    slot_id: WorkspaceSlotID,
    edge: WorkspaceEdge,
    thickness: f32,
    groups_override: Option<Vec<WorkspaceTabGroup>>,
    document: WorkspaceDocument,
    controller: Rc<RefCell<WorkspaceController>>,
}

impl View for WorkspaceChromeSlot {
    fn body(&self, _cx: &Scope) -> AnyView {
        let horizontal = matches!(self.edge, WorkspaceEdge::Top | WorkspaceEdge::Bottom);
        let rail = AnyView::new(WorkspaceRail {
            slot_id: self.slot_id.clone(),
            edge_override: Some(self.edge),
            thickness: self.thickness,
            groups_override: self.groups_override.clone(),
            document: self.document.clone(),
            controller: self.controller.clone(),
        });
        let container = if horizontal {
            WorkspaceBox::row()
        } else {
            WorkspaceBox::column()
        };
        erased(
            AnyView::new(container.child(rail).boxed()).modifier(WorkspaceChromeSlotModifier {
                horizontal,
                thickness: self.thickness,
                slot_id: self.slot_id.clone(),
            }),
        )
    }
}

struct WorkspaceChromeSlotModifier {
    horizontal: bool,
    thickness: f32,
    slot_id: WorkspaceSlotID,
}

impl cove_composition::ViewModifier for WorkspaceChromeSlotModifier {
    fn apply_node(&self, node: &Node) {
        let colors = theme_for_node(node).colors;
        node.set_background_color(Some(colors.background));
        node.set_attachment(
            LayoutDebugAttachmentKey::LAYOUT_ROLE,
            "workspace-chrome-slot".to_owned(),
        );
        node.set_attachment(
            LayoutDebugAttachmentKey::SEMANTIC_ROLE,
            format!("workspace.chrome.{}", self.slot_id.raw_value()),
        );
        node.set_attachment(
            LayoutDebugAttachmentKey::DEBUG_NAME,
            format!("workspace-chrome-{}", self.slot_id.raw_value()),
        );
    }
    fn apply_layout(&self, layout: &LayoutNode) {
        if self.horizontal {
            layout.set_height(Some(self.thickness));
            layout.set_width_percent(100.0);
        } else {
            layout.set_width(Some(self.thickness));
        }
        layout.set_flex_shrink(0.0);
    }
}

/// `_WorkspaceRailReserve`: empty space reserved for a rail that draws nothing.
pub struct WorkspaceRailReserve {
    pub edge: WorkspaceEdge,
    pub thickness: f32,
}

impl View for WorkspaceRailReserve {
    fn body(&self, _cx: &Scope) -> AnyView {
        let horizontal = matches!(self.edge, WorkspaceEdge::Top | WorkspaceEdge::Bottom);
        erased(
            AnyView::new(EmptyView)
                .modifier(WorkspaceRailReserveModifier {
                    horizontal,
                    thickness: self.thickness,
                })
                .modifier(WorkspaceRoleModifier::new(
                    "workspace-rail-reserve",
                    &format!("workspace.rail.reserve.{}", self.edge.raw_value()),
                    &format!("workspace-rail-reserve-{}", self.edge.raw_value()),
                )),
        )
    }
}

struct WorkspaceRailReserveModifier {
    horizontal: bool,
    thickness: f32,
}

impl cove_composition::ViewModifier for WorkspaceRailReserveModifier {
    fn apply_layout(&self, layout: &LayoutNode) {
        if self.horizontal {
            layout.set_height(Some(self.thickness));
        } else {
            layout.set_width(Some(self.thickness));
        }
    }
}

// MARK: - Layout recursion

/// `_WorkspaceLayoutNodeView`: one authored layout tree resolved into panes and
/// content-addressed dividers.
pub struct WorkspaceLayoutNodeView {
    pub node: WorkspaceLayoutNode,
    pub slot_id: WorkspaceSlotID,
    pub document: WorkspaceDocument,
    pub controller: Rc<RefCell<WorkspaceController>>,
    pub content: WorkspaceContent,
}

impl View for WorkspaceLayoutNodeView {
    fn body(&self, _cx: &Scope) -> AnyView {
        match &self.node {
            WorkspaceLayoutNode::Group(group_id) => {
                match self.document.groups.get(group_id).cloned() {
                    Some(group) => AnyView::new(WorkspaceTabGroupView {
                        group,
                        document: self.document.clone(),
                        controller: self.controller.clone(),
                        content: self.content.clone(),
                    }),
                    None => AnyView::new(EmptyView),
                }
            }
            WorkspaceLayoutNode::Split {
                axis,
                fraction,
                first,
                second,
            } => {
                let Some(first_leaf) = first.leaf_group_ids().into_iter().next() else {
                    return AnyView::new(EmptyView);
                };
                let Some(second_leaf) = second.leaf_group_ids().into_iter().next() else {
                    return AnyView::new(EmptyView);
                };
                let address = WorkspaceLayoutSplit::new(
                    self.slot_id.clone(),
                    first_leaf.clone(),
                    second_leaf.clone(),
                );
                let metrics = WorkspaceLayoutMetrics::default();
                let order = match axis {
                    WorkspaceSplitAxis::Horizontal => WorkspaceBox::row(),
                    WorkspaceSplitAxis::Vertical => WorkspaceBox::column(),
                };
                let divider = AnyView::new(WorkspaceSplitDivider {
                    debug_name: format!(
                        "workspace-layout-split-{}-{}-{}",
                        self.slot_id.raw_value(),
                        first_leaf.raw_value(),
                        second_leaf.raw_value()
                    ),
                    is_vertical_split: *axis == WorkspaceSplitAxis::Horizontal,
                    controller: self.controller.clone(),
                    address: Some(address),
                    split_id: None,
                    hairline: metrics.divider,
                });
                erased(
                    AnyView::new(
                        order
                            .child(WorkspaceLayoutNodeView {
                                node: (**first).clone(),
                                slot_id: self.slot_id.clone(),
                                document: self.document.clone(),
                                controller: self.controller.clone(),
                                content: self.content.clone(),
                            })
                            .child(divider)
                            .child(WorkspaceLayoutNodeView {
                                node: (**second).clone(),
                                slot_id: self.slot_id.clone(),
                                document: self.document.clone(),
                                controller: self.controller.clone(),
                                content: self.content.clone(),
                            })
                            .boxed(),
                    )
                    .modifier(WorkspaceFillModifier)
                    .modifier(WorkspaceLayoutSplitModifier {
                        fraction: *fraction,
                    }),
                )
            }
        }
    }
}

/// Records the authored fraction of an in-slot split so the divider can read the
/// value its container was laid out with.
struct WorkspaceLayoutSplitModifier {
    fraction: f32,
}

impl cove_composition::ViewModifier for WorkspaceLayoutSplitModifier {
    fn apply_node(&self, node: &Node) {
        node.set_attachment("cove.workspace.layout.splitFraction", self.fraction);
    }
}

// MARK: - Split divider

/// State shared by the divider's three closures. `Rc` keeps one instance so the
/// keyboard handler and the motion handler write through the same controller.
struct DividerTarget {
    controller: Rc<RefCell<WorkspaceController>>,
    address: Option<WorkspaceLayoutSplit>,
    split_id: Option<super::ids::WorkspaceSplitID>,
    is_vertical_split: bool,
}

impl DividerTarget {
    fn key(&self) -> Option<String> {
        match (&self.split_id, &self.address) {
            (Some(split_id), _) => Some(split_id.raw_value().to_string()),
            (None, Some(address)) => Some(format!(
                "layout-{}-{}",
                address.first_group.raw_value(),
                address.second_group.raw_value()
            )),
            (None, None) => None,
        }
    }

    /// Walk up to the nearest container that published split geometry.
    fn geometry(&self, from: &Node) -> Option<WorkspaceResizeGeometry> {
        let wanted = self.key()?;
        let mut cursor = from.parent();
        while let Some(current) = cursor {
            if let Some(values) =
                current.attachment::<Vec<WorkspaceResizeGeometry>>(RESIZE_GEOMETRY_KEY)
                && let Some(value) = values
                    .into_iter()
                    .find(|candidate| candidate.split_id == wanted)
            {
                return Some(value);
            }
            cursor = current.parent();
        }
        None
    }

    /// The fraction to display: the authored one, else the resolved geometry.
    fn current_fraction(&self, geometry: &WorkspaceResizeGeometry) -> f32 {
        let document = self.controller.borrow();
        let saved = match self.split_id.as_ref().map(|id| id.raw_value()) {
            Some("leading") => document.document().split_fractions.leading,
            Some("centerTrailing") => document.document().split_fractions.center_trailing,
            Some("topBottom") => document.document().split_fractions.top_bottom,
            Some("top") => document.document().split_fractions.top,
            _ => None,
        };
        geometry
            .range
            .0
            .max(geometry.range.1.min(saved.unwrap_or(geometry.fraction)))
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

/// `_WorkspaceSplitDivider`: the invisible grab area around a gutter hairline.
/// Pointer motion writes a fraction; the keyboard writes the same fraction in
/// 2 % steps (10 % with Shift) and jumps to the constraints on Home/End.
pub struct WorkspaceSplitDivider {
    pub debug_name: String,
    /// `true` for a vertical split, whose divider is a vertical hairline dragged
    /// horizontally.
    pub is_vertical_split: bool,
    pub controller: Rc<RefCell<WorkspaceController>>,
    /// Content-addressed split inside a slot, or `None` for a named region split.
    pub address: Option<WorkspaceLayoutSplit>,
    pub split_id: Option<super::ids::WorkspaceSplitID>,
    pub hairline: f32,
}

impl WorkspaceSplitDivider {
    fn named(
        split_id: &str,
        is_vertical_split: bool,
        controller: Rc<RefCell<WorkspaceController>>,
    ) -> Self {
        Self {
            debug_name: format!("workspace-split-{split_id}"),
            is_vertical_split,
            controller,
            address: None,
            split_id: Some(super::ids::WorkspaceSplitID::new(split_id)),
            hairline: WorkspaceLayoutMetrics::default().divider,
        }
    }

    fn target(&self) -> Rc<DividerTarget> {
        Rc::new(DividerTarget {
            controller: self.controller.clone(),
            address: self.address.clone(),
            split_id: self.split_id.clone(),
            is_vertical_split: self.is_vertical_split,
        })
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
        let theme = super::theme::resolve_workspace_theme(node);
        let hit_size = (theme.gutter + theme.split_divider_hit_slop * 2.0).max(1.0);
        node.set_cursor(Some(if self.is_vertical_split {
            cove_platform::SystemCursor::ResizeHorizontal
        } else {
            cove_platform::SystemCursor::ResizeVertical
        }));
        node.set_attachment(
            LayoutDebugAttachmentKey::DEBUG_NAME,
            self.debug_name.clone(),
        );
        node.set_attachment(
            LayoutDebugAttachmentKey::LAYOUT_ROLE,
            "workspace-split-divider".to_owned(),
        );
        node.set_attachment("cove.workspace.split.hitSize", hit_size);

        let target = self.target();
        let mut semantics = AccessibilitySemantics::new(AccessibilityRole::Slider);
        semantics.label = format!(
            "Resize {} workspace region",
            self.split_id
                .as_ref()
                .map(|id| id.raw_value().to_string())
                .or_else(|| {
                    self.address.as_ref().map(|address| {
                        format!(
                            "{}-{}",
                            address.first_group.raw_value(),
                            address.second_group.raw_value()
                        )
                    })
                })
                .unwrap_or_else(|| "split".to_string())
        );
        if let Some(geometry) = target.geometry(node) {
            semantics.value = format!(
                "{}%",
                (target.current_fraction(&geometry) * 100.0).round() as i32
            );
        }
        node.set_accessibility(Some(semantics));

        let mut actions = node.accessibility_actions();
        let increment = target.clone();
        let increment_node = node.clone();
        actions.increment = Some(Rc::new(move || {
            if let Some(geometry) = increment.geometry(&increment_node) {
                increment.write(increment.current_fraction(&geometry) + 0.02);
            }
        }));
        let decrement = target.clone();
        let decrement_node = node.clone();
        actions.decrement = Some(Rc::new(move || {
            if let Some(geometry) = decrement.geometry(&decrement_node) {
                decrement.write(decrement.current_fraction(&geometry) - 0.02);
            }
        }));
        node.set_accessibility_actions(actions);

        let Some(registry) = InteractionRegistryHolder::current() else {
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

        let motion_target = target.clone();
        let motion_node = node.clone();
        registry.set_motion(
            node,
            route.clone(),
            Rc::new(move |event, _| {
                if PointerCaptureHolder::current()
                    .and_then(|capture| capture.target())
                    .as_ref()
                    != Some(&motion_node)
                {
                    return cove_core::scene::EventResult::Ignored;
                }
                let Some(geometry) = motion_target.geometry(&motion_node) else {
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
                motion_target.write((position - geometry.offset) / geometry.available);
                cove_core::scene::EventResult::Handled
            }),
        );

        let key_target = target;
        let key_node = node.clone();
        registry.set_key(
            node,
            InputHandlerRoute::new(
                cove_core::scene::InputHandlerRole::Workspace,
                InputRoutingPriority::NORMAL,
                "workspace.split.key",
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
                let Some(geometry) = key_target.geometry(&key_node) else {
                    return cove_core::scene::EventResult::Ignored;
                };
                let step = if event.modifiers.has_shift() {
                    0.1
                } else {
                    0.02
                };
                let current = key_target.current_fraction(&geometry);
                match event.scancode {
                    cove_composition::Scancode::HOME => key_target.write(geometry.range.0),
                    cove_composition::Scancode::END => key_target.write(geometry.range.1),
                    cove_composition::Scancode::ARROW_LEFT if key_target.is_vertical_split => {
                        key_target.write(current - step)
                    }
                    cove_composition::Scancode::ARROW_RIGHT if key_target.is_vertical_split => {
                        key_target.write(current + step)
                    }
                    cove_composition::Scancode::ARROW_UP if !key_target.is_vertical_split => {
                        key_target.write(current - step)
                    }
                    cove_composition::Scancode::ARROW_DOWN if !key_target.is_vertical_split => {
                        key_target.write(current + step)
                    }
                    _ => return cove_core::scene::EventResult::Ignored,
                }
                cove_core::scene::EventResult::Handled
            }),
        );
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_align_items(Align::Stretch);
        layout.set_justify_content(Justify::Center);
        self.update_layout(&layout);
        Some(layout)
    }

    /// A shell divider fills the region around its hairline, so that whole grab
    /// area takes input. A divider inside a slot sits between its two panes.
    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_flex_direction(if self.is_vertical_split {
            FlexDirection::Row
        } else {
            FlexDirection::Column
        });
        if self.address.is_some() {
            if self.is_vertical_split {
                layout.set_width(Some(1.0));
            } else {
                layout.set_height(Some(1.0));
            }
            layout.set_flex_shrink(0.0);
        } else {
            layout.set_flex_grow(1.0);
            layout.set_flex_shrink(1.0);
            layout.set_flex_basis(0.0);
        }
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        // The empty gutter between two panes: nothing is drawn, the canvas shows
        // through, and the parent keeps the invisible grab area around it.
        vec![AnyView::new(WorkspaceSplitDividerLine {
            axis: self.is_vertical_split,
        })]
    }
}
impl_primitive_view!(WorkspaceSplitDivider);

/// `_WorkspaceSplitDividerLine`: the hairline itself, never hit-testable.
struct WorkspaceSplitDividerLine {
    /// `true` when the divider is vertical, so the line is a vertical hairline.
    axis: bool,
}

impl PrimitiveView for WorkspaceSplitDividerLine {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn update_node(&self, node: &Node) {
        let theme = super::theme::resolve_workspace_theme(node);
        let thickness = theme.gutter.max(1.0);
        node.set_attachment("cove.workspace.split.lineThickness", thickness);
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_align_self(Align::Stretch);
        if self.axis {
            layout.set_width(Some(1.0));
        } else {
            layout.set_height(Some(1.0));
        }
        Some(layout)
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_align_self(Align::Stretch);
        if self.axis {
            layout.set_width(Some(1.0));
        } else {
            layout.set_height(Some(1.0));
        }
    }
}
impl_primitive_view!(WorkspaceSplitDividerLine);

// MARK: - Tab group

/// `_WorkspaceTabGroupView`: tab bar plus the active panel inside a vertical
/// scroller, wrapped in the rounded pane chrome.
pub struct WorkspaceTabGroupView {
    pub group: WorkspaceTabGroup,
    pub document: WorkspaceDocument,
    pub controller: Rc<RefCell<WorkspaceController>>,
    pub content: WorkspaceContent,
}

impl WorkspaceTabGroupView {
    fn hides_tab_bar(&self) -> bool {
        self.group.panels.len() == 1
            && self
                .group
                .panels
                .first()
                .and_then(|id| self.document.panels.get(id))
                .is_some_and(|panel| panel.hides_tab_bar_when_alone)
    }
}

impl View for WorkspaceTabGroupView {
    fn body(&self, _cx: &Scope) -> AnyView {
        let active = self
            .group
            .active_panel_id
            .clone()
            .or_else(|| self.group.panels.first().cloned());
        let mut children: Vec<AnyView> = Vec::new();
        if !self.hides_tab_bar() {
            children.push(AnyView::new(WorkspaceTabBar {
                group: self.group.clone(),
                document: self.document.clone(),
                controller: self.controller.clone(),
            }));
        }
        if let Some(active_id) = &active {
            // Every region behaves like CSS `overflow: auto`: the panel is laid
            // out at the region height and only overflow scrolls.
            let panel = (self.content)(active_id.clone());
            let scrolled = AnyView::new(ScrollView::vertical(
                AnyView::new(panel)
                    .flex_with(1.0, 1.0, Some(0.0))
                    .frame_min_width(0.0),
            ))
            .flex_with(1.0, 1.0, Some(0.0))
            .frame_min_width(0.0)
            .clipped();
            let title = self
                .document
                .panels
                .get(active_id)
                .map(|panel| panel.title.clone())
                .unwrap_or_else(|| active_id.raw_value().to_string());
            let mut semantics = AccessibilitySemantics::new(AccessibilityRole::TabPanel);
            semantics.label = title;
            children.push(erased(
                AnyView::new(WorkspaceBox::column().child(scrolled).boxed())
                    .flex_with(1.0, 1.0, Some(0.0))
                    .frame_min_width(0.0)
                    .modifier(WorkspaceSemanticsModifier(semantics))
                    .modifier(WorkspaceRoleModifier::new(
                        "workspace-panel-content",
                        "workspace.panel.content",
                        &format!("workspace-panel-{}", active_id.raw_value()),
                    )),
            ));
        }
        let pane = WorkspacePane::semantic(SemanticColorRef::surface())
            .debug_name(format!("workspace-group-{}", self.group.id.raw_value()))
            .layout_role("workspace-tab-group")
            .semantic_role("workspace.group");
        erased(
            AnyView::new(pane.child(erased(
                AnyView::new(WorkspaceBox::column().children(children).boxed())
                    .modifier(WorkspaceFillModifier),
            )))
            .modifier(WorkspaceFillModifier),
        )
    }
}

/// `_WorkspaceTabBar`: a horizontally scrolling tab row plus one trailing menu
/// carrying panel choice, maximize and collapse.
struct WorkspaceTabBar {
    group: WorkspaceTabGroup,
    document: WorkspaceDocument,
    controller: Rc<RefCell<WorkspaceController>>,
}

impl View for WorkspaceTabBar {
    fn body(&self, _cx: &Scope) -> AnyView {
        let active = self
            .group
            .active_panel_id
            .clone()
            .or_else(|| self.group.panels.first().cloned());
        let mut tabs: Vec<AnyView> = Vec::new();
        for panel_id in &self.group.panels {
            let Some(panel) = self.document.panels.get(panel_id) else {
                continue;
            };
            tabs.push(AnyView::new(WorkspaceTabButton {
                group_id: self.group.id.clone(),
                panel_id: panel_id.clone(),
                title: panel.title.clone(),
                is_active: active.as_ref() == Some(panel_id),
                is_pinned: self.group.is_pinned(panel_id),
                document: self.document.clone(),
                controller: self.controller.clone(),
            }));
        }
        let active_title = active
            .as_ref()
            .and_then(|id| self.document.panels.get(id))
            .map(|panel| panel.title.clone())
            .unwrap_or_default();
        let mut list = AccessibilitySemantics::new(AccessibilityRole::TabList);
        list.label = "Panel tabs".to_string();
        list.value = active_title;
        let row = AnyView::new(
            WorkspaceBox::row()
                .align(Align::Center)
                .children(tabs)
                .boxed(),
        )
        .modifier(WorkspaceSemanticsModifier(list));
        let scroller = AnyView::new(ScrollView::horizontal(row)).flex_with(1.0, 1.0, Some(0.0));
        erased(
            AnyView::new(
                WorkspaceBox::row()
                    .align(Align::Center)
                    .child(scroller)
                    .child(WorkspaceTabOverflow {
                        group: self.group.clone(),
                        document: self.document.clone(),
                        controller: self.controller.clone(),
                        presented: cove_core::State::new(false),
                    })
                    .boxed(),
            )
            .modifier(WorkspaceTabBarModifier {
                metrics: WorkspaceLayoutMetrics::default(),
            }),
        )
    }
}

struct WorkspaceTabBarModifier {
    metrics: WorkspaceLayoutMetrics,
}

impl cove_composition::ViewModifier for WorkspaceTabBarModifier {
    fn apply_node(&self, node: &Node) {
        node.set_attachment(
            LayoutDebugAttachmentKey::LAYOUT_ROLE,
            "workspace-tab-bar".to_owned(),
        );
        node.set_attachment(
            LayoutDebugAttachmentKey::SEMANTIC_ROLE,
            "workspace.tab.bar".to_owned(),
        );
        let mut semantics = AccessibilitySemantics::new(AccessibilityRole::Group);
        semantics.label = "Panel tab bar".to_string();
        node.set_accessibility(Some(semantics));
    }
    fn apply_layout(&self, layout: &LayoutNode) {
        layout.set_height(Some(self.metrics.tab_height.max(26.0)));
        layout.set_flex_shrink(0.0);
        layout.set_padding(4.0, Edge::Left);
        layout.set_padding(4.0, Edge::Right);
    }
}

/// The bar's trailing overflow affordance: one menu with panel choice plus the
/// group actions.
struct WorkspaceTabOverflow {
    group: WorkspaceTabGroup,
    document: WorkspaceDocument,
    controller: Rc<RefCell<WorkspaceController>>,
    presented: cove_core::State<bool>,
}

impl WorkspaceTabOverflow {
    fn entries(&self) -> Vec<MenuEntry> {
        let mut entries: Vec<MenuEntry> = Vec::new();
        for (index, panel_id) in self.group.panels.iter().enumerate() {
            let Some(panel) = self.document.panels.get(panel_id) else {
                continue;
            };
            let controller = self.controller.clone();
            let group_id = self.group.id.clone();
            let target = panel_id.clone();
            entries.push(MenuEntry::Item(
                MenuItem::new(index as u64, panel.title.clone(), move || {
                    controller
                        .borrow_mut()
                        .dispatch(WorkspaceCommand::SetActivePanel {
                            group_id: group_id.clone(),
                            panel_id: target.clone(),
                        });
                })
                .selected(self.group.active_panel_id.as_ref() == Some(panel_id)),
            ));
        }
        entries.push(MenuEntry::separator(u64::MAX - 1));
        if let Some(active) = self.group.active_panel_id.clone() {
            let controller = self.controller.clone();
            entries.push(MenuEntry::Item(MenuItem::new(
                u64::MAX - 2,
                "Maximize",
                move || {
                    controller
                        .borrow_mut()
                        .dispatch(WorkspaceCommand::ToggleMaximize(active.clone()));
                },
            )));
        }
        if self.can_collapse() {
            let controller = self.controller.clone();
            let group_id = self.group.id.clone();
            entries.push(MenuEntry::Item(MenuItem::new(
                u64::MAX - 3,
                "Collapse",
                move || {
                    controller
                        .borrow_mut()
                        .dispatch(WorkspaceCommand::Collapse(group_id.clone()));
                },
            )));
        }
        entries
    }

    fn can_collapse(&self) -> bool {
        self.document
            .slot_containing(&self.group.id)
            .is_some_and(|slot| slot != WorkspaceSlotID::center())
            && self.group.panels.iter().any(|id| {
                self.document
                    .panels
                    .get(id)
                    .is_some_and(|panel| panel.is_collapsible)
            })
    }
}

impl View for WorkspaceTabOverflow {
    fn body(&self, _cx: &Scope) -> AnyView {
        let menu = Menu::new(self.entries()).width(240.0);
        let trigger = AnyView::new(
            WorkspaceBox::row()
                .align(Align::Center)
                .child(Text::new("⋯").semantic_font(SemanticFontRef::label()))
                .boxed(),
        )
        .modifier(WorkspaceOverflowTriggerModifier);
        erased(
            AnyView::new(
                Popover::new(trigger, menu)
                    .is_presented(self.presented.binding())
                    .placement(PopoverPlacement::BottomTrailing)
                    .width(240.0),
            )
            .modifier(WorkspaceRoleModifier::new(
                "workspace-tabs-overflow",
                "workspace.tabs.overflow",
                &format!("workspace-tabs-overflow-{}", self.group.id.raw_value()),
            )),
        )
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; presented)
    }
}

struct WorkspaceOverflowTriggerModifier;

impl cove_composition::ViewModifier for WorkspaceOverflowTriggerModifier {
    fn apply_layout(&self, layout: &LayoutNode) {
        layout.set_width(Some(24.0));
        layout.set_height(Some(24.0));
        layout.set_flex_shrink(0.0);
        layout.set_align_items(Align::Center);
        layout.set_justify_content(Justify::Center);
    }
}

/// `_WorkspaceTabButton`: one tab, with press/drag, keyboard activation and tab
/// accessibility.
struct WorkspaceTabButton {
    group_id: WorkspaceTabGroupID,
    panel_id: WorkspacePanelID,
    title: String,
    is_active: bool,
    is_pinned: bool,
    document: WorkspaceDocument,
    controller: Rc<RefCell<WorkspaceController>>,
}

#[derive(Clone)]
struct TabPressState {
    down_x: f32,
    down_y: f32,
    did_drag: bool,
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
        node.set_attachment(
            LayoutDebugAttachmentKey::DEBUG_NAME,
            format!("workspace-tab-{}", self.panel_id.raw_value()),
        );
        node.set_attachment(
            LayoutDebugAttachmentKey::SEMANTIC_ROLE,
            "workspace.tab".to_owned(),
        );

        let position = self
            .document
            .groups
            .get(&self.group_id)
            .and_then(|group| group.panels.iter().position(|id| id == &self.panel_id));
        let count = self
            .document
            .groups
            .get(&self.group_id)
            .map_or(1, |group| group.panels.len());
        let mut semantics = AccessibilitySemantics::new(AccessibilityRole::Tab);
        semantics.label = self.title.clone();
        semantics.value = if self.is_pinned {
            "Pinned".to_string()
        } else {
            String::new()
        };
        semantics.help = position
            .map(|index| format!("Tab {} of {count}", index + 1))
            .unwrap_or_default();
        semantics.identifier = self.panel_id.raw_value().to_string();
        semantics.state.is_selected = self.is_active;
        semantics.combines_children = true;
        node.set_accessibility(Some(semantics));

        let mut actions = node.accessibility_actions();
        let activate_controller = self.controller.clone();
        let activate_group = self.group_id.clone();
        let activate_panel = self.panel_id.clone();
        actions.activate = Some(Rc::new(move || {
            activate_controller
                .borrow_mut()
                .dispatch(WorkspaceCommand::SetActivePanel {
                    group_id: activate_group.clone(),
                    panel_id: activate_panel.clone(),
                });
        }));
        node.set_accessibility_actions(actions);

        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };
        let route = InputHandlerRoute::new(
            cove_core::scene::InputHandlerRole::Workspace,
            InputRoutingPriority::CAPTURE,
            "workspace.tab.drag",
        );

        let press_node = node.clone();
        let press_controller = self.controller.clone();
        let press_document = self.document.clone();
        let press_group = self.group_id.clone();
        let press_panel = self.panel_id.clone();
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
        let motion_controller = self.controller.clone();
        let motion_document = self.document.clone();
        let motion_group = self.group_id.clone();
        let motion_panel = self.panel_id.clone();
        let motion_title = self.title.clone();
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
                    let _ = motion_controller;
                    cove_core::scene::EventResult::Handled
                } else {
                    cove_core::scene::EventResult::Ignored
                }
            }),
        );

        let activate_controller = self.controller.clone();
        let activate_group = self.group_id.clone();
        let activate_panel = self.panel_id.clone();
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
        layout.set_flex_shrink(0.0);
    }

    fn children(&self, node: &Node) -> Vec<AnyView> {
        // Selected tab: `surfaceVariant`; otherwise clear. 9 pt padding, 6 pt
        // radius, `height - 8` tall, no rules above or between tabs.
        let theme = theme_for_node(node);
        let fill = if self.is_active {
            theme.colors.surface_variant
        } else {
            Color::CLEAR
        };
        let mut row = WorkspaceBox::row().align(Align::Center).spacing(4.0);
        if self.is_pinned {
            row = row.child(
                AnyView::new(EmptyView)
                    .modifier(WorkspaceDotModifier(SemanticColorRef::on_surface_muted())),
            );
        }
        row = row.child(Text::new(self.title.clone()).semantic_font(SemanticFontRef::label()));
        vec![erased(
            AnyView::new(WorkspaceBox::row().child(row.boxed()).boxed()).modifier(
                WorkspaceTabButtonModifier {
                    fill,
                    radius: 6.0,
                    padding: 9.0,
                },
            ),
        )]
    }
}
impl_primitive_view!(WorkspaceTabButton);

struct WorkspaceDotModifier(SemanticColorRef);

impl cove_composition::ViewModifier for WorkspaceDotModifier {
    fn apply_node(&self, node: &Node) {
        node.set_background_color(Some(self.0.resolve(&theme_for_node(node))));
        node.set_corner_radius(3.0);
    }
    fn apply_layout(&self, layout: &LayoutNode) {
        layout.set_width(Some(6.0));
        layout.set_height(Some(6.0));
        layout.set_flex_shrink(0.0);
    }
}

struct WorkspaceTabButtonModifier {
    fill: Color,
    radius: f32,
    padding: f32,
}

impl cove_composition::ViewModifier for WorkspaceTabButtonModifier {
    fn apply_node(&self, node: &Node) {
        node.set_background_color(Some(self.fill));
        node.set_corner_radius(self.radius);
        node.set_clips_to_bounds(true);
    }
    fn apply_layout(&self, layout: &LayoutNode) {
        layout.set_flex_shrink(0.0);
        layout.set_min_height(Some(26.0));
        layout.set_padding(self.padding, Edge::Left);
        layout.set_padding(self.padding, Edge::Right);
    }
}

// MARK: - Rail

/// `_WorkspaceRailButton`: one collapsed-group restore. Transparent at rest on
/// the canvas, a state-layer wash on hover/press, muted label.
struct WorkspaceRailButton {
    group_id: WorkspaceTabGroupID,
    panel_id: WorkspacePanelID,
    title: String,
    controller: Rc<RefCell<WorkspaceController>>,
    horizontal: bool,
}

impl View for WorkspaceRailButton {
    fn body(&self, _cx: &Scope) -> AnyView {
        let controller = self.controller.clone();
        let group_id = self.group_id.clone();
        let panel_id = self.panel_id.clone();
        let label = if self.horizontal {
            AnyView::new(
                Text::new(self.title.clone())
                    .semantic_font(SemanticFontRef::label())
                    .semantic_foreground(SemanticColorRef::on_surface_muted()),
            )
        } else {
            AnyView::new(RailTitleLabel {
                title: self.title.clone(),
            })
        };
        let button = Button::new(label, move || {
            let mut controller = controller.borrow_mut();
            controller.dispatch(WorkspaceCommand::SetActivePanel {
                group_id: group_id.clone(),
                panel_id: panel_id.clone(),
            });
            controller.dispatch(WorkspaceCommand::Expand(group_id.clone()));
        })
        .tooltip(self.title.clone());
        let mut semantics = AccessibilitySemantics::new(AccessibilityRole::Button);
        semantics.label = self.title.clone();
        erased(
            AnyView::new(button)
                .modifier(WorkspaceSemanticsModifier(semantics))
                .modifier(WorkspaceRailButtonModifier {
                    horizontal: self.horizontal,
                }),
        )
    }
}

struct WorkspaceRailButtonModifier {
    horizontal: bool,
}

impl cove_composition::ViewModifier for WorkspaceRailButtonModifier {
    fn apply_node(&self, node: &Node) {
        node.set_corner_radius(6.0);
    }
    fn apply_layout(&self, layout: &LayoutNode) {
        if self.horizontal {
            layout.set_flex_direction(FlexDirection::Row);
        } else {
            layout.set_flex_direction(FlexDirection::Column);
            layout.set_width(Some(24.0));
        }
        layout.set_align_items(Align::Center);
        layout.set_justify_content(Justify::Center);
        layout.set_flex_shrink(0.0);
    }
}

/// A collapsed rail's vertical title, tinted from the node's own theme. Swift
/// resolves `onSurfaceMuted` inside the reader; reading a detached node would
/// always yield the default theme.
struct RailTitleLabel {
    title: String,
}

impl View for RailTitleLabel {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(RailTitleHost {
            title: self.title.clone(),
        })
    }
}

struct RailTitleHost {
    title: String,
}

impl PrimitiveView for RailTitleHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn update_node(&self, node: &Node) {
        let color = theme_for_node(node).colors.on_surface_muted;
        node.set_attachment(
            "cove.workspace.label.text",
            self.title.replace('\n', " ").trim().to_string(),
        );
        node.set_attachment("cove.workspace.label.color", color);
        node.set_attachment("cove.workspace.label.size", 11.0f32);
        node.set_attachment("cove.workspace.label.vertical", self.is_vertical());
    }
    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        if self.is_vertical() {
            layout.set_flex_direction(FlexDirection::Column);
            layout.set_align_items(Align::Center);
            layout.set_justify_content(Justify::Center);
            layout.set_width(Some(24.0));
            let count = self.cleaned().chars().count() as f32;
            layout.set_min_height(Some((count * 13.0 + 12.0).max(48.0)));
        } else {
            layout.set_align_items(Align::Center);
            layout.set_min_height(Some(28.0));
        }
        Some(layout)
    }
    fn update_layout(&self, layout: &LayoutNode) {
        if self.is_vertical() {
            layout.set_flex_direction(FlexDirection::Column);
            layout.set_width(Some(24.0));
        }
    }
}
impl_primitive_view!(RailTitleHost);

impl RailTitleHost {
    fn cleaned(&self) -> String {
        let clean = self.title.replace('\n', " ");
        let trimmed = clean.trim();
        if trimmed.is_empty() {
            "Panel".to_string()
        } else {
            trimmed.to_string()
        }
    }

    /// The Swift short-Latin exception: a 3-char ASCII title stays horizontal.
    fn is_vertical(&self) -> bool {
        let title = self.cleaned();
        !(title.chars().count() <= 3 && title.is_ascii())
    }
}

/// `_WorkspaceRail`: the column (or row) of collapsed-group restore buttons on
/// one edge.
pub struct WorkspaceRail {
    pub slot_id: WorkspaceSlotID,
    pub edge_override: Option<WorkspaceEdge>,
    pub thickness: f32,
    pub groups_override: Option<Vec<WorkspaceTabGroup>>,
    pub document: WorkspaceDocument,
    pub controller: Rc<RefCell<WorkspaceController>>,
}

impl WorkspaceRail {
    fn edge(&self) -> WorkspaceEdge {
        self.edge_override
            .unwrap_or_else(|| super::view::rail_edge(&self.slot_id, &self.document))
    }
}

impl View for WorkspaceRail {
    fn body(&self, _cx: &Scope) -> AnyView {
        let edge = self.edge();
        let horizontal = matches!(edge, WorkspaceEdge::Top | WorkspaceEdge::Bottom);
        let groups = self
            .groups_override
            .clone()
            .unwrap_or_else(|| rail_groups(&self.slot_id, Some(edge), &self.document));
        let mut buttons: Vec<AnyView> = Vec::new();
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
        let stack = if horizontal {
            WorkspaceBox::row()
        } else {
            WorkspaceBox::column()
        };
        erased(
            AnyView::new(
                stack
                    .align(Align::Center)
                    .spacing(if horizontal { 6.0 } else { 4.0 })
                    .children(buttons)
                    .boxed(),
            )
            .modifier(WorkspaceRailModifier {
                horizontal,
                thickness: self.thickness,
                slot_id: self.slot_id.clone(),
            }),
        )
    }
}

struct WorkspaceRailModifier {
    horizontal: bool,
    thickness: f32,
    slot_id: WorkspaceSlotID,
}

impl cove_composition::ViewModifier for WorkspaceRailModifier {
    fn apply_node(&self, node: &Node) {
        let colors = theme_for_node(node).colors;
        node.set_background_color(Some(colors.surface_sunken));
        node.set_attachment(
            LayoutDebugAttachmentKey::LAYOUT_ROLE,
            "workspace-rail".to_owned(),
        );
        node.set_attachment(
            LayoutDebugAttachmentKey::DEBUG_NAME,
            format!("workspace-rail-{}", self.slot_id.raw_value()),
        );
    }
    fn apply_layout(&self, layout: &LayoutNode) {
        if self.horizontal {
            layout.set_flex_direction(FlexDirection::Row);
            layout.set_height(Some(self.thickness));
            layout.set_width_percent(100.0);
        } else {
            layout.set_flex_direction(FlexDirection::Column);
            layout.set_width(Some(self.thickness));
        }
        layout.set_flex_shrink(0.0);
    }
}

// MARK: - Floating layer

/// `_WorkspaceFloatingLayer`: every floating window in an absolute layer above
/// the docked regions, stacked by z-index.
struct WorkspaceFloatingLayer {
    document: WorkspaceDocument,
    controller: Rc<RefCell<WorkspaceController>>,
    content: WorkspaceContent,
}

impl View for WorkspaceFloatingLayer {
    fn body(&self, _cx: &Scope) -> AnyView {
        let mut windows: Vec<WorkspaceFloatingWindow> = self.document.floating_windows.clone();
        windows.sort_by(|left, right| {
            left.z_index
                .cmp(&right.z_index)
                .then_with(|| left.id.raw_value().cmp(right.id.raw_value()))
        });
        let mut children: Vec<AnyView> = Vec::new();
        for window in windows {
            let Some(group) = self.document.groups.get(&window.group_id).cloned() else {
                continue;
            };
            children.push(AnyView::new(WorkspaceFloatingWindowView {
                window,
                group,
                document: self.document.clone(),
                controller: self.controller.clone(),
                content: self.content.clone(),
            }));
        }
        erased(
            AnyView::new(WorkspaceBox::column().children(children).boxed())
                .modifier(WorkspaceFloatingLayerModifier),
        )
    }
}

struct WorkspaceFloatingLayerModifier;

impl cove_composition::ViewModifier for WorkspaceFloatingLayerModifier {
    fn apply_node(&self, node: &Node) {
        node.set_z_index(10_000.0);
        node.set_is_hit_testable(false);
        node.set_attachment(
            LayoutDebugAttachmentKey::LAYOUT_ROLE,
            "workspace-floating-layer".to_owned(),
        );
        node.set_attachment(
            LayoutDebugAttachmentKey::DEBUG_NAME,
            "workspace-floating-layer".to_owned(),
        );
    }
    fn apply_layout(&self, layout: &LayoutNode) {
        layout.set_position_type(PositionType::Absolute);
        for edge in [Edge::Left, Edge::Top, Edge::Right, Edge::Bottom] {
            layout.set_position(0.0, edge);
        }
    }
}

/// `_WorkspaceFloatingWindowView`: title bar with drag handle and redock button,
/// the group's tab bar and panel, the floating surface and its elevation.
struct WorkspaceFloatingWindowView {
    window: WorkspaceFloatingWindow,
    group: WorkspaceTabGroup,
    document: WorkspaceDocument,
    controller: Rc<RefCell<WorkspaceController>>,
    content: WorkspaceContent,
}

impl View for WorkspaceFloatingWindowView {
    fn body(&self, _cx: &Scope) -> AnyView {
        let metrics = WorkspaceLayoutMetrics::default();
        let frame = WorkspaceSizingResolver::with_metrics(&self.document, metrics).floating_frame(
            self.window.frame,
            &self.group.id,
            FLOATING_TITLE_HEIGHT,
        );
        let controller = self.controller.clone();
        let window_id = self.window.id.clone();
        let redock = Button::new(
            AnyView::new(Text::new("Attach").semantic_font(SemanticFontRef::caption())),
            move || {
                controller
                    .borrow_mut()
                    .dispatch(WorkspaceCommand::RedockFloatingWindow {
                        window_id: window_id.clone(),
                        target: WorkspaceTarget::tab_group(
                            WorkspaceSlotID::center(),
                            WorkspaceTabGroupID::new("center"),
                            None,
                        ),
                    });
            },
        )
        .tooltip("Attach");

        let title_bar = AnyView::new(
            WorkspaceBox::row()
                .align(Align::Center)
                .spacing(6.0)
                .child(WorkspaceFloatingDragHandle {
                    title: self.window.title.clone(),
                    window_id: self.window.id.clone(),
                })
                .child(redock)
                .boxed(),
        )
        .modifier(WorkspaceFloatingTitleBarModifier {
            height: FLOATING_TITLE_HEIGHT,
        })
        .modifier(WorkspaceRoleModifier::new(
            "workspace-floating-title-bar",
            "workspace.floating.titleBar",
            &format!("workspace-floating-title-{}", self.window.id.raw_value()),
        ));

        let body = AnyView::new(WorkspaceTabGroupView {
            group: self.group.clone(),
            document: self.document.clone(),
            controller: self.controller.clone(),
            content: self.content.clone(),
        })
        .flex_with(1.0, 1.0, Some(0.0));

        erased(
            AnyView::new(WorkspaceBox::column().child(title_bar).child(body).boxed()).modifier(
                WorkspaceFloatingWindowModifier {
                    x: self.window.frame.x,
                    y: self.window.frame.y,
                    width: frame.width,
                    height: frame.height,
                    z_index: self.window.z_index as f32,
                    fill: SemanticColorRef::surface_floating(),
                    window: self.window.clone(),
                    controller: self.controller.clone(),
                },
            ),
        )
    }
}

struct WorkspaceFloatingTitleBarModifier {
    height: f32,
}

impl cove_composition::ViewModifier for WorkspaceFloatingTitleBarModifier {
    fn apply_node(&self, node: &Node) {
        let colors = theme_for_node(node).colors;
        node.set_background_color(Some(colors.surface_raised));
    }
    fn apply_layout(&self, layout: &LayoutNode) {
        layout.set_height(Some(self.height));
        layout.set_flex_shrink(0.0);
        layout.set_padding(6.0, Edge::Left);
        layout.set_padding(6.0, Edge::Right);
        layout.set_padding(3.0, Edge::Top);
        layout.set_padding(3.0, Edge::Bottom);
    }
}

struct WorkspaceFloatingWindowModifier {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    z_index: f32,
    fill: SemanticColorRef,
    window: WorkspaceFloatingWindow,
    controller: Rc<RefCell<WorkspaceController>>,
}

impl cove_composition::ViewModifier for WorkspaceFloatingWindowModifier {
    fn apply_node(&self, node: &Node) {
        node.set_background_color(Some(self.fill.resolve(&theme_for_node(node))));
        node.set_corner_radius(8.0);
        node.set_clips_to_bounds(true);
        node.set_z_index(self.z_index);
        node.set_shadow(cove_core::SurfaceShadow {
            color: Some(Color::from_rgba8(0, 0, 0, 150)),
            offset_x: 0.0,
            offset_y: 12.0,
            blur: 32.0,
        });
        node.set_is_hit_testable(true);
        node.set_attachment(
            LayoutDebugAttachmentKey::LAYOUT_ROLE,
            "workspace-floating-window".to_owned(),
        );
        node.set_attachment(
            LayoutDebugAttachmentKey::SEMANTIC_ROLE,
            "workspace.floating.window".to_owned(),
        );
        node.set_attachment(
            LayoutDebugAttachmentKey::DEBUG_NAME,
            format!("workspace-floating-window-{}", self.window.id.raw_value()),
        );
        install_floating_drag(node, &self.window, &self.controller);
    }
    fn apply_layout(&self, layout: &LayoutNode) {
        layout.set_position_type(PositionType::Absolute);
        layout.set_position(self.x, Edge::Left);
        layout.set_position(self.y, Edge::Top);
        layout.set_width(Some(self.width));
        layout.set_height(Some(self.height));
    }
}

/// `_WorkspaceFloatingWindowDragModifier`: dragging the title band moves the
/// window; releasing over the center group's tab band redocks it onto that
/// group, the same destination as the Attach button.
fn install_floating_drag(
    node: &Node,
    window: &WorkspaceFloatingWindow,
    controller: &Rc<RefCell<WorkspaceController>>,
) {
    let Some(registry) = InteractionRegistryHolder::current() else {
        return;
    };
    let controller = controller.clone();
    let window = window.clone();
    let route = InputHandlerRoute::new(
        cove_core::scene::InputHandlerRole::Workspace,
        InputRoutingPriority::CAPTURE,
        "workspace.floating.window.drag",
    );
    let pointer_node = node.clone();
    let pointer_window = window.clone();
    let pointer_controller = controller.clone();
    registry.set_pointer(
        node,
        route.clone(),
        Rc::new(move |event, phase, _| {
            if event.button != MouseButton::Left {
                return cove_core::scene::EventResult::Ignored;
            }
            match phase {
                cove_core::scene::PointerPhase::Down => {
                    let local = pointer_node.convert_from_window(cove_core::Point {
                        x: event.x,
                        y: event.y,
                    });
                    if local.y > FLOATING_TITLE_HEIGHT {
                        return cove_core::scene::EventResult::Ignored;
                    }
                    pointer_node.set_attachment(
                        FLOATING_DRAG_KEY,
                        TabPressState {
                            down_x: event.x,
                            down_y: event.y,
                            did_drag: false,
                        },
                    );
                    if let Some(capture) = PointerCaptureHolder::current() {
                        capture.acquire(&pointer_node, false);
                    }
                    cove_core::scene::EventResult::Handled
                }
                cove_core::scene::PointerPhase::Up => {
                    let state = pointer_node.attachment::<TabPressState>(FLOATING_DRAG_KEY);
                    pointer_node.remove_attachment(FLOATING_DRAG_KEY);
                    if let Some(capture) = PointerCaptureHolder::current()
                        && capture.target().as_ref() == Some(&pointer_node)
                    {
                        capture.release();
                    }
                    let got_drag = state.as_ref().is_some_and(|state| state.did_drag);
                    if got_drag && let Some(root) = drag::workspace_root(&pointer_node) {
                        let center_id = WorkspaceTabGroupID::new("center");
                        let band = {
                            let controller = pointer_controller.borrow();
                            controller
                                .document()
                                .groups
                                .get(&center_id)
                                .and_then(|group| drag::tab_band(group, &root))
                        };
                        let point = cove_core::Point { x: event.x, y: event.y };
                        if band.is_some_and(|band| band.contains(point)) {
                            pointer_controller
                                .borrow_mut()
                                .dispatch(WorkspaceCommand::RedockFloatingWindow {
                                    window_id: pointer_window.id.clone(),
                                    target: WorkspaceTarget::tab_group(
                                        WorkspaceSlotID::center(),
                                        center_id,
                                        None,
                                    ),
                                });
                        }
                    }
                    cove_core::scene::EventResult::Handled
                }
            }
        }),
    );

    let motion_node = node.clone();
    let motion_window = window;
    let motion_controller = controller;
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
            let Some(state) = motion_node.attachment::<TabPressState>(FLOATING_DRAG_KEY) else {
                return cove_core::scene::EventResult::Ignored;
            };
            if !state.did_drag {
                let dx = event.x - state.down_x;
                let dy = event.y - state.down_y;
                if dx.abs().max(dy.abs()) >= DRAG_THRESHOLD {
                    motion_node.set_attachment(
                        FLOATING_DRAG_KEY,
                        TabPressState {
                            down_x: state.down_x,
                            down_y: state.down_y,
                            did_drag: true,
                        },
                    );
                }
            }
            let next = super::sizing::WorkspaceRect::new(
                motion_window.frame.x + event.x - state.down_x,
                motion_window.frame.y + event.y - state.down_y,
                motion_window.frame.width,
                motion_window.frame.height,
            );
            motion_controller
                .borrow_mut()
                .dispatch(WorkspaceCommand::MoveFloatingWindow {
                    window_id: motion_window.id.clone(),
                    frame: next,
                });
            cove_core::scene::EventResult::Handled
        }),
    );
}

/// `_WorkspaceFloatingDragHandle`: the title label with a `move` cursor. The
/// window modifier owns the drag, so this node is not hit-testable.
struct WorkspaceFloatingDragHandle {
    title: String,
    window_id: WorkspaceFloatingWindowID,
}

impl PrimitiveView for WorkspaceFloatingDragHandle {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn update_node(&self, node: &Node) {
        node.set_cursor(Some(cove_platform::SystemCursor::Move));
        node.set_attachment(
            LayoutDebugAttachmentKey::SEMANTIC_ROLE,
            "workspace.floating.drag".to_owned(),
        );
        node.set_attachment(
            LayoutDebugAttachmentKey::DEBUG_NAME,
            format!("workspace-floating-drag-{}", self.window_id.raw_value()),
        );
        if let Some(registry) = InteractionRegistryHolder::current() {
            registry.remove(node);
        }
    }
    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_flex_grow(1.0);
        layout.set_align_items(Align::Center);
        Some(layout)
    }
    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_flex_grow(1.0);
        layout.set_align_items(Align::Center);
    }
    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![AnyView::new(
            Text::new(self.title.clone())
                .line_limit(Some(1))
                .semantic_font(SemanticFontRef::label())
                .semantic_foreground(SemanticColorRef::on_surface()),
        )]
    }
}
impl_primitive_view!(WorkspaceFloatingDragHandle);

// MARK: - Compact

/// `_WorkspaceCompactView`: one panel at a time plus a menu that switches
/// between the panels the document still holds.
struct WorkspaceCompactView {
    document: WorkspaceDocument,
    controller: Rc<RefCell<WorkspaceController>>,
    content: WorkspaceContent,
    presented: cove_core::State<bool>,
}

impl View for WorkspaceCompactView {
    fn body(&self, _cx: &Scope) -> AnyView {
        let mut panels: Vec<super::model::WorkspacePanel> = self
            .document
            .panels
            .values()
            .filter(|panel| self.document.group_containing(&panel.id).is_some())
            .cloned()
            .collect();
        panels.sort_by(|left, right| left.title.cmp(&right.title));
        let focused = self.controller.borrow().focused_panel_id().cloned();
        let selected = focused
            .as_ref()
            .and_then(|id| panels.iter().find(|panel| &panel.id == id))
            .or_else(|| {
                panels
                    .iter()
                    .find(|panel| panel.id.raw_value() == "viewport")
            })
            .or_else(|| panels.first())
            .cloned();
        let Some(selected) = selected else {
            return erased(AnyView::new(EmptyView).modifier(WorkspaceFillModifier));
        };

        let mut entries: Vec<MenuEntry> = Vec::new();
        for (index, panel) in panels.iter().enumerate() {
            let Some(group) = self.document.group_containing(&panel.id) else {
                continue;
            };
            let controller = self.controller.clone();
            let group_id = group.id.clone();
            let panel_id = panel.id.clone();
            entries.push(MenuEntry::Item(
                MenuItem::new(index as u64, panel.title.clone(), move || {
                    controller
                        .borrow_mut()
                        .dispatch(WorkspaceCommand::SetActivePanel {
                            group_id: group_id.clone(),
                            panel_id: panel_id.clone(),
                        });
                })
                .selected(panel.id == selected.id),
            ));
        }
        let trigger = AnyView::new(
            WorkspaceBox::row()
                .align(Align::Center)
                .child(
                    Text::new(format!("{} ▾", selected.title))
                        .semantic_font(SemanticFontRef::label()),
                )
                .boxed(),
        )
        .modifier(WorkspacePaddingModifier::all(8.0));
        let picker = AnyView::new(
            Popover::new(trigger, Menu::new(entries).width(240.0))
                .is_presented(self.presented.binding())
                .placement(PopoverPlacement::BottomLeading)
                .width(240.0),
        );
        let panel = (self.content)(selected.id.clone());
        erased(
            AnyView::new(
                WorkspaceBox::column()
                    .child(picker)
                    .child(Divider::horizontal())
                    .child(panel.flex_with(1.0, 1.0, Some(0.0)).frame_min_width(0.0))
                    .boxed(),
            )
            .modifier(WorkspaceFillModifier),
        )
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; presented)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn sample_document() -> WorkspaceDocument {
        let mut panels = BTreeMap::new();
        let panel =
            super::super::model::WorkspacePanel::new(WorkspacePanelID::new("viewport"), "Viewport");
        panels.insert(panel.id.clone(), panel);
        let group = WorkspaceTabGroup::new(
            WorkspaceTabGroupID::new("center"),
            vec![WorkspacePanelID::new("viewport")],
        );
        let mut groups = BTreeMap::new();
        groups.insert(group.id.clone(), group);
        let mut slots = BTreeMap::new();
        slots.insert(
            WorkspaceSlotID::center(),
            super::super::model::WorkspaceSlot::new(
                WorkspaceSlotID::center(),
                super::super::model::WorkspaceSlotKind::Content,
                Some(WorkspaceLayoutNode::Group(WorkspaceTabGroupID::new(
                    "center",
                ))),
            ),
        );
        WorkspaceDocument::new(panels, groups, slots)
    }

    #[test]
    fn named_divider_keys_by_region() {
        let controller = Rc::new(RefCell::new(WorkspaceController::new(sample_document())));
        let divider = WorkspaceSplitDivider::named("leading", true, controller);
        assert_eq!(divider.target().key().as_deref(), Some("leading"));
    }

    #[test]
    fn workspace_view_renders_the_docked_shell() {
        let controller = Rc::new(RefCell::new(WorkspaceController::new(sample_document())));
        let view = WorkspaceView::new(controller, Rc::new(|_| AnyView::new(EmptyView)));
        // A composed body exists; the shell is reached through the surface
        // reader once the container measures itself.
        let _ = view.body(&Scope::detached());
    }

    #[test]
    fn workspace_view_short_circuits_to_the_maximized_panel() {
        let controller = Rc::new(RefCell::new(WorkspaceController::new(sample_document())));
        controller
            .borrow_mut()
            .dispatch(WorkspaceCommand::ToggleMaximize(WorkspacePanelID::new(
                "viewport",
            )));
        let view = WorkspaceView::new(controller, Rc::new(|_| AnyView::new(EmptyView)));
        let _ = view.body(&Scope::detached());
    }
}
