//! Translation of Primitives/Controls/Rating/Rating.swift and RatingHost.swift.
//!
//! A controlled star rating with pointer preview, whole or fractional steps,
//! custom symbols and a single Tab stop. Hover and press state live with the
//! mounted node; the value is always the caller's.
use std::cell::RefCell;
use std::rc::Rc;

use crate::primitives::media::ui_icon_catalog;
use crate::primitives::media::vector_image_raster::VectorImageRaster;
use crate::theme::{ControlSize, ControlSizeEnvironment, SemanticColorRef, theme_for_node};
use crate::{AnyView, PrimitiveView, View};
use cove_core::scene::{
    AccessibilityActions, AccessibilityRole, AccessibilitySemantics, EventResult, FocusChainHolder,
    InputHandlerRoute, InteractionRegistryHolder, LayoutNode, Node, NodeResource, PointerCapture,
    PointerCaptureHolder, PointerPhase,
};
use cove_core::{Color, Point, Rect};
use cove_platform::{MouseButton, SystemCursor};

use super::rating_geometry::RatingGeometry;

/// How finely one symbol divides: whole, half, quarter, tenth, or any 1–20 equal
/// parts. Values snap to exact fractions, so `0.3` stays `0.3`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RatingPrecision {
    pub divisions: usize,
}

impl Default for RatingPrecision {
    fn default() -> Self {
        Self::WHOLE
    }
}

impl RatingPrecision {
    pub const WHOLE: Self = Self { divisions: 1 };
    pub const HALF: Self = Self { divisions: 2 };
    pub const QUARTER: Self = Self { divisions: 4 };
    pub const TENTH: Self = Self { divisions: 10 };

    pub fn divisions(count: usize) -> Self {
        Self {
            divisions: count.clamp(1, 20),
        }
    }

    pub fn increment(&self) -> f64 {
        1.0 / self.divisions as f64
    }

    pub fn snapped(&self, value: f64) -> f64 {
        (value * self.divisions as f64).round() / self.divisions as f64
    }

    /// The value a pointer selects within one symbol: the fraction rounds up, so
    /// touching a symbol's leading edge always selects at least one part.
    pub fn part(&self, fraction: f32) -> f64 {
        let divisions = self.divisions.clamp(1, 20);
        let parts = ((f64::from(fraction) * divisions as f64 - 1e-4).ceil() as i64)
            .clamp(1, divisions as i64);
        parts as f64 / divisions as f64
    }
}

/// The glyph pair a rating paints: an empty outline under a filled glyph clipped
/// to the value.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RatingSymbol {
    pub filled: &'static str,
    pub empty: &'static str,
}

impl RatingSymbol {
    pub fn star() -> Self {
        Self {
            filled: "star-fill",
            empty: "star",
        }
    }

    pub fn heart() -> Self {
        Self {
            filled: "heart-fill",
            empty: "heart",
        }
    }
}

impl Default for RatingSymbol {
    fn default() -> Self {
        Self::star()
    }
}

/// Authored selection rules.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RatingSelection {
    pub maximum: usize,
    pub precision: RatingPrecision,
    pub allows_clear: bool,
    pub is_enabled: bool,
    pub is_read_only: bool,
}

impl Default for RatingSelection {
    fn default() -> Self {
        Self {
            maximum: 5,
            precision: RatingPrecision::default(),
            allows_clear: true,
            is_enabled: true,
            is_read_only: false,
        }
    }
}

impl RatingSelection {
    pub fn validate(&mut self) {
        self.maximum = self.maximum.clamp(1, 20);
    }

    pub fn is_interactive(&self) -> bool {
        self.is_enabled && !self.is_read_only
    }

    pub fn clamped(&self, value: f64) -> f64 {
        if value.is_finite() {
            value.clamp(0.0, self.maximum as f64)
        } else {
            0.0
        }
    }

    pub fn snapped(&self, value: f64) -> f64 {
        let floor = if self.allows_clear {
            0.0
        } else {
            self.precision.increment()
        };
        self.clamped(self.precision.snapped(self.clamped(value)))
            .max(floor)
    }
}

/// Authored appearance.
#[derive(Clone)]
pub struct RatingAppearance {
    pub size: Option<ControlSize>,
    pub star_size: Option<f32>,
    pub symbol: RatingSymbol,
    pub spacing: f32,
    pub filled: SemanticColorRef,
    pub empty: SemanticColorRef,
}

impl Default for RatingAppearance {
    fn default() -> Self {
        Self {
            size: None,
            star_size: None,
            symbol: RatingSymbol::star(),
            spacing: 4.0,
            filled: SemanticColorRef::warning(),
            empty: SemanticColorRef::on_surface_muted(),
        }
    }
}

