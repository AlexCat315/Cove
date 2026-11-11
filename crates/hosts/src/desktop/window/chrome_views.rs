//! Immersive window chrome views.
//!
//! Translation of the view half of
//! `Hosts/Desktop/Sources/CoveUI/Window/WindowChrome.swift`. The Swift file mixed
//! the immersive title bar with the native hit-test collector; `chrome.rs` owns
//! the host half (the per-window ambient, attachment keys and the incremental
//! collector) and this module owns the composed views:
//!
//! - [`ImmersiveWindowTitleBar`] — the title bar row itself;
//! - [`WindowDragRegion`] — the region the platform may use to drag the window;
//! - [`WindowControlStrip`] / [`WindowControlButton`] — custom minimize,
//!   maximize/restore and close;
//! - [`WindowChromeInteractiveRegion`] — marks a region that must stay
//!   interactive.
//!
//! `NativeMenuInstaller` / `NativeMenuActionTarget` have no Rust counterpart by
//! design: `window::native_menu` owns the installer (an action table plus the
//! AppKit presenter) and the bridge routes chosen ids back to authored closures,
//! so a separate target object would duplicate the action table.
use cove_composition::Scope;
use cove_composition::{
    AnyView, LayoutDebugAttachmentKey, PrimitiveView, View, ViewExt, impl_primitive_view,
};
use cove_core::scene::{
    Align, FlexDirection, HoverPhase, InputHandlerRoute, InputRoutingPriority,
    InteractionRegistryHolder, Justify, LayoutNode, Node,
};
use cove_core::{Color, DrawList, Point, Rect};
use cove_platform::MouseButton;
use cove_shell::WindowChromeHitTest;
use std::rc::Rc;

use super::chrome::{
    AppWindowChromeContextHolder, CONFIGURATION_KEY, DRAG_REGION_KEY, INTERACTIVE_REGION_KEY,
    ImmersiveWindowControlStyle, platform_resize_border_width,
};
use crate::desktop::application::composition::AppDisplayHandleHolder;
use crate::desktop::display::AppDisplayHandle;

/// Default title bar height. The bar is `height` tall including its padding.
pub const DEFAULT_TITLE_BAR_HEIGHT: f32 = 34.0;
/// Default synthetic resize border on platforms without a native one.
pub const DEFAULT_RESIZE_BORDER_WIDTH: f32 = 6.0;
/// macOS keeps the native traffic lights, so the leading controls are reserved.
pub const DEFAULT_NATIVE_CONTROLS_LEADING_INSET: f32 = 76.0;
/// Windows 11 caption buttons are 46 pt wide and fill the title bar height.
pub const CONTROL_STRIP_WIDTH: f32 = 138.0;
pub const CONTROL_BUTTON_WIDTH: f32 = 46.0;

/// The immersive title bar.
///
/// The leading content is supplied by the host; the drag region absorbs the
/// remaining width so the whole bar row moves the window. On macOS the native
/// traffic lights occupy the leading inset and no custom strip is drawn.
pub struct ImmersiveWindowTitleBar {
    pub height: f32,
    pub resize_border_width: f32,
    pub control_style: ImmersiveWindowControlStyle,
    pub native_controls_leading_inset: f32,
    pub leading: AnyView,
}

impl ImmersiveWindowTitleBar {
    pub fn new(leading: impl View + 'static) -> Self {
        Self {
            height: DEFAULT_TITLE_BAR_HEIGHT,
            resize_border_width: DEFAULT_RESIZE_BORDER_WIDTH,
            control_style: ImmersiveWindowControlStyle::Automatic,
            native_controls_leading_inset: DEFAULT_NATIVE_CONTROLS_LEADING_INSET,
            leading: AnyView::new(leading),
        }
    }

    pub fn height(mut self, height: f32) -> Self {
        self.height = height.max(0.0);
        self
    }

    pub fn resize_border_width(mut self, width: f32) -> Self {
        self.resize_border_width = width.max(0.0);
        self
    }

