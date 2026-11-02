//! Translation of Primitives/Layout/Carousel/Carousel.swift.
//!
//! A paged view: one page fills the viewport, the rest are one gesture away.
//! Arrows at the edges, indicators underneath, left/right/Home/End on the
//! keyboard, and a horizontal drag with the same threshold-based feel as a
//! native swipe.
use crate::Scope;
mod autoplay;
mod indicator;

use std::rc::Rc;

use cove_core::scene::{AccessibilityRole, Align, FlexDirection, Justify, LayoutNode, Wrap};
use cove_core::{Binding, State};
use cove_platform::KeyEvent;

use crate::modifiers::interaction::{AccessibilityExt, HoverExt};
use crate::modifiers::layout::{FlexExt, FrameDimension, LayoutFrameExt};
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::primitives::controls::button::{Button, ButtonIconSource, ButtonRole};
use crate::primitives::overlays::selection_key_host::SelectionKeyHost;
use crate::primitives::text::Text;
use crate::theme::styles::buttons::{ButtonStyleViewExt, ghost, plain};
use crate::theme::{
    ControlSize, ControlSizeViewExt, SemanticColorRef, SemanticFontRef, Theme, theme_for_node,
};
use crate::{AnyView, BoxView, Node, PrimitiveView, Row, TupleView, View};

pub use autoplay::{
    CarouselAutoplay, CarouselAutoplayHost, CarouselAutoplaySession, ReducedMotionReader,
};
pub use indicator::{CarouselIndicator, CarouselProgressDot};

/// One page of a [`Carousel`].
#[derive(Clone)]
pub struct CarouselPage<Id: PartialEq + Clone> {
    pub id: Id,
    /// Optional line shown under the viewport; changes with the page.
    pub caption: String,
    pub content: AnyView,
}

impl<Id: PartialEq + Clone> CarouselPage<Id> {
    pub fn new(id: Id, content: AnyView) -> Self {
        Self {
            id,
            caption: String::new(),
            content,
        }
    }

    pub fn caption(mut self, caption: impl Into<String>) -> Self {
        self.caption = caption.into();
        self
    }
}

/// Shared paging logic captured by action closures. It owns a snapshot of the
/// pages so a mounted closure never borrows the short-lived view reference.
struct CarouselActions<Id: PartialEq + Eq + Clone + Send + Sync + 'static> {
    pages: Vec<CarouselPage<Id>>,
    selection: Option<Binding<Id>>,
    internal: State<Option<Id>>,
    allows_wrap: bool,
    is_enabled: bool,
}

impl<Id: PartialEq + Eq + Clone + Send + Sync + 'static> CarouselActions<Id> {
    fn active_id(&self) -> Option<Id> {
        if let Some(selection) = &self.selection {
            Some(selection.get())
        } else if let Some(id) = self.internal.get() {
            Some(id)
        } else {
            self.pages.first().map(|page| page.id.clone())
        }
    }

    fn current_index(&self) -> usize {
        match self.active_id() {
            Some(id) => self
                .pages
                .iter()
                .position(|page| page.id == id)
                .unwrap_or(0),
            None => 0,
        }
    }

    fn can_step(&self, delta: i32) -> bool {
        let index = self.current_index();
        if delta < 0 {
            index > 0
        } else {
            index < self.pages.len() - 1
        }
    }

    fn go_to(&self, index: usize) {
        if !self.is_enabled || index >= self.pages.len() {
            return;
        }
        let id = self.pages[index].id.clone();
        if let Some(selection) = &self.selection {
            selection.set(id);
        } else {
            self.internal.set(Some(id));
        }
    }

    fn move_by(&self, delta: i32) {
        if !self.is_enabled || self.pages.is_empty() {
            return;
        }
        let index = self.current_index();
        let count = self.pages.len();
        let next = if self.allows_wrap {
            (index as i32 + delta).rem_euclid(count as i32) as usize
        } else {
            (index as i32 + delta).clamp(0, count as i32 - 1) as usize
        };
        self.go_to(next);
    }
}

/// Static, copyable configuration of one carousel body.
#[derive(Clone, Copy)]
struct CarouselConfig {
    page_count: usize,
    shows_arrows: bool,
    indicator: CarouselIndicator,
    autoplay: Option<CarouselAutoplay>,
    allows_wrap: bool,
    height: Option<f32>,
    is_enabled: bool,
}

