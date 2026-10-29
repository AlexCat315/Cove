//! Translation of CoveUICoreTests/TextScaleTests.swift.

use cove_core::{Insets, Keyboard, TextScale, TextScaleSample};

#[test]
fn native_text_scale_interpolates_the_system_ramp() {
    let scale = TextScale::from_samples([
        TextScaleSample {
            size: 16.0,
            scaled_size: 32.0,
        },
        TextScaleSample {
            size: 32.0,
            scaled_size: 48.0,
        },
    ]);
    assert_eq!(scale.factor(), 2.0);
    assert_eq!(scale.scale(16.0), 32.0);
    assert_eq!(scale.scale(24.0), 40.0);
    assert_eq!(scale.scale(8.0), 16.0);
    assert_eq!(scale.scale(64.0), 96.0);
    assert_eq!(scale.scale(-24.0), -40.0);
    assert_eq!(scale.scale(0.0), 0.0);
}

#[test]
fn invalid_text_scale_and_insets_have_deterministic_defaults() {
    for factor in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 0.0, -1.0] {
        assert_eq!(TextScale::linear(factor).scale(16.0), 16.0);
    }
    for samples in [
        vec![],
        vec![TextScaleSample {
            size: f32::NAN,
            scaled_size: 1.0,
        }],
        vec![
            TextScaleSample {
                size: 16.0,
                scaled_size: 32.0,
            },
            TextScaleSample {
                size: 16.0,
                scaled_size: 48.0,
            },
        ],
        vec![
            TextScaleSample {
                size: 16.0,
                scaled_size: 32.0,
            },
            TextScaleSample {
                size: 32.0,
                scaled_size: 16.0,
            },
        ],
    ] {
        assert_eq!(TextScale::from_samples(samples).factor(), 1.0);
    }
    let safe = Insets {
        top: f32::NAN,
        leading: f32::NEG_INFINITY,
        bottom: -10.0,
        trailing: 14.0,
    }
    .normalized();
    assert_eq!(safe.top, 0.0);
    assert_eq!(safe.leading, 0.0);
    assert_eq!(safe.bottom, 0.0);
    assert_eq!(safe.trailing, 14.0);
    assert_eq!(Keyboard::new(f32::INFINITY).height, 0.0);
}
