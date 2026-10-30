//! Bridges the shared `CounterView` example to the browser host contract.
//!
//! Swift imports `CounterExample` directly. The Rust equivalent lives in the
//! examples crate, so this module only adapts it to [`PrototypeContent`]; it
//! holds no state of its own.

use std::collections::BTreeMap;
use std::rc::Rc;

use cove_composition::ViewGraph;
use cove_examples::CounterView;

use crate::prototype::PrototypeContent;

impl PrototypeContent for CounterView {
    fn install(self: &Rc<Self>, graph: &Rc<ViewGraph>) {
        graph.install(self.clone(), Default::default());
    }

    fn checkpoint(&self) -> BTreeMap<String, String> {
        CounterView::checkpoint(self)
    }

    fn restore(&self, values: &BTreeMap<String, String>) -> bool {
        CounterView::restore(self, values)
    }

    fn summary(&self) -> (i64, bool, String) {
        (
            i64::from(self.count.get()),
            self.dark.get(),
            self.note.get(),
        )
    }
}