    pub fn control_style(mut self, style: ImmersiveWindowControlStyle) -> Self {
        self.control_style = style;
        self
    }

    pub fn native_controls_leading_inset(mut self, inset: f32) -> Self {
        self.native_controls_leading_inset = inset.max(0.0);
        self
    }
}

impl View for ImmersiveWindowTitleBar {
    fn body(&self, _cx: &Scope) -> AnyView {
        let style = self.control_style.resolve();
        let mut children: Vec<AnyView> = Vec::new();
        if style == ImmersiveWindowControlStyle::Native {
            children.push(AnyView::new(WindowChromeFixedSpace {
                width: self.native_controls_leading_inset,
            }));
        }
        children.push(AnyView::new(WindowChromeInteractiveRegion::new(
            self.leading.clone(),
        )));
        children.push(AnyView::new(
            AnyView::new(WindowDragRegion::default()).modifier(WindowDragRegionHeight(self.height)),
        ));
        if style == ImmersiveWindowControlStyle::Custom {
            children.push(AnyView::new(
                AnyView::new(WindowControlStrip::new()).modifier(WindowControlStripFrame),
            ));
        }
        children.push(AnyView::new(
            AnyView::new(TitleBarHairline).modifier(TitleBarHairlineModifier),
        ));
        AnyView::new(
            WindowChromeRow { children }.modifier(WindowTitleBarModifier {
                height: self.height,
                resize_border_width: platform_resize_border_width(self.resize_border_width),
            }),
        )
    }
}

/// `WindowChromeFixedSpace`: a non-hit-testable width reservation for the
/// platform's own window controls.
pub struct WindowChromeFixedSpace {
    pub width: f32,
}

impl PrimitiveView for WindowChromeFixedSpace {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn update_node(&self, _node: &Node) {}
    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_width(Some(self.width));
        layout.set_height(Some(1.0));
        layout.set_flex_shrink(0.0);
        Some(layout)
    }
    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_width(Some(self.width));
        layout.set_height(Some(1.0));
        layout.set_flex_shrink(0.0);
    }
}
impl_primitive_view!(WindowChromeFixedSpace);

/// The title bar row: leading, drag region, optional custom controls.
struct WindowChromeRow {
    children: Vec<AnyView>,
}

impl PrimitiveView for WindowChromeRow {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn update_node(&self, _node: &Node) {}
    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_flex_direction(FlexDirection::Row);
        layout.set_align_items(Align::Center);
        layout.set_justify_content(Justify::FlexStart);
        layout.set_min_width(Some(0.0));
        Some(layout)
    }
    fn children(&self, _node: &Node) -> Vec<AnyView> {
        self.children.clone()
    }
}
impl_primitive_view!(WindowChromeRow);

/// Installs the authored hit-test declaration, then the bar's own frame and
/// roles. The installer node is a zero-size child so a title bar can carry more
/// than one.
struct WindowTitleBarModifier {
    height: f32,
    resize_border_width: f32,
}

impl cove_composition::ViewModifier for WindowTitleBarModifier {
    fn apply_node(&self, node: &Node) {
        node.set_attachment(
            CONFIGURATION_KEY,
            WindowChromeHitTest {
                title_bar_height: self.height,
                resize_border_width: self.resize_border_width,
                uses_explicit_drag_rects: true,
                ..WindowChromeHitTest::default()
            },
        );
        node.set_z_index(20_000.0);
        node.set_attachment(
            LayoutDebugAttachmentKey::LAYOUT_ROLE,
            "app-window-title-bar".to_owned(),
        );
        node.set_attachment(
            LayoutDebugAttachmentKey::DEBUG_NAME,
            "app-window-title-bar".to_owned(),
        );
        let colors = cove_ui_compose::theme::theme_for_node(node).colors;
        let environment =
            node.composition_value(&cove_ui_compose::adaptive::SurfaceEnvironment::key());
        node.set_background_color(title_bar_background(
            colors.surface,
            environment.translucent_window,
            environment.title_bar_divider,
        ));
    }
    fn apply_layout(&self, layout: &LayoutNode) {
        layout.set_flex_direction(FlexDirection::Row);
        layout.set_align_items(Align::Center);
        layout.set_height(Some(self.height));
        layout.set_flex_shrink(0.0);
        layout.set_padding(6.0, cove_core::scene::Edge::Left);
        layout.set_padding(6.0, cove_core::scene::Edge::Right);
        layout.set_padding(3.0, cove_core::scene::Edge::Top);
        layout.set_padding(3.0, cove_core::scene::Edge::Bottom);
    }
}

