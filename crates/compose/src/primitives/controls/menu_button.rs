//! Translation of Primitives/Controls/MenuButton.swift.
//!
//! A button that opens a menu anchored to itself. With a native menu backend on
//! a pointer surface it shows the system menu; otherwise it uses the regular
//! popover trigger contract, so the menu escapes clipping, closes on Escape or
//! an outside click, and exposes expanded state to accessibility.
use crate::Scope;
use cove_core::scene::{Align, FlexDirection, Justify, NativeMenuPresenter, Node, Wrap};
use cove_core::{Environment, Point, Rect, Signal};

use crate::adaptive::SurfaceReader;
use crate::modifiers::layout::{CaptureFrameExt, FrameBox, LayoutFrameExt};
use crate::primitives::controls::button::{ActionControlAccessibility, Button, ButtonRole};
use crate::primitives::overlays::anchored::PopoverPlacement;
use crate::primitives::overlays::menu::{Menu, MenuEntry, NativeMenuBuilder};
use crate::primitives::overlays::modal::uses_sheet;
use crate::primitives::overlays::popover::{Popover, PopoverTriggerDescription};
use crate::primitives::text::Text;
use crate::theme::SemanticFontRef;
use crate::theme::styles::buttons::{ButtonStyleViewExt, SecondaryButtonStyle};
use crate::{AnyView, BoxView, PrimitiveView, TupleView, View, ViewExt, impl_primitive_view};

/// A button that opens a menu.
#[derive(Clone)]
pub struct MenuButton<Label: View + Clone + 'static> {
    pub entries: Vec<MenuEntry>,
    pub is_enabled: bool,
    pub width: Option<f32>,
    pub placement: PopoverPlacement,
    pub accessibility_label: Option<String>,
    pub tooltip: Option<String>,
    /// Drop the push-button bezel, for a trigger inside a control that draws
    /// its own (a split button).
    pub is_borderless: bool,
    pub label: Label,
    /// Told when the menu opens and closes, for owners that must stay in place
    /// while it is shown (an auto-hiding control bar).
    pub on_open_change: Option<std::rc::Rc<dyn Fn(bool)>>,
}

impl<Label: View + Clone + 'static> MenuButton<Label> {
    pub fn ghost(mut self) -> Self {
        self.is_borderless = true;
        self
    }

    pub fn new(entries: Vec<MenuEntry>, label: Label) -> Self {
        Self {
            entries,
            is_enabled: true,
            width: None,
            placement: PopoverPlacement::BottomLeading,
            accessibility_label: None,
            tooltip: None,
            is_borderless: false,
            label,
            on_open_change: None,
        }
    }

    pub fn on_open_change(mut self, handler: impl Fn(bool) + 'static) -> Self {
        self.on_open_change = Some(std::rc::Rc::new(handler));
        self
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    pub fn placement(mut self, placement: PopoverPlacement) -> Self {
        self.placement = placement;
        self
    }

    pub fn accessibility_label(mut self, label: impl Into<String>) -> Self {
        self.accessibility_label = Some(label.into());
        self
    }

    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// The menu the popover presents; activating an item closes it.
    pub fn menu(&self, presented: Signal<bool>) -> Menu {
        let menu = Menu::new(self.entries.clone());
        let menu = match self.width {
            Some(width) => menu.width(width),
            None => menu,
        };
        let on_open_change = self.on_open_change.clone();
        menu.on_item_activated(move || {
            if presented.get_untracked()
                && let Some(handler) = on_open_change.as_ref()
            {
                handler(false);
            }
            presented.set(false);
        })
    }

    /// A button that opens the system menu anchored at its own corner.
    fn native_button(&self) -> AnyView {
        let frame = FrameBox::new();
        let captured = frame.clone();
        let entries = self.entries.clone();
        let placement = self.placement;
        let on_open_change = self.on_open_change.clone();
        let button = Button::with_options(
            self.label.clone(),
            move || {
                let Some(rect) = captured.current() else {
                    return;
                };
                let built = NativeMenuBuilder::build(&entries);
                let snapshot = built.snapshot.clone();
                if let Some(handler) = on_open_change.as_ref() {
                    handler(true);
                }
                let closed = on_open_change.clone();
                NativeMenuPresenter::present(
                    &snapshot,
                    anchor(rect, placement),
                    Box::new(move |chosen| {
                        if let Some(handler) = closed.as_ref() {
                            handler(false);
                        }
                        if let Some(chosen) = chosen {
                            built.run(chosen);
                        }
                    }),
                );
            },
            ButtonRole::Normal,
            self.is_enabled,
            false,
            false,
            self.tooltip.clone(),
        )
        .button_style_any(if self.is_borderless {
            crate::theme::styles::buttons::button_style::AnyButtonStyle::new(
                crate::theme::styles::buttons::GhostButtonStyle,
            )
        } else {
            crate::theme::styles::buttons::button_style::AnyButtonStyle::new(SecondaryButtonStyle)
        })
        .capture_frame(frame);
        match &self.accessibility_label {
            Some(label) => AnyView::new(
                button.composition_local(ActionControlAccessibility::label(), Some(label.clone())),
            ),
            None => AnyView::new(button),
        }
    }

    fn popover_button(&self, presented: Signal<bool>) -> AnyView {
        let binding = match self.on_open_change.clone() {
            Some(handler) => cove_core::Binding::new(
                move || presented.get(),
                move |open: bool| {
                    if presented.get_untracked() != open {
                        handler(open);
                    }
                    presented.set(open);
                },
            ),
            None => presented.into(),
        };
        let mut popover = Popover::new(self.label.clone(), self.menu(presented))
            .is_presented(binding)
            .enabled(self.is_enabled)
            .placement(self.placement);
        popover = if self.is_borderless {
            popover.trigger_style(crate::theme::styles::buttons::GhostButtonStyle)
        } else {
            popover.trigger_style(SecondaryButtonStyle)
        };
        if let Some(width) = self.width {
            popover = popover.width(width);
        }
        if let Some(label) = &self.accessibility_label {
            popover = popover.trigger_description(PopoverTriggerDescription {
                label: label.clone(),
                value: String::new(),
                help: self.tooltip.clone(),
            });
        }
        AnyView::new(popover)
    }
}

