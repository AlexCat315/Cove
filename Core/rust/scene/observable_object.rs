//! Translation of CoveUIScene/ObservableObject.swift.
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::Rc,
};
pub trait ObservableObject {
    fn register_observer(&self, handler: Rc<dyn Fn()>) -> u64;
    fn unregister_observer(&self, token: u64);
}
#[derive(Default)]
pub struct ObservablePublisher {
    next_key: Cell<u64>,
    observers: RefCell<BTreeMap<u64, Rc<dyn Fn()>>>,
}
impl ObservablePublisher {
    pub fn send(&self) {
        let observers = self
            .observers
            .borrow()
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for handler in observers {
            handler();
        }
    }
}
impl ObservableObject for ObservablePublisher {
    fn register_observer(&self, handler: Rc<dyn Fn()>) -> u64 {
        let key = self.next_key.get();
        self.next_key.set(
            key.checked_add(1)
                .expect("observer identity space exhausted"),
        );
        self.observers.borrow_mut().insert(key, handler);
        key
    }
    fn unregister_observer(&self, token: u64) {
        self.observers.borrow_mut().remove(&token);
    }
}
