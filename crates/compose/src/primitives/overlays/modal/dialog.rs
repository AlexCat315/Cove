//! Translation of `Primitives/Overlays/Modal/Dialog.swift`.
//!
//! Structured modal header, scrollable content and a separate action area.
//! On touch phones it rises from the bottom edge and is dragged away by its
//! grabber.

use std::rc::Rc;

use crate::adaptive::SurfaceReader;
use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::layout::{EdgeInsets, FlexExt, LayoutFrameExt, PaddingExt};
use crate::modifiers::style::SemanticStyleViewExt;
use crate::primitives::controls::button::{Button, ButtonIconSource, ButtonRole};
use crate::primitives::feedback::StatusTone;
use crate::primitives::layout::ActionGroupLayout;
use crate::primitives::layout::{ScrollAxis, ScrollView, ScrollbarGutter};
use crate::primitives::media::ui_icon_catalog;
use crate::primitives::overlays::modal::{
    Modal, ModalDismissal, ModalGeometry, ModalOptions, ModalPlacement, SheetDragHandle, uses_sheet,
};
use crate::primitives::text::{Divider, DividerAxis, Text};
use crate::theme::styles::buttons::{ButtonStyleViewExt, GhostButtonStyle};
use crate::theme::{ControlSize, ControlSizeViewExt, SemanticColorRef, SemanticFontRef};
use crate::{AnyView, BoxView, TupleView, View};
use cove_core::scene::{AccessibilityRole, Align, FlexDirection, Justify, Wrap};
use cove_core::{Binding, Environment, SizeClass};

/// Presentation knobs layered above `ModalOptions`.
#[derive(Clone)]
pub struct DialogOptions {
    pub modal: ModalOptions,
    pub description: String,
    pub shows_close_button: bool,
    /// Touch phones present a centered dialog as a bottom sheet.
    pub adapts_to_touch: bool,
}

impl Default for DialogOptions {
    fn default() -> Self {
        Self {
            modal: ModalOptions::default(),
            description: String::new(),
            shows_close_button: true,
            adapts_to_touch: true,
        }
    }
}

impl DialogOptions {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Structured modal with header, scrollable body and action footer.
pub struct Dialog {
    pub is_presented: Binding<bool>,
    pub title: String,
    pub options: DialogOptions,
    content: AnyView,
    footer: Option<AnyView>,
}

impl Dialog {
    pub fn new(
        is_presented: Binding<bool>,
        title: impl Into<String>,
        content: impl View + 'static,
    ) -> Self {
        Self {
            is_presented,
            title: title.into(),
            options: DialogOptions::default(),
            content: AnyView::new(content),
            footer: None,
        }
    }

    pub fn footer(mut self, footer: impl View + 'static) -> Self {
        self.footer = Some(AnyView::new(footer));
        self
    }

    pub fn options(mut self, options: DialogOptions) -> Self {
        self.options = options;
        self
    }