/// The anchor for a menu at a button's corner: bottom corners for bottom and
/// side placements, top corners for top placements.
fn anchor(frame: Rect, placement: PopoverPlacement) -> Point {
    use PopoverPlacement::*;
    match placement {
        TopLeading | Top | LeadingTop => Point::new(frame.x, frame.y),
        TopTrailing | TrailingTop => Point::new(frame.max_x(), frame.y),
        Leading | LeadingBottom | BottomLeading | Bottom => Point::new(frame.x, frame.max_y()),
        Trailing | TrailingBottom | BottomTrailing => Point::new(frame.max_x(), frame.max_y()),
    }
}

impl<Label: View + Clone + 'static> View for MenuButton<Label> {
    fn body(&self, cx: &Scope) -> AnyView {
        let presented = cx.state(|| false);
        let this = self.clone();
        AnyView::new(SurfaceReader::new(move |surface: &Environment| {
            if NativeMenuPresenter::is_installed() && !uses_sheet(surface) {
                this.native_button()
            } else {
                this.popover_button(presented)
            }
        }))
    }
}

/// A text-only menu button: the label and the dropdown arrow drawn together, so
/// it reads as a menu trigger.
pub struct MenuButtonTitle {
    pub title: String,
    pub entries: Vec<MenuEntry>,
    pub is_enabled: bool,
    pub width: Option<f32>,
    pub placement: PopoverPlacement,
    pub tooltip: Option<String>,
}

impl MenuButtonTitle {
    pub fn new(title: impl Into<String>, entries: Vec<MenuEntry>) -> Self {
        Self {
            title: title.into(),
            entries,
            is_enabled: true,
            width: None,
            placement: PopoverPlacement::BottomLeading,
            tooltip: None,
        }
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    pub fn placement(mut self, placement: PopoverPlacement) -> Self {
        self.placement = placement;
        self
    }

    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }
}

impl View for MenuButtonTitle {
    fn body(&self, _cx: &Scope) -> AnyView {
        let title = self.title.clone();
        let label = MenuButton::new(
            self.entries.clone(),
            AnyView::new(MenuButtonLabel {
                title: title.clone(),
            }),
        )
        .enabled(self.is_enabled)
        .placement(self.placement)
        .accessibility_label(title);
        let label = match self.width {
            Some(width) => label.width(width),
            None => label,
        };
        let label = match self.tooltip.clone() {
            Some(tooltip) => label.tooltip(tooltip),
            None => label,
        };
        AnyView::new(label)
    }
}

/// The title and its dropdown chevron.
struct MenuButtonLabel {
    title: String,
}

impl PrimitiveView for MenuButtonLabel {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![AnyView::new(
            BoxView::new(
                FlexDirection::Row, Align::Center, Justify::FlexStart, Wrap::NoWrap, 6.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(Text::new(self.title.clone()).truncated()),
                    AnyView::new(crate::primitives::controls::button::ButtonIcon::new(
                        crate::primitives::controls::button::ButtonIconSource::resource(
                            crate::primitives::media::ui_icon_catalog::navigation::dropdown_chevron_down(),
                        ),
                        Some(12.0),
                        None,
                    )),
                ])),
            )
            .frame_min_width(0.0),
        )]
    }
}
impl_primitive_view!(MenuButtonLabel);

impl MenuButtonTitle {
    /// The label font used by the text-only menu button.
    pub fn font() -> SemanticFontRef {
        SemanticFontRef::label()
    }
}

impl Clone for MenuButtonTitle {
    fn clone(&self) -> Self {
        Self {
            title: self.title.clone(),
            entries: self.entries.clone(),
            is_enabled: self.is_enabled,
            width: self.width,
            placement: self.placement,
            tooltip: self.tooltip.clone(),
        }
    }
}