/// A paged view: one page fills the viewport, the rest are one gesture away.
pub struct Carousel<Id: PartialEq + Eq + Clone + std::hash::Hash + Send + Sync + 'static> {
    pub pages: Vec<CarouselPage<Id>>,
    pub selection: Option<Binding<Id>>,
    pub shows_arrows: bool,
    pub indicator: CarouselIndicator,
    pub autoplay: Option<CarouselAutoplay>,
    pub allows_wrap: bool,
    pub height: Option<f32>,
    pub is_enabled: bool,
    internal_selection: State<Option<Id>>,
    rotation: Rc<CarouselAutoplaySession>,
}

impl<Id: PartialEq + Eq + Clone + std::hash::Hash + Send + Sync + 'static> Carousel<Id> {
    pub fn new(pages: Vec<CarouselPage<Id>>) -> Self {
        Self {
            pages,
            selection: None,
            shows_arrows: true,
            indicator: CarouselIndicator::Automatic,
            autoplay: None,
            allows_wrap: true,
            height: None,
            is_enabled: true,
            internal_selection: State::new(None),
            rotation: Rc::new(CarouselAutoplaySession::new()),
        }
    }

    pub fn selection(mut self, selection: Binding<Id>) -> Self {
        self.selection = Some(selection);
        self
    }
    pub fn shows_arrows(mut self, shows_arrows: bool) -> Self {
        self.shows_arrows = shows_arrows;
        self
    }
    pub fn indicator(mut self, indicator: CarouselIndicator) -> Self {
        self.indicator = indicator;
        self
    }
    pub fn autoplay(mut self, autoplay: CarouselAutoplay) -> Self {
        self.autoplay = Some(autoplay);
        self
    }
    pub fn allows_wrap(mut self, allows_wrap: bool) -> Self {
        self.allows_wrap = allows_wrap;
        self
    }
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }
    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    fn config(&self) -> CarouselConfig {
        CarouselConfig {
            page_count: self.pages.len(),
            shows_arrows: self.shows_arrows,
            indicator: self.indicator,
            autoplay: self.autoplay,
            allows_wrap: self.allows_wrap,
            height: self.height,
            is_enabled: self.is_enabled,
        }
    }

    fn actions(&self) -> Rc<CarouselActions<Id>> {
        Rc::new(CarouselActions {
            pages: self.pages.clone(),
            selection: self.selection.clone(),
            internal: self.internal_selection.clone(),
            allows_wrap: self.allows_wrap,
            is_enabled: self.is_enabled,
        })
    }
}

/// The interval while rotation can make progress.
fn running_interval<Id: PartialEq + Eq + Clone + Send + Sync + 'static>(
    config: &CarouselConfig,
    actions: &CarouselActions<Id>,
) -> Option<f64> {
    if config.autoplay.is_some()
        && config.is_enabled
        && config.page_count > 1
        && (config.allows_wrap || actions.can_step(1))
    {
        Some(config.autoplay.expect("checked above").interval)
    } else {
        None
    }
}

fn build_viewport<Id: PartialEq + Eq + Clone + Send + Sync + 'static>(
    theme: &Theme,
    actions: Rc<CarouselActions<Id>>,
    config: CarouselConfig,
) -> AnyView {
    let mut row_items: Vec<AnyView> = Vec::new();
    if config.shows_arrows && config.page_count > 1 {
        row_items.push(build_step_button(theme, -1, actions.clone(), config));
    }
    let swipe_actions = actions.clone();
    let current = actions.current_index();
    let page_content = actions
        .pages
        .get(current)
        .map(|page| page.content.clone())
        .unwrap_or_else(|| AnyView::new(crate::EmptyView));
    let swipe = crate::primitives::interaction::HorizontalSwipe::new(
        move |direction| {
            use crate::primitives::interaction::HorizontalSwipeDirection;
            swipe_actions.move_by(if direction == HorizontalSwipeDirection::Left {
                1
            } else {
                -1
            });
        },
        page_content,
    )
    .enabled(config.is_enabled && config.page_count > 1)
    .flex(1.0)
    .clipped()
    .corner_radius(theme.radius.md)
    .semantic_background(SemanticColorRef::surface_sunken());
    row_items.push(AnyView::new(swipe));
    if config.shows_arrows && config.page_count > 1 {
        row_items.push(build_step_button(theme, 1, actions.clone(), config));
    }
    let row = BoxView::new(
        FlexDirection::Row,
        Align::Stretch,
        Justify::FlexStart,
        Wrap::NoWrap,
        theme.spacing.sm,
        AnyView::new(TupleView::new(row_items)),
    )
    .frame(
        Some(FrameDimension::Percent(100.0)),
        None,
        None,
        None,
        None,
        None,
    );
    let spanning = AnyView::new(row);
    match config.height {
        Some(height) => AnyView::new(spanning.frame(
            None,
            Some(FrameDimension::Points(if height.is_finite() {
                height.max(1.0)
            } else {
                180.0
            })),
            None,
            None,
            None,
            None,
        )),
        None => spanning,
    }
}