impl RatingAppearance {
    pub fn validate(&mut self) {
        if let Some(star_size) = self.star_size {
            self.star_size = if star_size.is_finite() {
                Some(star_size.clamp(10.0, 64.0))
            } else {
                None
            };
        }
        self.spacing = if self.spacing.is_finite() {
            self.spacing.clamp(0.0, 24.0)
        } else {
            4.0
        };
    }
}

/// Painted colours for one rating.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RatingPalette {
    pub filled: Color,
    pub empty: Color,
    pub focus: Color,
}

impl RatingPalette {
    pub fn resolve(
        node: &Node,
        appearance: &RatingAppearance,
        selection: &RatingSelection,
    ) -> Self {
        let theme = theme_for_node(node);
        let alpha = if selection.is_enabled { 1.0 } else { 0.4 };
        Self {
            filled: appearance.filled.resolve(&theme).multiplied_alpha(alpha),
            empty: appearance.empty.resolve(&theme).multiplied_alpha(alpha),
            focus: theme.colors.focus_ring,
        }
    }
}

/// Hover and press state that survives recomposition.
#[derive(Default)]
pub struct RatingSession {
    pub value: RefCell<f64>,
    pub on_change: RefCell<Option<Rc<dyn Fn(f64)>>>,
    pub preview: RefCell<Option<f64>>,
    pub pressed_candidate: RefCell<Option<f64>>,
    pub has_moved: RefCell<bool>,
    pub configuration: RefCell<Option<RatingConfiguration>>,
    capture: RefCell<Option<Rc<PointerCapture>>>,
    focus: RefCell<Option<Rc<cove_core::scene::FocusChain>>>,
}

/// The value, rules and geometry the session last saw.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RatingConfiguration {
    pub value: f64,
    pub selection: RatingSelection,
    pub geometry: RatingGeometry,
}

pub const RATING_SESSION: &str = "rating.session";

impl RatingSession {
    pub fn new() -> Rc<Self> {
        Rc::new(Self::default())
    }

    pub fn hover(&self, candidate: Option<f64>, node: &Node) {
        if self.pressed_candidate.borrow().is_some() && *self.preview.borrow() != candidate {
            *self.has_moved.borrow_mut() = true;
        }
        if *self.preview.borrow() == candidate {
            return;
        }
        *self.preview.borrow_mut() = candidate;
        node.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
            "ratingInteraction",
        ));
    }

    pub fn begin(&self, candidate: f64, node: &Node) {
        *self.preview.borrow_mut() = Some(candidate);
        *self.pressed_candidate.borrow_mut() = Some(candidate);
        *self.has_moved.borrow_mut() = false;
        if let Some(capture) = self.capture.borrow().as_ref() {
            capture.acquire(node, true);
        }
        node.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
            "ratingInteraction",
        ));
    }

    pub fn cancel(&self, node: &Node) {
        *self.preview.borrow_mut() = None;
        *self.pressed_candidate.borrow_mut() = None;
        *self.has_moved.borrow_mut() = false;
        if let Some(capture) = self.capture.borrow().as_ref()
            && capture.target().is_some_and(|target| target == *node)
        {
            capture.release();
        }
        node.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
            "ratingInteraction",
        ));
    }
}

impl NodeResource for RatingSession {
    fn mount(&self, _node: &Node) {
        *self.capture.borrow_mut() = PointerCaptureHolder::current();
        *self.focus.borrow_mut() = FocusChainHolder::current();
    }

    fn unmount(&self, node: &Node) {
        self.cancel(node);
        if let Some(focus) = self.focus.borrow().as_ref()
            && focus.focused().is_some_and(|focused| focused == *node)
        {
            focus.clear();
        }
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn std::any::Any> {
        self
    }
}

/// A controlled star rating.
pub struct Rating {
    pub label: String,
    pub value: f64,
    pub on_change: Rc<dyn Fn(f64)>,
    pub selection: RatingSelection,
    pub appearance: RatingAppearance,
    pub session: Rc<RatingSession>,
}

impl Rating {
    pub fn new(label: impl Into<String>, value: f64, on_change: impl Fn(f64) + 'static) -> Self {
        let mut selection = RatingSelection::default();
        selection.validate();
        let mut appearance = RatingAppearance::default();
        appearance.validate();
        Self {
            label: label.into(),
            value,
            on_change: Rc::new(on_change),
            selection,
            appearance,
            session: RatingSession::new(),
        }
    }

    pub fn selection(mut self, selection: RatingSelection) -> Self {
        let mut selection = selection;
        selection.validate();
        self.selection = selection;
        self
    }

