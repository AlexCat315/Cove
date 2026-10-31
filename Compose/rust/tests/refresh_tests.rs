//! Translation of `Tests/CoveUIComposeTests/RefreshTests.swift`.
//!
//! Behaviour checklist:
//! - the shared pull model only triggers at or past the threshold, and only while
//!   it is not already refreshing
//! - `is_refreshing` suppresses a second trigger so a held pull fires once
//! - the label is authored by the caller and defaults to none
//!
//! Named differences (production gap, recorded rather than fixed): none of the
//! three Swift cases can run as written, because nothing in `Compose/rust`
//! reaches the refresh model.
//! - `RefreshOptions::should_trigger` and `RefreshOptions::trigger` have no call
//!   sites in the crate (only an `Examples` unit test), and `List` stores the
//!   options without ever consulting them. So neither a touch pull nor Cmd+R can
//!   invoke `on_refresh`. `list_never_consults_its_refresh_options` pins that.
//! - `ScrollView` publishes no `refresh-indicator` debug name, so the Swift
//!   indicator-visibility probe has no target.
//! - `RefreshOptions::is_refreshing` is a plain `bool`, not a `Binding<bool>` as in
//!   Swift, so a caller cannot observe or drive the in-flight state from
//!   `@State`; the model tests below exercise the same rules through the value
//!   API that does exist.

mod support;

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::AnyView;
use cove_core::scene::Node;
use cove_core::{Binding, Environment, InputCapabilities};
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::data::lists::{CollectionSelection, List, RefreshOptions};

use support::GlobalTestLock;

fn counting(start: Rc<RefCell<i32>>) -> (RefreshOptions, Rc<RefCell<i32>>) {
    let counter = start.clone();
    (RefreshOptions::new(move || *counter.borrow_mut() += 1), start)
}

#[test]
fn the_pull_only_triggers_at_the_threshold_and_only_once() {
    GlobalTestLock::locked(|| {
        let (options, refreshes) = counting(Rc::new(RefCell::new(0)));

        assert!(
            !options.should_trigger(63.9),
            "a pull below the 64pt threshold does not trigger"
        );
        assert!(
            options.should_trigger(64.0),
            "a pull at the threshold triggers"
        );
        assert!(
            options.should_trigger(220.0),
            "a pull past the threshold still triggers"
        );

        options.trigger();
        assert_eq!(*refreshes.borrow(), 1, "the trigger ran the refresh once");

        // A held pull must not fire a second time. The model expresses that
        // through `is_refreshing`, which Swift drove from a `Binding`.
        let refreshing = options.clone().refreshing(true);
        assert!(
            !refreshing.should_trigger(220.0),
            "an in-flight refresh suppresses the pull"
        );
        refreshing.trigger();
        assert_eq!(
            *refreshes.borrow(),
            1,
            "a trigger while refreshing does nothing"
        );

        let done = refreshing.clone().refreshing(false);
        assert!(
            done.should_trigger(220.0),
            "the pull is armed again once the refresh finishes"
        );
        done.trigger();
        assert_eq!(*refreshes.borrow(), 2, "the second refresh ran");
    });
}

#[test]
fn the_threshold_and_label_are_authored_by_the_caller() {
    GlobalTestLock::locked(|| {
        let (options, _) = counting(Rc::new(RefCell::new(0)));
        assert_eq!(options.threshold, 64.0, "the default threshold is 64pt");
        assert!(options.label.is_none(), "no label is written by default");

        let labelled = options.clone().threshold(120.0).label("Refreshing");
        assert_eq!(labelled.threshold, 120.0, "the caller's threshold applies");
        assert!(
            !labelled.should_trigger(119.0),
            "the authored threshold governs the trigger"
        );
        assert!(labelled.should_trigger(120.0));
        assert_eq!(
            labelled.label.as_deref(),
            Some("Refreshing"),
            "the caller's label is kept"
        );
    });
}

#[test]
fn list_never_consults_its_refresh_options() {
    // Gap: see the module note. `List` accepts `RefreshOptions` but never calls
    // `should_trigger` or `trigger`, so no pull or keyboard path can refresh.
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let refreshes = Rc::new(RefCell::new(0));
            let counter = refreshes.clone();
            let graph = support::graph();
            let mut environment = Environment::default();
            environment.viewport.width = 360.0;
            environment.input = InputCapabilities::TOUCH;
            let list = List::new(
                CollectionSelection::single(Binding::constant(None::<usize>)),
                |index: usize, _selected: bool| {
                    AnyView::new(cove_ui_compose::primitives::text::Text::new(format!(
                        "Row {}",
                        index + 1
                    )))
                },
            )
            .rows((0..20_usize).collect())
            .refresh(RefreshOptions::new(move || *counter.borrow_mut() += 1))
            .frame_width(320.0)
            .frame_height(300.0);
            graph.install_view(
                AnyView::new(list.surface_environment(environment)),
                Default::default(),
            );
            for _ in 0..20 {
                graph.recomposer.commit_all();
                graph.compute_layout(360.0, 360.0);
            }

            let root = graph.tree.borrow().root().expect("root");
            let mut collected = vec![root.clone()];
            let mut cursor = 0;
            while cursor < collected.len() {
                let children = collected[cursor].children();
                collected.extend(children);
                cursor += 1;
            }

            assert!(
                !collected.iter().any(|node| {
                    support::debug_name(node).as_deref() == Some("refresh-indicator")
                }),
                "no refresh indicator is mounted"
            );
            assert_eq!(
                *refreshes.borrow(),
                0,
                "installing and laying out a list never refreshes"
            );
        });
    });
}
