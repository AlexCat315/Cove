//! One import for ordinary view code.
//!
//! `use cove_ui_compose::prelude::*;` brings the components, the layout and
//! style modifiers, and the scene vocabulary a view body normally needs. It is
//! assembled from the public component exports of each group.
//!
//! ```
//! use cove_ui_compose::prelude::*;
//!
//! let date = State::new(None::<Date>);
//! let picker = DatePicker::new(date.binding());
//! let stack = ZStack::new(picker).alignment(Alignment::TopLeading);
//! let spinner = Spinner::new().size(24.0);
//! let image = Image::from_texture(7, Size { width: 32.0, height: 20.0 })
//!     .content_mode(ImageContentMode::Fit);
//! let tabs = TabView::try_new(Binding::constant(1), vec![TabItem::new(1, "Date", stack)])?;
//! # Ok::<(), DuplicateId>(())
//! ```
#![deny(ambiguous_glob_reexports)]

pub use crate::primitives::DuplicateId;
pub use crate::primitives::controls::api::*;
pub use crate::primitives::data::api::*;
pub use crate::primitives::feedback::*;
pub use crate::primitives::interaction::api::*;
pub use crate::primitives::layout::api::*;
pub use crate::primitives::media::api::*;
pub use crate::primitives::navigation::api::*;
pub use crate::primitives::overlays::api::*;
pub use crate::primitives::text::api::*;

pub use crate::foundation::Date;
pub use crate::modifiers::layout::{
    FlexExt, FrameDimension, GridCell, GridExt, GridSpec, LayoutFrameExt, PaddingExt,
};
pub use crate::modifiers::style::SemanticStyleViewExt;
pub use crate::theme::{ControlSize, ControlSizeViewExt, SemanticColorRef, SemanticFontRef};
pub use cove_composition::LayoutDebugExt;

pub use crate::{Alignment, AnyView, BoxView, Column, Row, TupleView, View, ViewExt};
pub use cove_composition::Scope;
pub use cove_core::scene::{
    Align, Direction, FlexDirection, GridAreaTemplate, GridAutoFlow, GridBreadth, GridPlacement,
    GridRepeat, GridTemplateTrack, GridTrack, Justify, Point, Rect, Size, Wrap,
};
pub use cove_core::{Binding, Color, Signal, State};
