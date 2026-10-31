//! Translation of `Primitives/Controls/WheelPicker.swift` (the option model).
//!
//! A five-row wheel. Pointer drags follow the finger, wheel and arrow keys move
//! one option, and the selection lands only on enabled options.
//!
//! Behaviour checklist:
//! - an option is identified by its value, not its label
//! - a disabled option stays visible but cannot be committed
//! - stepping skips disabled rows in either direction
//! - stepping stops at the ends rather than wrapping
//! - an empty wheel has no selection

/// One row of a wheel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WheelPickerOption<Value> {
    pub value: Value,
    pub label: String,
    pub is_enabled: bool,
}

impl<Value: PartialEq + Clone> WheelPickerOption<Value> {
    pub fn new(value: Value, label: impl Into<String>) -> Self {
        Self {
            value,
            label: label.into(),
            is_enabled: true,
        }
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }
}

/// The rows of one wheel.
#[derive(Clone, Debug, PartialEq)]
pub struct WheelPicker<Value> {
    pub options: Vec<WheelPickerOption<Value>>,
    value: Option<Value>,
}

impl<Value: PartialEq + Clone> WheelPicker<Value> {
    /// A wheel starting on `value`, or on the first enabled row when `value` is
    /// not present or is disabled.
    pub fn new(options: Vec<WheelPickerOption<Value>>, value: Option<Value>) -> Self {
        let mut picker = Self {
            options,
            value: None,
        };
        let landing = value
            .filter(|candidate| {
                picker
                    .index_of(candidate)
                    .is_some_and(|index| picker.options[index].is_enabled)
            })
            .or_else(|| picker.first_enabled().cloned());
        picker.value = landing;
        picker
    }

    pub fn value(&self) -> Option<&Value> {
        self.value.as_ref()
    }

    pub fn index_of(&self, value: &Value) -> Option<usize> {
        self.options
            .iter()
            .position(|option| &option.value == value)
    }

    fn first_enabled(&self) -> Option<&Value> {
        self.options
            .iter()
            .find(|option| option.is_enabled)
            .map(|option| &option.value)
    }

    /// Whether a row may be committed.
    pub fn is_selectable(&self, index: usize) -> bool {
        self.options
            .get(index)
            .is_some_and(|option| option.is_enabled)
    }

    /// Moves the selection by `delta` rows, skipping disabled ones and stopping
    /// at the ends.
    ///
    /// Returns whether the selection moved.
    pub fn step(&mut self, delta: i32) -> bool {
        let Some(current) = self.value.as_ref().and_then(|value| self.index_of(value)) else {
            return false;
        };
        let step: i64 = if delta < 0 { -1 } else { 1 };
        let mut index = current as i64 + step;
        while (0..self.options.len() as i64).contains(&index) {
            if self.options[index as usize].is_enabled {
                self.value = Some(self.options[index as usize].value.clone());
                return true;
            }
            index += step;
        }
        // A delta of zero, or a wall of disabled rows, leaves the selection alone.
        false
    }

    /// Jumps to the first enabled row.
    pub fn select_first(&mut self) -> bool {
        match self.first_enabled() {
            Some(value) => {
                let changed = self.value.as_ref() != Some(value);
                self.value = Some(value.clone());
                changed
            }
            None => false,
        }
    }

    /// Jumps to the last enabled row.
    pub fn select_last(&mut self) -> bool {
        let landing = self
            .options
            .iter()
            .rev()
            .find(|option| option.is_enabled)
            .map(|option| option.value.clone());
        match landing {
            Some(value) => {
                let changed = self.value.as_ref() != Some(&value);
                self.value = Some(value);
                changed
            }
            None => false,
        }
    }

    /// The label of the committed row, which is what a badge shows.
    pub fn selected_label(&self) -> Option<&str> {
        self.value
            .as_ref()
            .and_then(|value| self.index_of(value))
            .map(|index| self.options[index].label.as_str())
    }

    /// The rows visible around the selection, at most `count` of them, centred
    /// on the committed value where the ends allow.
    pub fn visible_window(&self, count: usize) -> std::ops::Range<usize> {
        if count == 0 || self.options.is_empty() {
            return 0..0;
        }
        let Some(current) = self.value.as_ref().and_then(|value| self.index_of(value)) else {
            return 0..count.min(self.options.len());
        };
        let total = self.options.len();
        let half = count / 2;
        let mut start = current.saturating_sub(half);
        // Slide the whole window back when it runs past the end, so it stays full
        // rather than shrinking near the last row.
        if start + count > total {
            start = total.saturating_sub(count);
        }
        start..(start + count).min(total)
    }
}