/// The bar's fill for a given window material and divider preference.
///
/// Seamless + translucent leaves the bar clear so the material runs through it.
/// A separated bar on a translucent window uses a *tinted* surface rather than
/// an opaque one, so choosing the classic look never throws the material away.
fn title_bar_background(
    surface: cove_core::Color,
    translucent: bool,
    separated: bool,
) -> Option<cove_core::Color> {
    if separated {
        Some(if translucent {
            surface.multiplied_alpha(0.72)
        } else {
            surface
        })
    } else if !translucent {
        Some(surface)
    } else {
        None
    }
}

/// The hairline along the bottom of the title bar row.
///
/// It stays in the tree in both modes so switching the preference only changes
/// a color, never rebuilds the chrome or its hit-test regions. Absolute
/// positioning keeps it out of the row's flex layout.
struct TitleBarHairline;

impl PrimitiveView for TitleBarHairline {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn update_node(&self, _node: &Node) {}
    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }
}
impl_primitive_view!(TitleBarHairline);

struct TitleBarHairlineModifier;

impl cove_composition::ViewModifier for TitleBarHairlineModifier {
    fn apply_node(&self, node: &Node) {
        let separated =
            node.composition_value(&cove_ui_compose::adaptive::SurfaceEnvironment::key())
                .title_bar_divider;
        let colors = cove_ui_compose::theme::theme_for_node(node).colors;
        // `divider` is too faint for a 1px line on a light material; `border`
        // matches the hairline Windows draws under a classic menu bar.
        node.set_background_color(separated.then_some(colors.border));
    }
    fn apply_layout(&self, layout: &LayoutNode) {
        layout.set_position_type(cove_core::scene::PositionType::Absolute);
        layout.set_position(0.0, cove_core::scene::Edge::Left);
        layout.set_position(0.0, cove_core::scene::Edge::Right);
        layout.set_position(0.0, cove_core::scene::Edge::Bottom);
        layout.set_height(Some(1.0));
    }
}

struct WindowDragRegionHeight(f32);

impl cove_composition::ViewModifier for WindowDragRegionHeight {
    fn apply_layout(&self, layout: &LayoutNode) {
        layout.set_height(Some(self.0));
    }
}

struct WindowControlStripFrame;

impl cove_composition::ViewModifier for WindowControlStripFrame {
    fn apply_layout(&self, layout: &LayoutNode) {
        layout.set_width(Some(CONTROL_STRIP_WIDTH));
        layout.set_height(Some(DEFAULT_TITLE_BAR_HEIGHT));
        layout.set_flex_shrink(0.0);
    }
}

/// `WindowDragRegion`: the empty band the platform treats as a drag handle. A
/// double click toggles maximize on every platform, matching the Swift pointer
/// handler.
#[derive(Clone, Copy, Debug, Default)]
pub struct WindowDragRegion {
    pub min_length: f32,
}

impl WindowDragRegion {
    pub fn new(min_length: f32) -> Self {
        Self {
            min_length: min_length.max(0.0),
        }
    }
}

