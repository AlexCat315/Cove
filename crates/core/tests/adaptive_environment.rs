//! Translation of CoveUICoreTests/AdaptiveEnvironmentTests.swift.

use cove_core::{
    DesignLanguage, DeviceClass, DevicePreset, DisplayFeature, Environment, EnvironmentOverride,
    FeatureOrientation, FoldableKind, HeightClass, InputCapabilities, Insets, Keyboard, Platform,
    Posture, Rect, TextScale, Viewport, WidthClass,
};

#[test]
fn surface_adapts_to_space_and_input_independently() {
    let mut environment = Environment {
        design_language: DesignLanguage::Cupertino,
        ..Environment::default()
    };
    assert_eq!(environment.width_class(), WidthClass::Large);
    environment.viewport.width = 390.0;
    assert_eq!(environment.width_class(), WidthClass::Compact);
    environment.viewport.width = 768.0;
    environment.input = InputCapabilities::TOUCH | InputCapabilities::KEYBOARD;
    assert_eq!(environment.width_class(), WidthClass::Medium);
    assert_eq!(environment.minimum_control_extent(), 44.0);
    environment.design_language = DesignLanguage::Material;
    assert_eq!(environment.minimum_control_extent(), 48.0);
    environment.design_language = DesignLanguage::Cupertino;
    assert!(!environment.supports_hover());
    environment.input = InputCapabilities::POINTER | InputCapabilities::KEYBOARD;
    assert_eq!(environment.minimum_control_extent(), 28.0);
    assert!(environment.supports_hover());
    environment.input = InputCapabilities::TOUCH | InputCapabilities::POINTER;
    assert_eq!(environment.minimum_control_extent(), 44.0);
    assert!(environment.supports_hover());
}

#[test]
fn width_classes_follow_material_breakpoints() {
    let class = |width| {
        Viewport {
            width,
            height: 800.0,
            scale: 1.0,
        }
        .width_class()
    };
    assert_eq!(class(599.0), WidthClass::Compact);
    assert_eq!(class(600.0), WidthClass::Medium);
    assert_eq!(class(839.0), WidthClass::Medium);
    assert_eq!(class(840.0), WidthClass::Expanded);
    assert_eq!(class(1199.0), WidthClass::Expanded);
    assert_eq!(class(1200.0), WidthClass::Large);
    assert_eq!(class(1600.0), WidthClass::ExtraLarge);
    assert!(WidthClass::Compact < WidthClass::Expanded);
    let height = |height| {
        Viewport {
            width: 800.0,
            height,
            scale: 1.0,
        }
        .height_class()
    };
    assert_eq!(height(479.0), HeightClass::Compact);
    assert_eq!(height(480.0), HeightClass::Medium);
    assert_eq!(height(900.0), HeightClass::Expanded);
}

#[test]
fn device_class_is_a_host_fact_rather_than_a_width_guess() {
    let mut environment = Environment::default();
    assert_eq!(environment.device_class, DeviceClass::Desktop);
    environment.viewport.width = 390.0;
    assert_eq!(environment.device_class, DeviceClass::Desktop);
    environment.device_class = DeviceClass::Foldable(FoldableKind::Book);
    environment.viewport.width = 841.0;
    assert!(environment.device_class.is_handheld());
    assert_eq!(environment.width_class(), WidthClass::Expanded);
    assert!(!DeviceClass::Tablet.is_handheld());
}

#[test]
fn platform_implies_its_design_language() {
    assert_eq!(Platform::IOS.design_language(), DesignLanguage::Cupertino);
    assert_eq!(
        Platform::Android.design_language(),
        DesignLanguage::Material
    );
    assert_eq!(Platform::Windows.design_language(), DesignLanguage::Fluent);
    assert_eq!(Platform::Linux.design_language(), DesignLanguage::Adwaita);
    for platform in Platform::ALL {
        assert_eq!(Platform::parse(platform.name()), Some(platform));
    }
    let mut environment = Environment::default();
    EnvironmentOverride {
        platform: Some(Platform::Android),
        ..EnvironmentOverride::default()
    }
    .apply(&mut environment);
    assert_eq!(environment.design_language, DesignLanguage::Material);
    EnvironmentOverride {
        platform: Some(Platform::IOS),
        design_language: Some(DesignLanguage::Material),
        ..EnvironmentOverride::default()
    }
    .apply(&mut environment);
    assert_eq!(environment.platform, Platform::IOS);
    assert_eq!(environment.design_language, DesignLanguage::Material);
}

fn sized(width: f32, height: f32, features: Vec<DisplayFeature>) -> Environment {
    Environment {
        viewport: Viewport {
            width,
            height,
            scale: 1.0,
        },
        display_features: features,
        ..Environment::default()
    }
}