    pub fn appearance(mut self, appearance: RatingAppearance) -> Self {
        let mut appearance = appearance;
        appearance.validate();
        self.appearance = appearance;
        self
    }
}

impl View for Rating {
    fn body(&self) -> AnyView {
        AnyView::new(RatingBody {
            rating: self.snapshot(),
        })
    }
}

impl Rating {
    fn snapshot(&self) -> Rating {
        Rating {
            label: self.label.clone(),
            value: self.value,
            on_change: self.on_change.clone(),
            selection: self.selection,
            appearance: self.appearance.clone(),
            session: self.session.clone(),
        }
    }
}

struct RatingBody {
    rating: Rating,
}

impl PrimitiveView for RatingBody {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(self.rating.selection.is_interactive());
        node.set_is_focusable(self.rating.selection.is_interactive());
        let session: Rc<dyn NodeResource> = self.rating.session.clone();
        node.add_resource(session.clone());
        node.set_attachment(RATING_SESSION, self.rating.session.clone());
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_layout(&self, layout: &LayoutNode) {
        let theme_node = None::<&Node>;
        let _ = theme_node;
        layout.set_flex_shrink(0.0);
    }

    fn update_node(&self, node: &Node) {
        let rating = &self.rating;
        let density = node.composition_value(&ControlSizeEnvironment::key());
        let geometry =
            RatingGeometry::new(&rating.appearance, &rating.selection, density, false, 0.0);
        let session = rating.session.clone();
        *session.value.borrow_mut() = rating.selection.clamped(rating.value);
        *session.on_change.borrow_mut() = Some(rating.on_change.clone());
        let configuration = RatingConfiguration {
            value: rating.selection.clamped(rating.value),
            selection: rating.selection,
            geometry,
        };
        if *session.configuration.borrow() != Some(configuration) {
            session.cancel(node);
            *session.configuration.borrow_mut() = Some(configuration);
        }

        if let Some(layout) = node.layout_node() {
            layout.set_width(Some(geometry.width()));
            layout.set_height(Some(geometry.height()));
            layout.set_flex_shrink(0.0);
        }
        node.set_cursor(Some(if rating.selection.is_interactive() {
            SystemCursor::Pointer
        } else {
            SystemCursor::Arrow
        }));
        if !rating.selection.is_interactive()
            && let Some(chain) = FocusChainHolder::current()
            && chain.focused().is_some_and(|focused| focused == *node)
        {
            chain.clear();
        }

        node.set_accessibility(Some(AccessibilitySemantics {
            role: if rating.selection.is_read_only {
                AccessibilityRole::Image
            } else {
                AccessibilityRole::Slider
            },
            label: rating.label.clone(),
            value: format_value(configuration.value),
            help: format!(
                "{} out of {}",
                format_value(configuration.value),
                rating.selection.maximum
            ),
            state: cove_core::scene::AccessibilityState {
                is_enabled: rating.selection.is_enabled,
                is_read_only: rating.selection.is_read_only,
                ..Default::default()
            },
            ..Default::default()
        }));

        let write = make_writer(rating, node);
        let mut actions = AccessibilityActions::default();
        if rating.selection.is_interactive() {
            let increment = write.clone();
            let decrement = write.clone();
            let selection = rating.selection;
            actions.set_value = Some(Rc::new(move |text: &str| {
                if let Ok(number) = text.parse::<f64>()
                    && number.is_finite()
                {
                    write(number);
                }
            }));
            let current = configuration.value;
            actions.increment = Some(Rc::new(move || {
                increment(current + selection.precision.increment())
            }));
            actions.decrement = Some(Rc::new(move || {
                decrement(current - selection.precision.increment())
            }));
        }
        node.set_accessibility_actions(actions);

        let palette = RatingPalette::resolve(node, &rating.appearance, &rating.selection);
        let appearance = rating.appearance.clone();
        let session_for_draw = session.clone();
        let weak = node.downgrade();
        let geometry_for_draw = geometry;
        node.update_draw(
            (configuration, palette, appearance.symbol.clone()),
            move |list, origin| {
                let Some(owner) = Node::upgrade(&weak) else {
                    return;
                };
                let value = session_for_draw
                    .preview
                    .borrow()
                    .unwrap_or(configuration.value);
                paint(
                    &owner,
                    value,
                    geometry_for_draw,
                    palette,
                    &appearance.symbol,
                    origin,
                    list,
                );
            },
        );

        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };
        if !rating.selection.is_interactive() {
            registry.remove(node);
            return;
        }
        install_input(rating, node, registry, geometry);
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        Vec::new()
    }
}
crate::impl_primitive_view!(RatingBody);

