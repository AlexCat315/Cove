//! A two-way reference to a value owned elsewhere.
//!
//! A binding over a [`Signal`] reads through the signal, so a view that reads
//! a binding in its body is subscribed exactly as if it read the signal.
use crate::Signal;
use std::rc::Rc;

pub struct Binding<T> {
    get: Rc<dyn Fn() -> T>,
    set: Rc<dyn Fn(T)>,
}

impl<T> Clone for Binding<T> {
    fn clone(&self) -> Self {
        Self {
            get: self.get.clone(),
            set: self.set.clone(),
        }
    }
}

impl<T: 'static> Binding<T> {
    pub fn new(get: impl Fn() -> T + 'static, set: impl Fn(T) + 'static) -> Self {
        Self {
            get: Rc::new(get),
            set: Rc::new(set),
        }
    }

    pub fn get(&self) -> T {
        (self.get)()
    }

    pub fn set(&self, value: T) {
        (self.set)(value);
    }

    /// A binding that always reads `value` and drops writes.
    pub fn constant(value: T) -> Self
    where
        T: Clone,
    {
        Self::new(move || value.clone(), |_| {})
    }

    /// A projection onto part of the value: `get` extracts it, `set` writes it
    /// back into a copy of the whole.
    pub fn map<U: 'static>(
        &self,
        get: impl Fn(&T) -> U + 'static,
        set: impl Fn(&mut T, U) + 'static,
    ) -> Binding<U> {
        let read = self.clone();
        let write = self.clone();
        Binding::new(
            move || get(&read.get()),
            move |value| {
                let mut whole = write.get();
                set(&mut whole, value);
                write.set(whole);
            },
        )
    }
}

/// The binding retains the signal's value cell, like a Swift binding retains
/// its `@State` storage: a closure that outlives the owning scope reads the
/// last value and writes into the detached cell instead of failing.
impl<T: Clone + 'static> From<Signal<T>> for Binding<T> {
    fn from(signal: Signal<T>) -> Self {
        let read = signal.retained_cell();
        let write = read.clone();
        Self::new(
            move || {
                signal.track_key();
                read.borrow().clone()
            },
            move |value| {
                *write.borrow_mut() = value;
                signal.notify();
            },
        )
    }
}

impl<T: Clone + 'static> Signal<T> {
    pub fn binding(&self) -> Binding<T> {
        (*self).into()
    }
}
