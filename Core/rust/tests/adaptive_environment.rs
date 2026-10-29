//! Translation of CoveUICoreTests/AdaptiveEnvironmentTests.swift.

use cove_core::{
    Environment, Idiom, InputCapabilities, Insets, Keyboard, SizeClass, TextScale, Viewport,
};

#[test]
fn surface_adapts_to_space_and_input_independently() {
    let mut environment = Environment::default();
    assert_eq!(environment.viewport.width_class(), SizeClass::Expanded);
    environment.viewport.width = 390.0;
    assert_eq!(environment.viewport.width_class(), SizeClass::Compact);
    environment.viewport.width = 768.0;
    environment.input = InputCapabilities::TOUCH | InputCapabilities::KEYBOARD;
    assert_eq!(environment.viewport.width_class(), SizeClass::Medium);
    assert_eq!(environment.minimum_control_extent(), 44.0);
    assert!(!environment.supports_hover());
    environment.input = InputCapabilities::POINTER | InputCapabilities::KEYBOARD;
    assert_eq!(environment.minimum_control_extent(), 28.0);
    assert!(environment.supports_hover());
    environment.input = InputCapabilities::TOUCH | InputCapabilities::POINTER;
    assert_eq!(environment.minimum_control_extent(), 44.0);
    assert!(environment.supports_hover());
}

#[test]
fn idiom_is_a_host_fact_rather_than_a_width_guess() {
    let mut environment = Environment::default();
    assert_eq!(environment.idiom, Idiom::Desktop);
    environment.viewport.width = 390.0;
    assert_eq!(environment.idiom, Idiom::Desktop);
    environment.idiom = Idiom::Phone;
    environment.viewport.width = 932.0;
    assert_eq!(environment.idiom, Idiom::Phone);
    environment.idiom = Idiom::Tablet;
    assert_ne!(environment.idiom, Idiom::Phone);
    assert_eq!(
        [Idiom::Phone, Idiom::Tablet, Idiom::Desktop],
        [Idiom::Phone, Idiom::Tablet, Idiom::Desktop]
    );
}

#[test]
fn text_scale_and_keyboard_stay_separate_from_the_safe_area() {
    let mut environment = Environment::default();
    assert_eq!(environment.text_scale.factor(), 1.0);
    assert!(!environment.keyboard.is_visible());
    environment.safe_area.bottom = 34.0;
    environment.keyboard = Keyboard::new(336.0);
    environment.text_scale = TextScale::linear(1.3);
    assert!(environment.keyboard.is_visible());
    assert_eq!(environment.safe_area.bottom, 34.0);
    assert_eq!(environment.text_scale.factor(), 1.3);
}

#[test]
fn defaults_match_the_portable_surface() {
    assert_eq!(
        Viewport::default(),
        Viewport {
            width: 1280.0,
            height: 720.0,
            scale: 1.0
        }
    );
    assert_eq!(Insets::default(), Insets::default());
}
