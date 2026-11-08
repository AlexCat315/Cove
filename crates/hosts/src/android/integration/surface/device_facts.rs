//! Device class, posture and folds reported by Jetpack WindowManager.
use cove_core::{
    DeviceClass, DisplayFeature, DisplayFeatureKind, FeatureOrientation, FoldableKind, Posture,
    Rect,
};

/// Floats per display feature crossing JNI:
/// `kind, x, y, width, height, orientation, occludes, separates`.
pub const FEATURE_STRIDE: usize = 8;
/// Bounds the JNI array so a corrupt length cannot allocate unboundedly.
pub const MAXIMUM_FEATURES: usize = 8;

#[derive(Clone, Debug, PartialEq)]
pub struct DeviceFacts {
    pub device_class: DeviceClass,
    pub posture: Posture,
    pub display_features: Vec<DisplayFeature>,
}

impl DeviceFacts {
    pub fn phone() -> Self {
        Self::flat(DeviceClass::Phone)
    }

    pub fn tablet() -> Self {
        Self::flat(DeviceClass::Tablet)
    }

    pub fn flat(device_class: DeviceClass) -> Self {
        Self {
            device_class,
            posture: Posture::Flat,
            display_features: Vec::new(),
        }
    }

    /// Decodes the JNI encoding: device `0` phone, `1` tablet, `2` book
    /// foldable, `3` flip, `4` dual screen; posture `0` flat, `1` tabletop,
    /// `2` book; features in logical window points.
    pub fn decode(device: i32, posture: i32, features: &[f32]) -> Self {
        let device_class = match device {
            1 => DeviceClass::Tablet,
            2 => DeviceClass::Foldable(FoldableKind::Book),
            3 => DeviceClass::Foldable(FoldableKind::Flip),
            4 => DeviceClass::Foldable(FoldableKind::DualScreen),
            _ => DeviceClass::Phone,
        };
        let posture = match posture {
            1 => Posture::Tabletop,
            2 => Posture::Book,
            _ => Posture::Flat,
        };
        let display_features = features
            .as_chunks::<FEATURE_STRIDE>()
            .0
            .iter()
            .take(MAXIMUM_FEATURES)
            .filter_map(decode_feature)
            .collect();
        Self {
            device_class,
            posture,
            display_features,
        }
    }
}

fn decode_feature(values: &[f32; FEATURE_STRIDE]) -> Option<DisplayFeature> {
    let kind = match values[0] as i32 {
        0 => DisplayFeatureKind::Fold,
        1 => DisplayFeatureKind::Hinge,
        _ => return None,
    };
    let orientation = if values[5] as i32 == 0 {
        FeatureOrientation::Horizontal
    } else {
        FeatureOrientation::Vertical
    };
    DisplayFeature {
        kind,
        bounds: Rect::new(values[1], values[2], values[3], values[4]),
        orientation,
        occludes: values[6] != 0.0,
        separates: values[7] != 0.0,
    }
    .normalized()
}