fn build_controls<Id: PartialEq + Eq + Clone + Send + Sync + 'static>(
    theme: &Theme,
    paused: bool,
    actions: Rc<CarouselActions<Id>>,
    rotation: Rc<CarouselAutoplaySession>,
    config: CarouselConfig,
) -> AnyView {
    let mut items: Vec<AnyView> = Vec::new();
    if config.autoplay.is_some() && config.page_count > 1 {
        let icon = if paused {
            crate::primitives::media::ui_icon_catalog::editor::play()
        } else {
            crate::primitives::media::ui_icon_catalog::editor::pause()
        };
        let toggle_rotation = rotation.clone();
        let button = Button::icon_with_options(
            ButtonIconSource::resource(icon),
            Some(12.0),
            None,
            ButtonRole::Normal,
            config.is_enabled,
            false,
            false,
            Some(if paused { "Play" } else { "Pause" }.to_string()),
            move || toggle_rotation.set_paused(!paused),
        )
        .button_style(ghost())
        .control_size(ControlSize::Mini)
        .accessibility_label(if paused {
            "Start automatic rotation"
        } else {
            "Pause automatic rotation"
        });
        items.push(AnyView::new(button));
    }
    match config.indicator.resolved(config.page_count) {
        CarouselIndicator::Dots => items.push(build_dots(theme, actions.clone(), rotation, config)),
        CarouselIndicator::Fraction => {
            let text = Text::new(format!(
                "{} / {}",
                actions.current_index() + 1,
                config.page_count
            ))
            .semantic_font(SemanticFontRef::caption())
            .semantic_foreground(SemanticColorRef::on_surface_muted())
            .accessibility_hidden(true);
            items.push(AnyView::new(text));
        }
        CarouselIndicator::Automatic | CarouselIndicator::Hidden => {}
    }
    AnyView::new(Row::new(
        crate::VerticalAlignment::Center,
        6.0,
        AnyView::new(TupleView::new(items)),
    ))
}

fn build_dots<Id: PartialEq + Eq + Clone + Send + Sync + 'static>(
    theme: &Theme,
    actions: Rc<CarouselActions<Id>>,
    rotation: Rc<CarouselAutoplaySession>,
    config: CarouselConfig,
) -> AnyView {
    let mut items: Vec<AnyView> = Vec::new();
    let current = actions.current_index();
    let paused = rotation.is_paused(false);
    let running = running_interval(&config, &actions);
    for index in 0..config.page_count {
        let dot = if index == current {
            AnyView::new(CarouselProgressDot::new(
                rotation.clone(),
                if paused { None } else { running },
            ))
        } else {
            AnyView::new(
                BoxView::new(
                    FlexDirection::Row,
                    Align::Center,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    0.0,
                    AnyView::new(crate::EmptyView),
                )
                .frame(
                    Some(FrameDimension::Points(7.0)),
                    Some(FrameDimension::Points(7.0)),
                    None,
                    None,
                    None,
                    None,
                )
                .background_color(theme.colors.on_surface_muted.multiplied_alpha(0.45))
                .corner_radius(3.5),
            )
        };
        let dot_actions = actions.clone();
        let button = Button::with_options(
            dot,
            move || dot_actions.go_to(index),
            ButtonRole::Normal,
            config.is_enabled,
            false,
            index == current,
            Some(format!("Page {}", index + 1)),
        )
        .button_style(plain())
        .accessibility_label(format!("Page {}", index + 1))
        .accessibility(move |semantics| semantics.state.is_selected = index == current)
        .frame(
            Some(FrameDimension::Points(24.0)),
            Some(FrameDimension::Points(24.0)),
            None,
            None,
            None,
            None,
        );
        items.push(AnyView::new(button));
    }
    AnyView::new(Row::new(
        crate::VerticalAlignment::Center,
        6.0,
        AnyView::new(TupleView::new(items)),
    ))
}

