//! Localized rendering: the embedded catalogs reach the composed scene.
use crate::support;

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::AnyView;
use cove_core::scene::{AccessibilityRole, Node};
use cove_ui_compose::foundation::{Calendar, DateParts};
use cove_ui_compose::i18n;
use cove_ui_compose::primitives::controls::calendar::calendar::{CalendarOptions, CalendarView};
use cove_ui_compose::primitives::controls::calendar::period::CalendarGranularity;
use cove_ui_compose::primitives::controls::date_values::DateAvailability;

use support::InteractiveScene;

/// Mounts a March 2024 calendar at `granularity` under `locale`.
fn render(
    locale: &str,
    granularity: CalendarGranularity,
    test: impl FnOnce(&InteractiveScene),
) {
    i18n::set_locale_override(Some(locale));
    InteractiveScene::run(720.0, 900.0, |scene| {
        let mut calendar = Calendar::new();
        calendar.set_first_weekday(0);
        let selection = calendar.date(DateParts::new(2024, 3, 13));
        let mut options = CalendarOptions::default();
        options.calendar = calendar;
        options.granularity = granularity;
        options.availability = DateAvailability::default();
        options.today = selection;
        let store = Rc::new(RefCell::new(Some(selection)));
        let read = store.clone();
        let write = store.clone();
        scene.install(AnyView::new(
            CalendarView::new(cove_core::Binding::new(
                move || *read.borrow(),
                move |value| *write.borrow_mut() = value,
            ))
            .configure(|calendar| *calendar = options),
        ));
        test(scene);
    });
    i18n::set_locale_override(None);
}

fn labels(scene: &InteractiveScene) -> Vec<String> {
    scene
        .all(|node| {
            node.accessibility().is_some_and(|semantics| {
                semantics.role == AccessibilityRole::Button && !semantics.is_hidden
            })
        })
        .into_iter()
        .filter_map(|node: Node| node.accessibility().map(|semantics| semantics.label))
        .collect()
}

fn texts(scene: &InteractiveScene) -> Vec<String> {
    scene
        .all(|node| {
            node.layout_node()
                .is_some_and(|layout| layout.text_inputs().is_some())
        })
        .into_iter()
        .filter_map(|node: Node| {
            node.layout_node()
                .and_then(|layout| layout.text_inputs())
                .map(|inputs| inputs.text.to_string())
        })
        .collect()
}

#[test]
fn the_english_catalog_drives_the_month_grid() {
    render("en", CalendarGranularity::Month, |scene| {
        let labels = labels(scene);
        assert!(
            labels.iter().any(|label| label == "March 2024"),
            "the header reads the English month, saw {labels:?}"
        );
    });
    render("en", CalendarGranularity::Day, |scene| {
        let texts = texts(scene);
        for symbol in ["S", "M", "T", "W", "F"] {
            assert!(
                texts.iter().any(|text| text == symbol),
                "the weekday row carries {symbol:?}, saw {texts:?}"
            );
        }
    });
}

#[test]
fn the_chinese_catalog_drives_the_month_grid() {
    render("zh-Hans", CalendarGranularity::Month, |scene| {
        let labels = labels(scene);
        assert!(
            labels.iter().any(|label| label == "2024年3月"),
            "the header reads the Chinese month, saw {labels:?}"
        );
        assert!(
            !labels.iter().any(|label| label.contains("March")),
            "no English month survives, saw {labels:?}"
        );
    });
    render("zh-Hans", CalendarGranularity::Day, |scene| {
        let texts = texts(scene);
        for symbol in ["日", "一", "二", "三", "四", "五", "六"] {
            assert!(
                texts.iter().any(|text| text == symbol),
                "the weekday row carries {symbol:?}, saw {texts:?}"
            );
        }
        assert!(
            !texts.iter().any(|text| text == "S"),
            "no English weekday symbol survives, saw {texts:?}"
        );
    });
}
