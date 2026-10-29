//! Type-erased dynamic properties, the Rust counterpart of `_StateErased`.
//!
//! Swift discovers `@State` members with `Mirror`; Rust has no reflection, so a
//! view lists its observable members from `View::dynamic_properties`. The
//! wrapper keeps the state handle and its invalidation subscription together so
//! dropping the wrapper releases the observer.
use cove_core::state::LocalSubscription;
use cove_core::{ReadScopeId, State};
use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;

/// A state member exposed to composition for invalidation, devtools and value
/// transfer between successive view values (`ViewScope::replace_view`).
pub struct DynamicField {
    name: String,
    inner: Rc<dyn ErasedState>,
}
impl DynamicField {
    pub fn state<T: Clone + Send + 'static>(name: impl Into<String>, state: &State<T>) -> Self {
        Self { name: name.into(), inner: Rc::new(StateField { state: state.clone(), subscription: RefCell::new(None) }) }
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn diagnostic_kind(&self) -> &'static str {
        self.inner.diagnostic_kind()
    }
    pub fn dev_tools_value_type(&self) -> String {
        self.inner.dev_tools_value_type()
    }
    pub fn dev_tools_read(&self) -> Option<Rc<dyn Fn() -> String>> {
        self.inner.dev_tools_read()
    }
    /// Route writes into `invalidate`. The subscription lives in the field, so
    /// the caller must retain the field for as long as the view is installed.
    pub fn wire(&self, invalidate: Rc<dyn Fn()>) {
        self.inner.wire(invalidate);
    }
    pub fn wire_read_tracking(&self, scope: ReadScopeId, precise: bool) {
        self.inner.wire_read_tracking(scope, precise);
    }
    pub fn unwire(&self) {
        self.inner.unwire();
    }
    /// Copy the runtime value out of `other` when both wrap the same state type.
    pub fn copy_value_from(&self, other: &DynamicField) {
        self.inner.copy_value_from(other.inner.as_ref());
    }
}

trait ErasedState {
    fn diagnostic_kind(&self) -> &'static str;
    fn dev_tools_value_type(&self) -> String;
    fn dev_tools_read(&self) -> Option<Rc<dyn Fn() -> String>>;
    fn wire(&self, invalidate: Rc<dyn Fn()>);
    fn wire_read_tracking(&self, scope: ReadScopeId, precise: bool);
    fn unwire(&self);
    fn copy_value_from(&self, other: &dyn ErasedState);
    fn as_any(&self) -> &dyn Any;
}

struct StateField<T> {
    state: State<T>,
    subscription: RefCell<Option<LocalSubscription>>,
}
impl<T: Clone + Send + 'static> ErasedState for StateField<T> {
    fn diagnostic_kind(&self) -> &'static str {
        "state"
    }
    fn dev_tools_value_type(&self) -> String {
        self.state.value_type().to_string()
    }
    fn dev_tools_read(&self) -> Option<Rc<dyn Fn() -> String>> {
        let state = self.state.clone();
        state.inspection_summary()?;
        Some(Rc::new(move || state.inspection_summary().unwrap_or_default()))
    }
    fn wire(&self, invalidate: Rc<dyn Fn()>) {
        let mut slot = self.subscription.borrow_mut();
        if slot.is_some() {
            return;
        }
        *slot = Some(self.state.subscribe_local(move |_revision| invalidate()));
    }
    fn wire_read_tracking(&self, _scope: ReadScopeId, _precise: bool) {
        // Invalidation is driven by `wire` for every state member. The core
        // read-tracking registry requires `Send` invalidators, which an
        // `Rc`-owned composition scope cannot provide; precise field filtering
        // is therefore deferred until a local read-scope variant exists.
    }
    fn unwire(&self) {
        self.subscription.borrow_mut().take();
    }
    fn copy_value_from(&self, other: &dyn ErasedState) {
        let Some(other) = other.as_any().downcast_ref::<Self>() else { return };
        self.state.copy_runtime_value(&other.state);
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Build a `dynamic_properties` list without repeating the wrapper call.
#[macro_export]
macro_rules! dynamic_fields {
    ($($name:literal => $state:expr),* $(,)?) => {
        vec![$($crate::DynamicField::state($name, &$state)),*]
    };
}