fn build_step_button<Id: PartialEq + Eq + Clone + Send + Sync + 'static>(
    theme: &Theme,
    delta: i32,
    actions: Rc<CarouselActions<Id>>,
    config: CarouselConfig,
) -> AnyView {
    let enabled = config.is_enabled && (config.allows_wrap || actions.can_step(delta));
    let icon_resource = if delta < 0 {
        crate::primitives::media::ui_icon_catalog::navigation::chevron_left()
    } else {
        crate::primitives::media::ui_icon_catalog::navigation::chevron_right()
    };
    AnyView::new(
        Button::icon_with_options(
            ButtonIconSource::resource(icon_resource),
            Some(theme.icons.medium),
            None,
            ButtonRole::Normal,
            enabled,
            false,
            false,
            Some(if delta < 0 { "Previous" } else { "Next" }.to_string()),
            move || actions.move_by(delta),
        )
        .button_style(ghost())
        .control_size(ControlSize::Small)
        .accessibility_label(if delta < 0 {
            "Previous page"
        } else {
            "Next page"
        }),
    )
}

/// Builds the full carousel subtree at node time, reading both the theme and
/// the reduced-motion preference there. Replaces the Swift `ThemeReader` /
/// `ReducedMotionReader` nesting without borrowing either beyond the call.
fn build_subtree<Id: PartialEq + Eq + Clone + Send + Sync + 'static>(
    node: &Node,
    actions: Rc<CarouselActions<Id>>,
    rotation: Rc<CarouselAutoplaySession>,
    config: CarouselConfig,
) -> AnyView {
    let theme = theme_for_node(node);
    let reduced_motion = node.prefers_reduced_motion();
    let paused = rotation.is_paused(reduced_motion);
    let viewport_index = actions.current_index();

    let mut column_items: Vec<AnyView> = Vec::new();
    column_items.push(build_viewport(&theme, actions.clone(), config));
    if config.autoplay.is_some()
        || config.indicator.resolved(config.page_count) != CarouselIndicator::Hidden
    {
        column_items.push(build_controls(
            &theme,
            paused,
            actions.clone(),
            rotation.clone(),
            config,
        ));
    }
    if let Some(page) = actions.pages.get(viewport_index)
        && !page.caption.is_empty()
    {
        let caption = Text::new(page.caption.clone())
            .semantic_font(SemanticFontRef::caption())
            .semantic_foreground(SemanticColorRef::on_surface_muted());
        column_items.push(AnyView::new(caption));
    }
    let column = BoxView::new(
        FlexDirection::Column,
        Align::Center,
        Justify::FlexStart,
        Wrap::NoWrap,
        theme.spacing.sm,
        AnyView::new(TupleView::new(column_items)),
    )
    .on_hover(move |hovered| rotation.set_hovered(hovered))
    .accessibility(move |semantics| {
        semantics.role = AccessibilityRole::Group;
        semantics.label = "Carousel".to_string();
        semantics.value = if config.page_count == 0 {
            "No pages".to_string()
        } else {
            format!("Page {} of {}", viewport_index + 1, config.page_count)
        };
        semantics.state.is_enabled = config.is_enabled;
    })
    .opacity(if config.is_enabled { 1.0 } else { 0.55 });
    let key_actions = actions.clone();
    AnyView::new(SelectionKeyHost::new(
        column,
        config.is_enabled && config.page_count > 0,
        move |event: &KeyEvent| {
            if !config.is_enabled || config.page_count <= 1 || !event.modifiers.is_empty() {
                return false;
            }
            match event.scancode {
                crate::Scancode::ARROW_LEFT => key_actions.move_by(-1),
                crate::Scancode::ARROW_RIGHT => key_actions.move_by(1),
                crate::Scancode::HOME => key_actions.go_to(0),
                crate::Scancode::END => key_actions.go_to(config.page_count - 1),
                _ => return false,
            }
            true
        },
    ))
}

/// Primitive host that rebuilds its subtree when the node is mounted.
struct CarouselBodyHost<Id: PartialEq + Eq + Clone + Send + Sync + 'static> {
    actions: Rc<CarouselActions<Id>>,
    rotation: Rc<CarouselAutoplaySession>,
    config: CarouselConfig,
}

impl<Id: PartialEq + Eq + Clone + Send + Sync + 'static> PrimitiveView for CarouselBodyHost<Id> {
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
        vec![build_subtree(
            node,
            self.actions.clone(),
            self.rotation.clone(),
            self.config,
        )]
    }
}
impl<Id: PartialEq + Eq + Clone + Send + Sync + 'static> View for CarouselBodyHost<Id> {
    fn kind(&self) -> crate::ViewKind<'_> {
        crate::ViewKind::Primitive(self)
    }
}

impl<Id: PartialEq + Eq + Clone + std::hash::Hash + Send + Sync + 'static> View for Carousel<Id> {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(CarouselBodyHost {
            actions: self.actions(),
            rotation: self.rotation.clone(),
            config: self.config(),
        })
    }

    fn dynamic_properties(&self) -> Vec<crate::DynamicField> {
        crate::dynamic_fields!(self; internal_selection)
    }
}
