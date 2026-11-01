//! Translation of `Tests/CoveUIComposeTests/PortalScopeTests.swift`.
//!
//! Behaviour checklist:
//! - distinct `PortalStore`s never share entries, and the holder always reads
//!   whichever store is current
//! - a `PortalStoreAmbient` attached to a `PlatformInputContext` swaps the
//!   active store in lockstep with `with_current`, so two contexts stay
//!   isolated
//! - nested scopes restore the outer store on exit, and the outermost scope
//!   restores the previous store

mod support;

use std::rc::Rc;

use cove_composition::{AnyView, EmptyView};
use cove_core::Point;
use cove_core::scene::PlatformInputContext;
use cove_ui_compose::primitives::overlays::portal::{
    PortalStore, PortalStoreAmbient, PortalStoreHolder,
};

use support::GlobalTestLock;

fn entry() -> AnyView {
    AnyView::new(EmptyView)
}

#[test]
fn distinct_portal_stores_do_not_share_entries() {
    GlobalTestLock::locked(|| {
        let saved = PortalStoreHolder::current();
        let a = Rc::new(PortalStore::new());
        let b = Rc::new(PortalStore::new());

        PortalStoreHolder::set_current(Some(a.clone()));
        PortalStoreHolder::current().register(None, Point::default(), None, entry());

        assert_eq!(a.entries().len(), 1, "store A holds the entry");
        assert!(b.entries().is_empty(), "store B never saw store A's entry");

        PortalStoreHolder::set_current(Some(b.clone()));
        assert!(
            PortalStoreHolder::current().entries().is_empty(),
            "the holder reads the newly current store"
        );

        PortalStoreHolder::current().register(None, Point::default(), None, entry());
        assert_eq!(b.entries().len(), 1, "store B holds its own entry");
        assert_eq!(a.entries().len(), 1, "store A is untouched");

        PortalStoreHolder::set_current(Some(saved));
    });
}

#[test]
fn ambient_scopes_the_store_with_the_context() {
    GlobalTestLock::locked(|| {
        let saved = PortalStoreHolder::current();

        let store_a = Rc::new(PortalStore::new());
        let context_a = PlatformInputContext::new();
        context_a.add_scoped_ambient(Rc::new(PortalStoreAmbient::new(store_a.clone())));

        let store_b = Rc::new(PortalStore::new());
        let context_b = PlatformInputContext::new();
        context_b.add_scoped_ambient(Rc::new(PortalStoreAmbient::new(store_b.clone())));

        context_a.with_current(|| {
            PortalStoreHolder::current().register(None, Point::default(), None, entry());
        });
        context_b.with_current(|| {
            assert!(
                PortalStoreHolder::current().entries().is_empty(),
                "context B's scope starts empty; A's entry did not leak in"
            );
            PortalStoreHolder::current().register(None, Point::default(), None, entry());
        });

        assert_eq!(store_a.entries().len(), 1, "store A holds A's entry");
        assert_eq!(store_b.entries().len(), 1, "store B holds B's entry");

        PortalStoreHolder::set_current(Some(saved));
    });
}

#[test]
fn nested_scopes_restore_the_outer_store() {
    GlobalTestLock::locked(|| {
        let saved = PortalStoreHolder::current();

        let store_a = Rc::new(PortalStore::new());
        let context_a = PlatformInputContext::new();
        context_a.add_scoped_ambient(Rc::new(PortalStoreAmbient::new(store_a.clone())));

        let store_b = Rc::new(PortalStore::new());
        let context_b = PlatformInputContext::new();
        context_b.add_scoped_ambient(Rc::new(PortalStoreAmbient::new(store_b.clone())));

        context_a.with_current(|| {
            assert!(
                Rc::ptr_eq(&PortalStoreHolder::current(), &store_a),
                "context A installs store A"
            );
            context_b.with_current(|| {
                assert!(
                    Rc::ptr_eq(&PortalStoreHolder::current(), &store_b),
                    "the inner scope installs store B"
                );
            });
            assert!(
                Rc::ptr_eq(&PortalStoreHolder::current(), &store_a),
                "the inner scope restored store A on exit"
            );
        });
        assert!(
            Rc::ptr_eq(&PortalStoreHolder::current(), &saved),
            "the outer scope restored the previous store"
        );
    });
}