impl PrimitiveView for WindowDragRegion {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(true);
        node
    }

    fn update_node(&self, node: &Node) {
        node.set_attachment(DRAG_REGION_KEY, true);
        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };
        let route = InputHandlerRoute::new(
            cove_core::scene::InputHandlerRole::Control,
            InputRoutingPriority::CAPTURE,
            "window.chrome.dragRegion",
        );
        registry.set_pointer(
            node,
            route,
            Rc::new(move |event, phase, _| {
                if phase != cove_core::scene::PointerPhase::Down
                    || event.button != MouseButton::Left
                    || event.clicks < 2
                {
                    return cove_core::scene::EventResult::Ignored;
                }
                with_display_handle(|handle| match AppWindowChromeContextHolder::current() {
                    Some(context) => handle.toggle_maximize_window_for(context.window_id),
                    None => handle.toggle_maximize_window(),
                });
                cove_core::scene::EventResult::Handled
            }),
        );
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_flex_grow(1.0);
        if self.min_length > 0.0 {
            layout.set_flex_basis(self.min_length);
        }
    }
}
impl_primitive_view!(WindowDragRegion);

/// `WindowChromeInteractiveRegionModifier`: marks a subtree that must stay
/// interactive rather than becoming a drag handle.
pub struct WindowChromeInteractiveRegion {
    pub content: AnyView,
}

impl WindowChromeInteractiveRegion {
    pub fn new(content: impl View + 'static) -> Self {
        Self {
            content: AnyView::new(content),
        }
    }
}

impl View for WindowChromeInteractiveRegion {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(
            self.content
                .clone()
                .modifier(WindowChromeInteractiveRegionModifier),
        )
    }
}

struct WindowChromeInteractiveRegionModifier;

impl cove_composition::ViewModifier for WindowChromeInteractiveRegionModifier {
    fn apply_node(&self, node: &Node) {
        node.set_attachment(INTERACTIVE_REGION_KEY, true);
    }
}

/// `WindowControlStrip`: minimize, maximize/restore and close. The maximize glyph
/// is derived from the real window state each render, so a click also forces a
/// display request.
pub struct WindowControlStrip;

impl WindowControlStrip {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WindowControlStrip {
    fn default() -> Self {
        Self::new()
    }
}

impl View for WindowControlStrip {
    fn body(&self, _cx: &Scope) -> AnyView {
        let window_id = AppWindowChromeContextHolder::current().map(|context| context.window_id);
        let is_maximized = is_window_maximized(window_id);

        let minimize = WindowControlButton::new(WindowControlGlyph::Minimize, move || {
            with_display_handle(|handle| match window_id {
                Some(id) => handle.minimize_window_for(id),
                None => handle.minimize_window(),
            });
        });
        let maximize = WindowControlButton::new(
            if is_maximized {
                WindowControlGlyph::Restore
            } else {
                WindowControlGlyph::Maximize
            },
            move || {
                with_display_handle(|handle| match window_id {
                    Some(id) => handle.toggle_maximize_window_for(id),
                    None => handle.toggle_maximize_window(),
                });
                with_display_handle(|handle| handle.request_display());
            },
        );
        let close = WindowControlButton::new(WindowControlGlyph::Close, move || {
            with_display_handle(|handle| match window_id {
                Some(id) => handle.close_window(id),
                None => handle.close_main_window(),
            });
        });

        AnyView::new(WindowControlRow {
            children: vec![
                AnyView::new(minimize),
                AnyView::new(maximize),
                AnyView::new(close),
            ],
        })
    }
}

/// Windows-style caption glyphs, drawn as one-point geometric strokes rather
/// than as UI icons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowControlGlyph {
    Minimize,
    Maximize,
    Restore,
    Close,
}

impl WindowControlGlyph {
    fn label(self) -> &'static str {
        match self {
            Self::Minimize => "Minimize",
            Self::Maximize => "Maximize",
            Self::Restore => "Restore",
            Self::Close => "Close",
        }
    }

    fn debug_name(self) -> &'static str {
        match self {
            Self::Minimize => "window-control-minimize",
            Self::Maximize => "window-control-maximize",
            Self::Restore => "window-control-restore",
            Self::Close => "window-control-close",
        }
    }

    fn destructive(self) -> bool {
        self == Self::Close
    }
}