    pub fn configure(mut self, configure: impl FnOnce(&mut DialogOptions)) -> Self {
        configure(&mut self.options);
        self
    }
}

impl View for Dialog {
    fn body(&self) -> AnyView {
        let is_presented = self.is_presented.clone();
        let title = self.title.clone();
        let options = self.options.clone();
        let content = self.content.clone();
        let footer = self.footer.clone();
        AnyView::new(SurfaceReader::new(move |surface: &Environment| {
            let sheet = options.adapts_to_touch
                && options.modal.geometry.placement == ModalPlacement::Center
                && uses_sheet(surface);
            let fits = sheet || options.modal.fits_content_height;
            let safe = surface.safe_area.normalized();
            let bottom = if sheet {
                safe.bottom.max(surface.keyboard.height.max(0.0))
            } else {
                0.0
            };
            let compact = surface.viewport.width_class() == SizeClass::Compact;

            let mut modal_options = options.modal.clone();
            if sheet {
                modal_options.geometry.placement = ModalPlacement::Bottom;
                modal_options.geometry.inset = 0.0;
                modal_options.geometry.corner_radius = 18.0;
                modal_options.geometry.width = if compact { f32::MAX } else { 560.0 };
                modal_options.geometry.height =
                    (surface.viewport.height - safe.top - 12.0).max(64.0);
                modal_options.fits_content_height = true;
            }

            let mut column: Vec<AnyView> = Vec::new();

            if sheet {
                let dismiss = is_presented.clone();
                let closes = options.modal.dismissal.closes_on_backdrop;
                let handle = SheetDragHandle::new(
                    move |_| {},
                    move |distance, velocity| {
                        if !closes {
                            return;
                        }
                        if distance > 80.0 || (distance > 0.0 && velocity > 900.0) {
                            dismiss.set(false);
                        }
                    },
                );
                column.push(AnyView::new(handle.accessibility_hidden(true)));
            }

            let mut header: Vec<AnyView> = Vec::new();
            let mut titles: Vec<AnyView> = vec![AnyView::new(
                Text::new(title.clone())
                    .semantic_font(SemanticFontRef::headline())
                    .frame(None, None, Some(0.0), None, None, None),
            )];
            if !options.description.is_empty() {
                titles.push(AnyView::new(
                    Text::new(options.description.clone())
                        .semantic_font(SemanticFontRef::caption())
                        .semantic_foreground(SemanticColorRef::on_surface_muted()),
                ));
            }
            header.push(AnyView::new(
                AnyView::new(BoxView::new(
                    FlexDirection::Column,
                    Align::FlexStart,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    5.0,
                    AnyView::new(TupleView::new(titles)),
                ))
                .flex(1.0)
                .shrink(1.0),
            ));
            if options.shows_close_button && !sheet {
                let dismiss = is_presented.clone();
                header.push(AnyView::new(
                    Button::icon(
                        ButtonIconSource::resource(ui_icon_catalog::action::x()),
                        14.0,
                        move || dismiss.set(false),
                    )
                    .button_style(GhostButtonStyle)
                    .control_size(ControlSize::Small)
                    .accessibility_label("Close dialog"),
                ));
            }

            let header_insets = if sheet {
                EdgeInsets::new(0.0, 20.0, 4.0, 20.0)
            } else {
                EdgeInsets::all(18.0)
            };
            column.push(AnyView::new(
                AnyView::new(BoxView::new(
                    FlexDirection::Row,
                    Align::Center,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    12.0,
                    AnyView::new(TupleView::new(header)),
                ))
                .padding_insets(header_insets)
                .flex(0.0)
                .shrink(0.0),
            ));

            if !sheet {
                column.push(AnyView::new(Divider::new(
                    None,
                    1.0,
                    DividerAxis::Horizontal,
                )));
            }

            let body_insets = if sheet {
                EdgeInsets::new(8.0, 20.0, 8.0, 20.0)
            } else {
                EdgeInsets::all(18.0)
            };
            let scroller = ScrollView::new(
                ScrollAxis::Vertical,
                AnyView::new(
                    AnyView::new(content.clone())
                        .padding_insets(body_insets)
                        .flex(0.0)
                        .shrink(0.0),
                ),
            )
            .scrollbar_gutter(if sheet {
                ScrollbarGutter::Overlay
            } else {
                ScrollbarGutter::Stable
            });
            column.push(AnyView::new(
                AnyView::new(scroller)
                    .flex(if fits { 0.0 } else { 1.0 })
                    .shrink(1.0)
                    .frame(None, None, Some(0.0), None, None, None),
            ));

            if !sheet {
                column.push(AnyView::new(Divider::new(
                    None,
                    1.0,
                    DividerAxis::Horizontal,
                )));
            }

            if let Some(footer) = footer.clone() {
                let footer_insets = if sheet {
                    EdgeInsets::new(8.0, 20.0, bottom + 12.0, 20.0)
                } else {
                    EdgeInsets::all(14.0)
                };
                let group =
                    crate::LayoutContainer::<ActionGroupLayout, AnyView>::with_erased_content(
                        ActionGroupLayout {
                            stacks: compact,
                            alignment: crate::HorizontalAlignment::Trailing,
                            spacing: 8.0,
                        },
                        footer,
                    );
                column.push(AnyView::new(
                    AnyView::new(group)
                        .padding_insets(footer_insets)
                        .flex(0.0)
                        .shrink(0.0),
                ));
            }

            let label = title.clone();
            let help = options.description.clone();
            let body = AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(column)),
            ))
            .accessibility(move |semantics| {
                semantics.role = AccessibilityRole::Dialog;
                semantics.label = label.clone();
                semantics.help = help.clone();
            });

            AnyView::new(Modal::new(is_presented.clone(), body).options(modal_options))
        }))
    }
}

