//! Translation of `Tests/CoveUIComposeTests/SelectEnumerationTests.swift`.
//!
//! Behaviour checklist:
//! - CaseIterable values become every option, in authored order, with caller labels
//! - every option is enabled and the default placeholder is "Select"
//! - custom labels and an explicit width are preserved

use cove_core::Binding;
use cove_ui_compose::primitives::controls::fields::select::{Select, SelectOption};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Trigger {
    None,
    Death,
    Collision,
}

impl Trigger {
    fn all() -> [Self; 3] {
        [Self::None, Self::Death, Self::Collision]
    }

    fn default_label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Death => "death",
            Self::Collision => "collision",
        }
    }
}

#[test]
fn cases_become_options() {
    let plain_selection = Binding::constant(Trigger::None);
    let plain_options: Vec<SelectOption<Trigger>> = Trigger::all()
        .iter()
        .map(|value| SelectOption::new(*value, value.default_label(), true))
        .collect();
    let plain = Select::new(plain_selection, plain_options);

    let labels: Vec<&str> = plain
        .options
        .iter()
        .map(|option| option.label.as_str())
        .collect();
    assert_eq!(labels, ["none", "death", "collision"]);
    assert!(plain.options.iter().all(|option| option.is_enabled));
    assert_eq!(plain.placeholder, "Select");

    let labeled_selection = Binding::constant(Trigger::Death);
    let labeled_options: Vec<SelectOption<Trigger>> = Trigger::all()
        .iter()
        .map(|value| {
            let label = match value {
                Trigger::None => "None",
                Trigger::Death => "Death",
                Trigger::Collision => "Collision",
            };
            SelectOption::new(*value, label, true)
        })
        .collect();
    let labeled = Select::new(labeled_selection, labeled_options).width(112.0);

    let labels: Vec<&str> = labeled
        .options
        .iter()
        .map(|option| option.label.as_str())
        .collect();
    assert_eq!(labels, ["None", "Death", "Collision"]);
    let values: Vec<Trigger> = labeled.options.iter().map(|option| option.value).collect();
    assert_eq!(values, Trigger::all().to_vec());
    assert_eq!(labeled.width, Some(112.0));
}
