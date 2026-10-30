//! The authored `TextFieldNavigation` value, mirroring the same type in
//! Primitives/Text/TextField/TextFieldOptions.swift.
//!
//! One-shot navigation requests, separate from the live caret/selection state.
//! The Rust port has always spelled the same payload
//! `TextFieldNavigationRequest`; that name is kept for existing call sites and
//! this is the reference spelling over the identical value.

pub use crate::primitives::text::text_field::text_field_options::TextFieldNavigationRequest as TextFieldNavigation;