/// Presentation knobs for the confirm/cancel dialog.
#[derive(Clone, Debug, PartialEq)]
pub struct AlertDialogOptions {
    pub confirm_title: String,
    pub cancel_title: String,
    pub is_destructive: bool,
    pub is_confirming: bool,
    pub dismiss_after_confirm: bool,
}

impl Default for AlertDialogOptions {
    fn default() -> Self {
        Self {
            confirm_title: "Confirm".to_string(),
            cancel_title: "Cancel".to_string(),
            is_destructive: false,
            is_confirming: false,
            dismiss_after_confirm: true,
        }
    }
}

impl AlertDialogOptions {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Message plus confirm/cancel pair over `Dialog`.
pub struct AlertDialog {
    pub is_presented: Binding<bool>,
    pub title: String,
    pub message: String,
    pub on_confirm: Rc<dyn Fn()>,
    pub options: AlertDialogOptions,
}

impl AlertDialog {
    pub fn new(
        is_presented: Binding<bool>,
        title: impl Into<String>,
        message: impl Into<String>,
        on_confirm: impl Fn() + 'static,
    ) -> Self {
        Self {
            is_presented,
            title: title.into(),
            message: message.into(),
            on_confirm: Rc::new(on_confirm),
            options: AlertDialogOptions::default(),
        }
    }

    pub fn options(mut self, options: AlertDialogOptions) -> Self {
        self.options = options;
        self
    }
}

impl View for AlertDialog {
    fn body(&self) -> AnyView {
        let is_presented = self.is_presented.clone();
        let title = self.title.clone();
        let message = self.message.clone();
        let options = self.options.clone();
        let on_confirm = self.on_confirm.clone();
        AnyView::new(crate::theme::TextScalerReader::new(move |scaler| {
            let close_cancel = is_presented.clone();
            let close_confirm = is_presented.clone();
            let confirm = on_confirm.clone();
            let confirming = options.is_confirming;
            let destructive = options.is_destructive;
            let dismiss_after = options.dismiss_after_confirm;

            let cancel = Button::title(options.cancel_title.clone(), move || {
                close_cancel.set(false)
            })
            .enabled(!confirming)
            .button_style(crate::theme::styles::buttons::SecondaryButtonStyle);
            let confirm_button = Button::title(options.confirm_title.clone(), move || {
                if !close_confirm.get() || confirming {
                    return;
                }
                confirm();
                if dismiss_after {
                    close_confirm.set(false);
                }
            })
            .role(if destructive {
                ButtonRole::Destructive
            } else {
                ButtonRole::Normal
            })
            .loading(confirming)
            .button_style_any(if destructive {
                crate::theme::styles::buttons::AnyButtonStyle::new(
                    crate::theme::styles::buttons::DestructiveButtonStyle,
                )
            } else {
                crate::theme::styles::buttons::AnyButtonStyle::new(
                    crate::theme::styles::buttons::PrimaryButtonStyle,
                )
            });

            let mut dialog = Dialog::new(
                is_presented.clone(),
                title.clone(),
                AnyView::new(
                    Text::new(message.clone())
                        .semantic_font(SemanticFontRef::body())
                        .semantic_foreground(SemanticColorRef::on_surface_variant()),
                ),
            )
            .footer(AnyView::new(TupleView::new(vec![
                AnyView::new(cancel),
                AnyView::new(confirm_button),
            ])));

            let mut dialog_options = DialogOptions::default();
            dialog_options.modal.geometry = ModalGeometry {
                width: 420.0,
                height: (scaler.scale(200.0) + 80.0).max(280.0),
                ..ModalGeometry::default()
            };
            dialog_options.modal.fits_content_height = true;
            dialog_options.modal.dismissal = ModalDismissal {
                closes_on_backdrop: !options.is_confirming,
                closes_on_escape: !options.is_confirming,
            };
            dialog_options.shows_close_button = false;
            dialog_options.adapts_to_touch = false;
            dialog = dialog.options(dialog_options);
            AnyView::new(dialog)
        }))
    }
}

/// Keeps `StatusTone` reachable for callers that tint a dialog footer.
pub fn alert_dialog_tone(options: &AlertDialogOptions) -> StatusTone {
    if options.is_destructive {
        StatusTone::Danger
    } else {
        StatusTone::Info
    }
}
