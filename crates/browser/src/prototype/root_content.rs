//! Selects the root the browser reactor renders.
//!
//! The Swift prototype hard-codes `CounterView`. The Rust host instead honours
//! the declaration published by `cove_ui::application!` through its Wasm module
//! initializer ([`cove_hosts::wasm`]), so any shared application runs in the
//! browser unchanged. The counter remains the fallback when no application
//! module has been initialized.

use std::collections::BTreeMap;
use std::rc::Rc;

use cove_composition::{AnyView, ViewGraph};
use cove_examples::CounterView;
use cove_hosts::ApplicationCheckpoint;
use cove_ui_compose::primitives::overlays::portal::LayerRoot;

use super::prototype::PrototypeContent;

/// The root content a browser host renders.
pub enum RootContent {
    /// The shared counter prototype the Swift host hard-coded.
    Counter(CounterView),
    /// An application registered through `cove_ui::application!`.
    Application {
        root: AnyView,
        checkpoint: Option<ApplicationCheckpoint>,
    },
}

impl RootContent {
    /// Selects the root for this browser surface.
    ///
    /// The page shell is the Counter prototype: its buttons, note field and
    /// `count`/`dark` frame payload only make sense with `CounterView`. Passing
    /// `?component=<name>` to the page selects the registered
    /// `cove_ui::application!` (Gallery) instead, so shared applications still
    /// run unchanged in the browser.
    pub fn from_registration() -> Self {
        let gallery = std::env::args().skip(1).any(|arg| arg == "--component");
        if !gallery {
            return Self::Counter(CounterView::new());
        }
        match cove_hosts::wasm::application() {
            Some(factory) => {
                let launch = factory();
                Self::Application {
                    root: launch.content(),
                    checkpoint: launch.checkpoint,
                }
            }
            None => Self::Counter(CounterView::new()),
        }
    }
}

impl PrototypeContent for RootContent {
    fn install(self: &Rc<Self>, graph: &Rc<ViewGraph>) {
        match self.as_ref() {
            Self::Counter(counter) => {
                // The counter fallback has no chrome of its own, so it needs a
                // portal layer for tooltips/menus/popovers to paint into. An
                // application root supplies its own `LayerRoot`, so it is
                // installed unchanged to avoid stacking two portal hosts.
                graph.install_view(
                    AnyView::new(LayerRoot::new(CounterView::clone(counter))),
                    Default::default(),
                );
            }
            Self::Application { root, .. } => {
                graph.install_view(root.clone(), Default::default());
            }
        }
    }

    fn checkpoint(&self) -> BTreeMap<String, String> {
        match self {
            Self::Counter(counter) => counter.checkpoint(),
            Self::Application {
                checkpoint: Some(hooks),
                ..
            } => hooks.capture(),
            Self::Application {
                checkpoint: None, ..
            } => BTreeMap::new(),
        }
    }

    fn restore(&self, values: &BTreeMap<String, String>) -> bool {
        match self {
            Self::Counter(counter) => counter.restore(values),
            Self::Application {
                checkpoint: Some(hooks),
                ..
            } => hooks.restore(values),
            Self::Application {
                checkpoint: None, ..
            } => values.is_empty(),
        }
    }

    fn summary(&self) -> (i64, bool, String) {
        match self {
            Self::Counter(counter) => (
                i64::from(counter.count.get()),
                counter.dark.get(),
                counter.note.get(),
            ),
            // The frame's counter fields are prototype-specific; an application
            // root publishes neutral values and exposes its state through the
            // checkpoint hooks instead.
            Self::Application { .. } => (0, false, String::new()),
        }
    }
}