/// A 10pt glyph centered in the caption button, matching the Windows 11
/// minimize line, maximize square, overlapping restore squares and close X.
fn draw_caption_glyph(
    list: &mut DrawList,
    origin: Point,
    width: f32,
    height: f32,
    glyph: WindowControlGlyph,
    color: Color,
) {
    let cx = origin.x + width * 0.5;
    let cy = origin.y + height * 0.5;
    let half = 5.0;
    let stroke = 1.0;
    let square = |x: f32, y: f32, size: f32| {
        Rect::new(x, y, size, size)
    };
    match glyph {
        WindowControlGlyph::Minimize => {
            let _ = list.add_line(
                Point::new(cx - half, cy),
                Point::new(cx + half, cy),
                stroke,
                color,
            );
        }
        WindowControlGlyph::Maximize => {
            let rect = square(cx - half, cy - half, half * 2.0);
            let _ = list.add_rounded_rect_stroke(rect, 0.0, stroke, color);
        }
        WindowControlGlyph::Restore => {
            let back = square(cx - half + 2.5, cy - half - 2.5, 7.5);
            let front = square(cx - half, cy - half + 2.5, 7.5);
            let _ = list.add_rounded_rect_stroke(back, 0.0, stroke, color);
            let _ = list.add_rounded_rect_stroke(front, 0.0, stroke, color);
        }
        WindowControlGlyph::Close => {
            let _ = list.add_line(
                Point::new(cx - half, cy - half),
                Point::new(cx + half, cy + half),
                stroke,
                color,
            );
            let _ = list.add_line(
                Point::new(cx + half, cy - half),
                Point::new(cx - half, cy + half),
                stroke,
                color,
            );
        }
    }
}

/// Hover repaints the caption background and glyph together; the close button
/// gets Windows' red plate with a white X.
fn apply_caption_appearance(node: &Node, glyph: WindowControlGlyph, hovered: bool) {
    let colors = cove_ui_compose::theme::theme_for_node(node).colors;
    node.set_background_color(if hovered {
        Some(if glyph.destructive() {
            colors.error
        } else {
            colors.state_layer_hover
        })
    } else {
        None
    });
    let stroke = if hovered && glyph.destructive() {
        colors.on_accent
    } else {
        colors.on_surface
    };
    let weak = node.downgrade();
    node.set_draw(Some(Rc::new(move |list, origin| {
        let Some(node) = Node::upgrade(&weak) else {
            return;
        };
        let frame = node.frame();
        if frame.width <= 0.0 || frame.height <= 0.0 {
            return;
        }
        draw_caption_glyph(list, origin, frame.width, frame.height, glyph, stroke);
    })));
}

/// `WindowControlButton`: one Windows-sized caption button.
pub struct WindowControlButton {
    glyph: WindowControlGlyph,
    action: Rc<dyn Fn()>,
}

impl WindowControlButton {
    pub fn new(glyph: WindowControlGlyph, action: impl Fn() + 'static) -> Self {
        Self {
            glyph,
            action: Rc::new(action),
        }
    }
}

impl PrimitiveView for WindowControlButton {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(true);
        node.set_is_focusable(true);
        node
    }

    fn update_node(&self, node: &Node) {
        node.set_attachment(INTERACTIVE_REGION_KEY, true);
        node.set_attachment(
            LayoutDebugAttachmentKey::DEBUG_NAME,
            self.glyph.debug_name().to_owned(),
        );
        let mut semantics = cove_core::scene::AccessibilitySemantics::new(
            cove_core::scene::AccessibilityRole::Button,
        );
        semantics.label = self.glyph.label().to_owned();
        node.set_accessibility(Some(semantics));
        let mut actions = node.accessibility_actions();
        let action = self.action.clone();
        actions.activate = Some(Rc::new(move || action()));
        node.set_accessibility_actions(actions);
        apply_caption_appearance(node, self.glyph, false);

        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };
        let pointer_action = self.action.clone();
        registry.set_pointer(
            node,
            InputHandlerRoute::new(
                cove_core::scene::InputHandlerRole::Control,
                InputRoutingPriority::CAPTURE,
                "window.chrome.control",
            ),
            Rc::new(move |event, phase, _| {
                if phase != cove_core::scene::PointerPhase::Up || event.button != MouseButton::Left
                {
                    return cove_core::scene::EventResult::Ignored;
                }
                pointer_action();
                cove_core::scene::EventResult::Handled
            }),
        );
        let weak = node.downgrade();
        let glyph = self.glyph;
        registry.set_hover(node, Rc::new(move |phase| {
            if let Some(node) = Node::upgrade(&weak) {
                apply_caption_appearance(&node, glyph, phase == HoverPhase::Enter);
            }
        }));
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_width(Some(CONTROL_BUTTON_WIDTH));
        layout.set_height(Some(DEFAULT_TITLE_BAR_HEIGHT));
        layout.set_flex_shrink(0.0);
        Some(layout)
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_width(Some(CONTROL_BUTTON_WIDTH));
        layout.set_height(Some(DEFAULT_TITLE_BAR_HEIGHT));
        layout.set_flex_shrink(0.0);
    }
}
impl_primitive_view!(WindowControlButton);

