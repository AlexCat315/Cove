//! Translation of CoveUICoreTests/DrawListResourceTests.swift.

use cove_core::{DrawList, DrawListResources, Point, Rect};
use std::sync::{Arc, Weak};

#[derive(Debug)]
struct Lease;

/// COW and translated composites retain resources after their source is reset.
#[test]
fn composite_lifetime() {
    let mut owner: Option<Arc<Lease>> = Some(Arc::new(Lease));
    let observed: Weak<Lease> = Arc::downgrade(owner.as_ref().unwrap());
    let mut cached = DrawList::default();
    cached.resources.retain(owner.clone().unwrap());
    cached.resources.retain(owner.clone().unwrap());
    cached
        .add_rect(Rect::new(0.0, 0.0, 10.0, 10.0), cove_core::Color::WHITE)
        .unwrap();
    let mut frame = DrawList::default();
    frame.append(&cached, Point::default()).unwrap();
    frame.append(&cached, Point { x: 10.0, y: 20.0 }).unwrap();
    assert_eq!(frame.resources.len(), 1);
    assert_eq!(frame.vertices().len(), 8);
    drop(owner.take());
    cached.reset();
    assert!(observed.upgrade().is_some());
    let mut restored = DrawList::default();
    restored
        .load(
            frame.vertices().to_vec(),
            frame.indices().to_vec(),
            frame.batches().to_vec(),
            frame.resources.clone(),
        )
        .unwrap();
    frame.reset();
    assert!(observed.upgrade().is_some());
    restored
        .load(vec![], vec![], vec![], DrawListResources::default())
        .unwrap();
    assert!(observed.upgrade().is_none());
}

/// Empty geometry can carry an ownership lease through append.
#[test]
fn empty_geometry() {
    let mut owner: Option<Arc<Lease>> = Some(Arc::new(Lease));
    let observed: Weak<Lease> = Arc::downgrade(owner.as_ref().unwrap());
    let mut source = DrawList::default();
    source.resources.retain(owner.clone().unwrap());
    let mut frame = DrawList::default();
    frame.append(&source, Point::default()).unwrap();
    drop(owner.take());
    source.reset();
    assert!(observed.upgrade().is_some());
    frame.reset();
    assert!(observed.upgrade().is_none());
}
