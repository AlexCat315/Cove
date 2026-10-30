//! Type-erased dynamic properties, the Rust counterpart of `_StateErased`.
//!
//! Swift discovers `@State` members with `Mirror`; Rust has no reflection, so a
//! view lists its observable members from `View::dynamic_properties`. The
//! wrapper keeps the state handle and its invalidation subscription together so
//! dropping the wrapper releases the observer.
use cove_core::state::LocalSubscription;
use cove_core::{ReadScope, State};
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
        Self {
            name: name.into(),
            inner: Rc::new(StateField {
                state: state.clone(),
                subscription: RefCell::new(None),
                tracking_owner: RefCell::new(None),
            }),
        }
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
    pub fn wire(&self, invalidate: Rc<dyn Fn()>, precise: bool) {
        self.inner.wire(invalidate, precise);
    }
    pub fn wire_read_tracking(&self, scope: ReadScope, precise: bool) {
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
    fn wire(&self, invalidate: Rc<dyn Fn()>, precise: bool);
    fn wire_read_tracking(&self, scope: ReadScope, precise: bool);
    fn unwire(&self);
    fn copy_value_from(&self, other: &dyn ErasedState);
    fn as_any(&self) -> &dyn Any;
}

struct StateField<T> {
    state: State<T>,
    subscription: RefCell<Option<LocalSubscription>>,
    tracking_owner: RefCell<Option<cove_core::ReadScopeId>>,
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
    fn wire(&self, invalidate: Rc<dyn Fn()>, precise: bool) {
        if precise {
            self.subscription.borrow_mut().take();
            return;
        }
        let mut slot = self.subscription.borrow_mut();
        if slot.is_some() {
            return;
        }
        *slot = Some(self.state.subscribe_local(move |_revision| invalidate()));
    }
    fn wire_read_tracking(&self, scope: ReadScope, precise: bool) {
        if precise {
            let previous = self.tracking_owner.borrow_mut().replace(scope.id);
            if let Some(previous) = previous.filter(|previous| *previous != scope.id) {
                self.state.clear_read_tracking_owner(previous);
            }
            self.state.set_read_tracking_owner(scope, true);
        } else {
            if let Some(previous) = self.tracking_owner.borrow_mut().take() {
                self.state.clear_read_tracking_owner(previous);
            }
        }
    }
    fn unwire(&self) {
        self.subscription.borrow_mut().take();
        if let Some(scope) = self.tracking_owner.borrow_mut().take() {
            self.state.clear_read_tracking_owner(scope);
        }
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
