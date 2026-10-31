//! Property wrapper that mirrors a value published by an observable object.
use cove_core::scene::ObservableObject;
use cove_core::{Binding, State};
use std::rc::Rc;

pub struct Observed<Object: ObservableObject + 'static, Value: Clone + PartialEq + Send + 'static> {
    state: State<Value>,
    _subscription: Rc<ObservedSubscription<Object>>,
}
impl<Object: ObservableObject + 'static, Value: Clone + PartialEq + Send + 'static> Clone
    for Observed<Object, Value>
{
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            _subscription: self._subscription.clone(),
        }
    }
}

impl<Object: ObservableObject + 'static, Value: Clone + PartialEq + Send + 'static>
    Observed<Object, Value>
{
    pub fn new(object: Rc<Object>, project: impl Fn(&Object) -> Value + 'static) -> Self {
        let state = State::new(project(&object));
        let weak_object = Rc::downgrade(&object);
        let observer_state = state.clone();
        let project = Rc::new(project);
        let callback_project = project.clone();
        let token = object.register_observer(Rc::new(move || {
            let Some(object) = weak_object.upgrade() else {
                return;
            };
            let value = callback_project(&object);
            if observer_state.get() != value {
                observer_state.set(value);
            }
        }));
        Self {
            state,
            _subscription: Rc::new(ObservedSubscription { object, token }),
        }
    }

    pub fn get(&self) -> Value {
        self.state.get()
    }
    pub fn binding(&self) -> Binding<Value> {
        self.state.binding()
    }
    pub fn dynamic_field(&self, name: impl Into<String>) -> crate::DynamicField {
        crate::DynamicField::state(name, &self.state)
    }
}

struct ObservedSubscription<Object: ObservableObject> {
    object: Rc<Object>,
    token: u64,
}
impl<Object: ObservableObject> Drop for ObservedSubscription<Object> {
    fn drop(&mut self) {
        self.object.unregister_observer(self.token);
    }
}