#[test]
fn a_flat_crease_does_not_divide_the_window() {
    let environment = sized(
        841.0,
        701.0,
        vec![DisplayFeature::fold(
            Rect::new(420.5, 0.0, 0.0, 701.0),
            FeatureOrientation::Vertical,
            false,
        )],
    );
    assert_eq!(
        environment.segments(),
        vec![Rect::new(0.0, 0.0, 841.0, 701.0)]
    );
    assert!(!environment.is_spanned());
}

#[test]
fn a_half_open_fold_divides_without_removing_space() {
    let environment = sized(
        701.0,
        841.0,
        vec![DisplayFeature::fold(
            Rect::new(0.0, 420.5, 701.0, 0.0),
            FeatureOrientation::Horizontal,
            true,
        )],
    );
    assert_eq!(
        environment.segments(),
        vec![
            Rect::new(0.0, 0.0, 701.0, 420.5),
            Rect::new(0.0, 420.5, 701.0, 420.5)
        ]
    );
}

#[test]
fn an_occluding_hinge_removes_its_area() {
    let environment = sized(
        1114.0,
        705.0,
        vec![DisplayFeature::hinge(
            Rect::new(540.0, 0.0, 34.0, 705.0),
            FeatureOrientation::Vertical,
        )],
    );
    let segments = environment.segments();
    assert_eq!(
        segments,
        vec![
            Rect::new(0.0, 0.0, 540.0, 705.0),
            Rect::new(574.0, 0.0, 540.0, 705.0)
        ]
    );
    assert_eq!(
        environment.occluded_regions().collect::<Vec<_>>(),
        vec![Rect::new(540.0, 0.0, 34.0, 705.0)]
    );
    // A dialog centered on the hinge moves into one segment.
    let dialog = Rect::new(407.0, 200.0, 300.0, 200.0);
    let host = environment.segment_for(dialog);
    assert_eq!(host, segments[0]);
    assert_eq!(
        cove_core::display_features::confine(dialog, host),
        Rect::new(240.0, 200.0, 300.0, 200.0)
    );
    assert_eq!(
        environment.segment_for(Rect::new(800.0, 10.0, 10.0, 10.0)),
        segments[1]
    );
}

#[test]
fn presets_describe_complete_devices() {
    for name in DevicePreset::NAMES {
        let preset = DevicePreset::named(name).expect(name);
        assert_eq!(preset.name, name);
        let mut environment = sized(preset.size.width, preset.size.height, vec![]);
        preset.environment.apply(&mut environment);
        assert!(environment.platform == preset.environment.platform.unwrap());
    }
    let mut environment = sized(841.0, 701.0, vec![]);
    DevicePreset::named("fold-book")
        .unwrap()
        .environment
        .apply(&mut environment);
    assert_eq!(environment.posture, Posture::Book);
    assert_eq!(environment.segments().len(), 2);
    assert_eq!(environment.minimum_control_extent(), 48.0);
    assert!(DevicePreset::named("unknown").is_none());
}

#[test]
fn overrides_raise_the_safe_area_and_round_trip_as_json() {
    let mut environment = Environment::default();
    environment.safe_area.top = 70.0;
    let value = EnvironmentOverride {
        device_class: Some(DeviceClass::Foldable(FoldableKind::DualScreen)),
        minimum_safe_area: Some(Insets {
            top: 59.0,
            bottom: 34.0,
            ..Insets::default()
        }),
        ..EnvironmentOverride::default()
    };
    value.apply(&mut environment);
    assert_eq!(environment.safe_area.top, 70.0);
    assert_eq!(environment.safe_area.bottom, 34.0);
    let json = serde_json::to_string(&value).unwrap();
    let decoded: EnvironmentOverride = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, value);
    let parsed: EnvironmentOverride =
        serde_json::from_str(r#"{"platform":"ipados","posture":"tabletop"}"#).unwrap();
    assert_eq!(parsed.platform, Some(Platform::IPadOS));
    assert_eq!(parsed.posture, Some(Posture::Tabletop));
    assert!(EnvironmentOverride::default().is_empty());
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

#[test]
fn presentations_choose_the_controls_or_leading_segment() {
    use cove_core::display_features::presentation_segment;
    let left = Rect::new(0.0, 0.0, 540.0, 705.0);
    let right = Rect::new(574.0, 0.0, 540.0, 705.0);
    assert_eq!(presentation_segment(&[left, right], false), Some(left));
    assert_eq!(presentation_segment(&[left, right], true), Some(right));
    let top = Rect::new(0.0, 0.0, 701.0, 420.5);
    let bottom = Rect::new(0.0, 420.5, 701.0, 420.5);
    assert_eq!(presentation_segment(&[top, bottom], false), Some(bottom));
    assert_eq!(presentation_segment(&[top, bottom], true), Some(bottom));
    assert_eq!(presentation_segment(&[left], true), Some(left));
    assert_eq!(presentation_segment(&[], false), None);
}