fn format_value(value: f64) -> String {
    if (value - value.round()).abs() < f64::EPSILON {
        format!("{}", value.round() as i64)
    } else {
        format!("{value}")
    }
}

/// The single write path: cancel any press, snap, then report only on change.
fn make_writer(rating: &Rating, node: &Node) -> Rc<dyn Fn(f64)> {
    let selection = rating.selection;
    let on_change = rating.on_change.clone();
    let session = rating.session.clone();
    let weak = node.downgrade();
    Rc::new(move |raw: f64| {
        let Some(owner) = Node::upgrade(&weak) else {
            return;
        };
        session.cancel(&owner);
        let next = selection.snapped(raw);
        if next != *session.value.borrow() {
            on_change(next);
        }
        owner.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
            "ratingValue",
        ));
    })
}

/// A cached raster for one catalog icon at a logical size.
fn icon_raster(slug: &str, size: f32) -> Option<VectorImageRaster> {
    let path = ui_icon_catalog::asset(slug).path()?;
    let initial = crate::primitives::media::image::Image::resolve_path(&path, size, size);
    Some(VectorImageRaster::new(path, initial, size, size))
}

fn paint(
    node: &Node,
    value: f64,
    geometry: RatingGeometry,
    palette: RatingPalette,
    symbol: &RatingSymbol,
    origin: Point,
    list: &mut cove_core::DrawList,
) {
    let frame = node.frame();
    if frame.width <= 0.0 || frame.height <= 0.0 {
        return;
    }
    let opacity = node.opacity();
    let right_to_left = node.resolves_right_to_left();
    let outline = icon_raster(symbol.empty, geometry.star_size);
    let filled = icon_raster(symbol.filled, geometry.star_size);
    let bounds = Rect::new(origin.x, origin.y, frame.width, frame.height);
    for index in 0..geometry.maximum {
        let rect = geometry.star_rect(index, origin, frame.height, right_to_left);
        if let Some(asset) = outline
            .as_ref()
            .map(|raster| raster.resolve(geometry.star_size, geometry.star_size))
        {
            let _ = list.add_image_mask_quad(
                rect,
                asset.texture_id(),
                palette.empty.multiplied_alpha(opacity),
                Point::new(0.0, 0.0),
                Point::new(1.0, 1.0),
            );
        }
        let fraction = ((value - index as f64).clamp(0.0, 1.0)) as f32;
        if fraction > 0.0
            && let Some(ref raster) = filled
        {
            let asset = raster.resolve(geometry.star_size, geometry.star_size);
            if fraction < 1.0 {
                let filled_width = rect.width * fraction;
                let clip_x = if right_to_left {
                    rect.x + rect.width - filled_width
                } else {
                    rect.x
                };
                // A partial symbol is clipped to its filled fraction.
                list.push_clip_rect(Rect::new(clip_x, rect.y, filled_width, rect.height));
            }
            let _ = list.add_image_mask_quad(
                rect,
                asset.texture_id(),
                palette.filled.multiplied_alpha(opacity),
                Point::new(0.0, 0.0),
                Point::new(1.0, 1.0),
            );
            if fraction < 1.0 {
                list.pop_clip_rect();
            }
        }
    }
    let _ = bounds;
    if let Some(chain) = FocusChainHolder::current()
        && chain.focused().is_some_and(|focused| focused == *node)
        && chain.is_focus_visible()
    {
        let theme = theme_for_node(node);
        let _ = list.add_rounded_rect_stroke(
            Rect::new(origin.x, origin.y, frame.width, frame.height),
            theme.radius.sm,
            2.0,
            palette.focus.multiplied_alpha(opacity),
        );
    }
}