/// Two-point-spaced row of window control buttons.
struct WindowControlRow {
    children: Vec<AnyView>,
}

impl PrimitiveView for WindowControlRow {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn update_node(&self, _node: &Node) {}
    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_flex_direction(FlexDirection::Row);
        layout.set_align_items(Align::Center);
        layout.set_justify_content(Justify::FlexEnd);
        layout.set_gap(2.0, cove_core::scene::Gutter::All);
        Some(layout)
    }
    fn children(&self, _node: &Node) -> Vec<AnyView> {
        self.children.clone()
    }
}
impl_primitive_view!(WindowControlRow);

fn is_window_maximized(window_id: Option<cove_platform::WindowId>) -> bool {
    let Some(handle) = AppDisplayHandleHolder::current() else {
        return false;
    };
    match window_id {
        Some(id) => handle.is_window_maximized_for(id),
        None => handle.is_window_maximized(),
    }
}

/// Run `body` with the installed display handle, mirroring Swift's
/// `withAppDisplayHandle`: no handle means the request goes nowhere.
fn with_display_handle(body: impl FnOnce(&AppDisplayHandle)) {
    if let Some(handle) = AppDisplayHandleHolder::current() {
        body(&handle);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seamless_bar_is_clear_on_a_translucent_window() {
        assert_eq!(
            title_bar_background(cove_core::Color::WHITE, true, false),
            None
        );
    }

    #[test]
    fn separated_bar_keeps_the_material_visible() {
        let surface = cove_core::Color::WHITE;
        let fill =
            title_bar_background(surface, true, true).expect("a separated bar has a fill");
        assert_ne!(fill, surface, "the band must stay translucent");
    }

    #[test]
    fn opaque_window_uses_the_full_surface_in_both_modes() {
        let surface = cove_core::Color::WHITE;
        assert_eq!(title_bar_background(surface, false, false), Some(surface));
        assert_eq!(title_bar_background(surface, false, true), Some(surface));
    }

    #[test]
    fn automatic_control_style_matches_the_platform() {
        let resolved = ImmersiveWindowControlStyle::Automatic.resolve();
        #[cfg(target_os = "macos")]
        assert_eq!(resolved, ImmersiveWindowControlStyle::Native);
        #[cfg(not(target_os = "macos"))]
        assert_eq!(resolved, ImmersiveWindowControlStyle::Custom);
    }

    #[test]
    fn macos_needs_no_synthetic_resize_border() {
        #[cfg(target_os = "macos")]
        assert_eq!(
            platform_resize_border_width(DEFAULT_RESIZE_BORDER_WIDTH),
            0.0
        );
        #[cfg(not(target_os = "macos"))]
        assert_eq!(
            platform_resize_border_width(DEFAULT_RESIZE_BORDER_WIDTH),
            DEFAULT_RESIZE_BORDER_WIDTH
        );
    }

    #[test]
    fn drag_region_never_reserves_a_width() {
        let region = WindowDragRegion::default();
        let layout = region.make_layout_node().unwrap();
        assert!(layout.width().is_none());
        assert_eq!(region.min_length, 0.0);
    }
}
