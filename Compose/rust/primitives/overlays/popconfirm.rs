//! Translation of Primitives/Overlays/Popconfirm/Popconfirm.swift.
//!
//! An anchored question with explicit Cancel and Confirm actions.
use std::rc::Rc;

use cove_core::scene::{AccessibilityRole, Align, FlexDirection, Justify, Wrap};
use cove_core::State;
use cove_imaging::BundleImageResource;

use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::layout::LayoutFrameExt;
use crate::primitives::controls::button::{Button, ButtonRole};
use crate::primitives::media::Icon;
use crate::primitives::overlays::anchored::{
    PopoverOverflow, PopoverPlacement, PopoverSurface,
};
use crate::primitives::overlays::popover::{Popover, PopoverTriggerDescription};
use crate::primitives::text::Text;
use crate::modifiers::style::SemanticStyleViewExt;
use crate::theme::styles::buttons::{
    destructive, primary, secondary, AnyButtonStyle, ButtonStyleViewExt,
};
use crate::theme::{SemanticColorRef, SemanticFontRef};
use crate::{AnyView, BoxView, Row, TupleView, View};

/// Small icon size used by the trigger; matches `Theme.icons.small` (12pt).
const TRIGGER_ICON_SIZE: f32 = 12.0;

/// The question a popconfirm asks, plus the wording of its two answers.
#[derive(Clone)]
struct PopconfirmQuestion {
    title: String,
    message: String,
    confirm_title: String,
    cancel_title: String,
    role: ButtonRole,
}

/// An anchored question with explicit Cancel and Confirm actions.
pub struct Popconfirm {
    trigger_title: String,
    icon: Option<BundleImageResource>,
    question: PopconfirmQuestion,
    placement: PopoverPlacement,
    is_enabled: bool,
    on_confirm: Rc<dyn Fn()>,
    is_presented: State<bool>,
}

impl Popconfirm {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        trigger_title: impl Into<String>,
        icon: Option<BundleImageResource>,
        title: impl Into<String>,
        message: impl Into<String>,
        confirm_title: impl Into<String>,
        cancel_title: impl Into<String>,
        role: ButtonRole,
        placement: PopoverPlacement,
        is_enabled: bool,
        on_confirm: impl Fn() + 'static,
    ) -> Self {
        let question = PopconfirmQuestion {
            title: title.into(),
            message: message.into(),
            confirm_title: confirm_title.into(),
            cancel_title: cancel_title.into(),
            role,
        };
        Self {
            trigger_title: trigger_title.into(),
            icon,
            question,
            placement,
            is_enabled,
            on_confirm: Rc::new(on_confirm),
            is_presented: State::new(false),
        }
    }

    /// Convenience matching the Swift default-argument call sites.
    pub fn standard(
        trigger_title: impl Into<String>,
        title: impl Into<String>,
        on_confirm: impl Fn() + 'static,
    ) -> Self {
        Self::new(
            trigger_title,
            None,
            title,
            "",
            "Confirm",
            "Cancel",
            ButtonRole::Normal,
            PopoverPlacement::Bottom,
            true,
            on_confirm,
        )
    }

    fn trigger_label(&self) -> AnyView {
        let is_destructive = self.question.role == ButtonRole::Destructive;
        let mut items: Vec<AnyView> = Vec::new();
        if let Some(icon) = &self.icon {
            let color = if is_destructive {
                SemanticColorRef::error()
            } else {
                SemanticColorRef::on_surface_variant()
            };
            let icon = Icon::with_semantic(icon.clone(), TRIGGER_ICON_SIZE, color)
                .accessibility_hidden(true);
            items.push(AnyView::new(icon));
        }
        let text = Text::new(self.trigger_title.clone())
            .semantic_font(SemanticFontRef::label())
            .semantic_foreground(if is_destructive {
                SemanticColorRef::error()
            } else {
                SemanticColorRef::on_surface()
            });
        items.push(AnyView::new(text));
        AnyView::new(Row::new(
            crate::VerticalAlignment::Center,
            6.0,
            AnyView::new(TupleView::new(items)),
        ))
    }

    fn content(&self) -> AnyView {
        let question = self.question.clone();
        let is_presented = self.is_presented.clone();
        let on_confirm = self.on_confirm.clone();
        let enabled = self.is_enabled;
        let cancel = {
            let is_presented = is_presented.clone();
            Button::with_options(
                Text::new(question.cancel_title.clone()).semantic_font(SemanticFontRef::label()),
                move || is_presented.set(false),
                ButtonRole::Cancel,
                true,
                false,
                false,
                None,
            )
            .button_style(secondary())
        };
        let confirm = Button::with_options(
            Text::new(question.confirm_title.clone()).semantic_font(SemanticFontRef::label()),
            move || {
                if !is_presented.get() || !enabled {
                    return;
                }
                is_presented.set(false);
                on_confirm();
            },
            question.role,
            true,
            false,
            false,
            None,
        )
        .button_style_any(if question.role == ButtonRole::Destructive {
            AnyButtonStyle::new(destructive())
        } else {
            AnyButtonStyle::new(primary())
        });
        let mut column_items: Vec<AnyView> = Vec::new();
        let heading = Text::new(question.title.clone())
            .semantic_font(SemanticFontRef::body_strong())
            .accessibility(|semantics| semantics.role = AccessibilityRole::Heading);
        column_items.push(AnyView::new(heading));
        if !question.message.is_empty() {
            let message = Text::new(question.message.clone())
                .semantic_foreground(SemanticColorRef::on_surface_variant());
            column_items.push(AnyView::new(message));
        }
        let button_row = BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexEnd,
            Wrap::Wrap,
            8.0,
            AnyView::new(TupleView::new(vec![AnyView::new(cancel), AnyView::new(confirm)])),
        )
        .frame_min_width(0.0);
        column_items.push(AnyView::new(button_row));
        let column = BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            10.0,
            AnyView::new(TupleView::new(column_items)),
        )
        .frame_min_width(0.0)
        .accessibility(move |semantics| {
            semantics.role = AccessibilityRole::Dialog;
            semantics.label = question.title.clone();
        });
        AnyView::new(column)
    }
}

impl View for Popconfirm {
    fn body(&self) -> AnyView {
        let popover = Popover::new(self.trigger_label(), self.content())
            .is_presented(self.is_presented.binding())
            .enabled(self.is_enabled)
            .width(300.0)
            .placement(self.placement)
            .surface(PopoverSurface::Bubble)
            .overflow(PopoverOverflow::Constrain)
            .trigger_description(PopoverTriggerDescription {
                label: self.trigger_title.clone(),
                value: String::new(),
                help: None,
            });
        AnyView::new(popover)
    }
}