fn install_input(
    rating: &Rating,
    node: &Node,
    registry: Rc<cove_core::scene::InteractionRegistry>,
    geometry: RatingGeometry,
) {
    let session = rating.session.clone();
    let selection = rating.selection;
    let write = make_writer(rating, node);

    // Hover previews the value the click would commit.
    let hover_session = session.clone();
    let hover_node = node.downgrade();
    registry.set_hover(
        node,
        Rc::new(move |phase| {
            let Some(owner) = Node::upgrade(&hover_node) else {
                return;
            };
            if phase == cove_core::scene::HoverPhase::Leave {
                hover_session.hover(None, &owner);
            }
        }),
    );

    let motion_session = session.clone();
    let motion_node = node.downgrade();
    let motion_geometry = geometry;
    let motion_write = write.clone();
    registry.set_motion(
        node,
        InputHandlerRoute::control(),
        Rc::new(move |event, phase| {
            if phase != cove_core::scene::EventPhase::Target {
                return EventResult::Ignored;
            }
            let Some(owner) = Node::upgrade(&motion_node) else {
                return EventResult::Ignored;
            };
            let pressed = motion_session.pressed_candidate.borrow().is_some();
            let origin = owner.absolute_frame();
            let inside = motion_geometry.contains(
                event.x,
                event.y,
                Point::new(origin.x, origin.y),
                owner.frame().size(),
            );
            motion_session.hover(
                inside.then(|| {
                    motion_geometry.candidate(
                        event.x,
                        origin.x,
                        owner.resolves_right_to_left(),
                        selection.precision,
                    )
                }),
                &owner,
            );
            let _ = &motion_write;
            if pressed {
                EventResult::Handled
            } else {
                EventResult::Ignored
            }
        }),
    );

    let pointer_session = session.clone();
    let pointer_node = node.downgrade();
    let pointer_geometry = geometry;
    let pointer_write = write.clone();
    registry.set_pointer(
        node,
        InputHandlerRoute::control(),
        Rc::new(move |event, phase, event_phase| {
            if event.button != MouseButton::Left
                || event_phase != cove_core::scene::EventPhase::Target
            {
                return EventResult::Ignored;
            }
            let Some(owner) = Node::upgrade(&pointer_node) else {
                return EventResult::Ignored;
            };
            let origin = owner.absolute_frame();
            let right_to_left = owner.resolves_right_to_left();
            match phase {
                PointerPhase::Down => {
                    if !pointer_geometry.contains(
                        event.x,
                        event.y,
                        Point::new(origin.x, origin.y),
                        owner.frame().size(),
                    ) {
                        return EventResult::Ignored;
                    }
                    pointer_session.begin(
                        pointer_geometry.candidate(
                            event.x,
                            origin.x,
                            right_to_left,
                            selection.precision,
                        ),
                        &owner,
                    );
                }
                PointerPhase::Up => {
                    if pointer_session.pressed_candidate.borrow().is_none() {
                        return EventResult::Ignored;
                    }
                    let inside = pointer_geometry.contains(
                        event.x,
                        event.y,
                        Point::new(origin.x, origin.y),
                        owner.frame().size(),
                    );
                    let candidate = pointer_geometry.candidate(
                        event.x,
                        origin.x,
                        right_to_left,
                        selection.precision,
                    );
                    // Clicking the current value clears, but only when the pointer
                    // never moved: a drag always commits.
                    let clears = selection.allows_clear
                        && !*pointer_session.has_moved.borrow()
                        && candidate == selection.clamped(*pointer_session.value.borrow());
                    pointer_session.cancel(&owner);
                    if inside {
                        pointer_write(if clears { 0.0 } else { candidate });
                    }
                }
            }
            EventResult::Handled
        }),
    );

    let key_session = session.clone();
    let key_node = node.downgrade();
    let key_write = write;
    registry.set_key(
        node,
        InputHandlerRoute::control(),
        Rc::new(move |event, phase| {
            if phase != cove_core::scene::EventPhase::Target {
                return EventResult::Ignored;
            }
            if event.modifiers.has_ctrl() || event.modifiers.has_gui() || event.modifiers.has_alt()
            {
                return EventResult::Ignored;
            }
            let Some(owner) = Node::upgrade(&key_node) else {
                return EventResult::Ignored;
            };
            let current = selection.clamped(*key_session.value.borrow());
            let right_to_left = owner.resolves_right_to_left();
            // Shift moves a whole symbol, so fine precision stays quick to traverse.
            let step = if event.modifiers.has_shift() {
                1.0
            } else {
                selection.precision.increment()
            };
            match event.scancode {
                crate::Scancode::ARROW_LEFT => {
                    key_write(current + if right_to_left { step } else { -step })
                }
                crate::Scancode::ARROW_RIGHT => {
                    key_write(current + if right_to_left { -step } else { step })
                }
                crate::Scancode::ARROW_DOWN => key_write(current - step),
                crate::Scancode::ARROW_UP => key_write(current + step),
                crate::Scancode::HOME => key_write(0.0),
                crate::Scancode::END => key_write(selection.maximum as f64),
                crate::Scancode::BACKSPACE | crate::Scancode::DELETE => {
                    if !selection.allows_clear {
                        return EventResult::Ignored;
                    }
                    key_write(0.0)
                }
                crate::Scancode::ESCAPE => key_session.cancel(&owner),
                _ => return EventResult::Ignored,
            }
            EventResult::Handled
        }),
    );
}
