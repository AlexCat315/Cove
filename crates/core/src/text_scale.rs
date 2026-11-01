#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextScaleSample {
    pub size: f32,
    pub scaled_size: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextScale {
    samples: Vec<TextScaleSample>,
}
impl Default for TextScale {
    fn default() -> Self {
        Self::linear(1.0)
    }
}
impl TextScale {
    pub fn linear(factor: f32) -> Self {
        Self {
            samples: vec![TextScaleSample {
                size: 1.0,
                scaled_size: if factor.is_finite() && factor > 0.0 {
                    factor
                } else {
                    1.0
                },
            }],
        }
    }
    pub fn from_samples(samples: impl IntoIterator<Item = TextScaleSample>) -> Self {
        let mut samples: Vec<_> = samples
            .into_iter()
            .filter(|s| {
                s.size.is_finite()
                    && s.size > 0.0
                    && s.scaled_size.is_finite()
                    && s.scaled_size > 0.0
            })
            .collect();
        samples.sort_by(|a, b| a.size.total_cmp(&b.size));
        if samples.is_empty()
            || samples
                .windows(2)
                .any(|s| s[0].size >= s[1].size || s[0].scaled_size > s[1].scaled_size)
        {
            return Self::default();
        }
        Self { samples }
    }
    pub fn factor(&self) -> f32 {
        self.scale(16.0) / 16.0
    }
    pub fn scale(&self, size: f32) -> f32 {
        if !size.is_finite() {
            return size;
        }
        if size < 0.0 {
            return -self.scale(-size);
        }
        let first = self.samples[0];
        if size <= first.size {
            return size * first.scaled_size / first.size;
        }
        for samples in self.samples.windows(2) {
            let (lower, upper) = (samples[0], samples[1]);
            if size <= upper.size {
                return lower.scaled_size
                    + (upper.scaled_size - lower.scaled_size) * (size - lower.size)
                        / (upper.size - lower.size);
            }
        }
        let last = self.samples.last().unwrap();
        size * last.scaled_size / last.size
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nonlinear_accessibility_ramps_and_invalid_samples() {
        let scale = TextScale::from_samples([
            TextScaleSample {
                size: 10.0,
                scaled_size: 15.0,
            },
            TextScaleSample {
                size: 20.0,
                scaled_size: 40.0,
            },
        ]);
        assert_eq!(scale.scale(5.0), 7.5);
        assert_eq!(scale.scale(15.0), 27.5);
        assert_eq!(scale.scale(-30.0), -60.0);
        assert!(scale.scale(f32::NAN).is_nan());
        assert_eq!(TextScale::linear(-2.0), TextScale::default());
        assert_eq!(
            TextScale::from_samples([
                TextScaleSample {
                    size: 1.0,
                    scaled_size: 2.0
                },
                TextScaleSample {
                    size: 1.0,
                    scaled_size: 3.0
                }
            ]),
            TextScale::default()
        );
    }
}
