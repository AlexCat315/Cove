//! Translation of CoveUICoreTests/PortableCoreTests.swift.

use cove_core::{Clip, Color, DrawList, Point, Rect, State, Vertex};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn binding_writes_invalidate_the_same_state_storage() {
    let state = State::new(0);
    let writes = Arc::new(AtomicUsize::new(0));
    let counter = writes.clone();
    let _subscription = state.subscribe(move |_| {
        counter.fetch_add(1, Ordering::SeqCst);
    });
    let binding = state.binding();
    binding.set(3);
    assert_eq!(state.get(), 3);
    assert_eq!(writes.load(Ordering::SeqCst), 1);
    state.set(5);
    assert_eq!(binding.get(), 5);
    assert_eq!(writes.load(Ordering::SeqCst), 2);
}

#[test]
fn cached_geometry_keeps_clipping_when_translated() {
    let mut source = DrawList::default();
    source.push_clip(Clip {
        rect: Rect::new(10.0, 10.0, 40.0, 40.0),
        mask: None,
    });
    source
        .add_rect(Rect::new(12.0, 14.0, 10.0, 10.0), Color::WHITE)
        .unwrap();
    let mut destination = DrawList::default();
    destination
        .append(&source, Point { x: 7.0, y: -5.0 })
        .unwrap();
    assert_eq!(destination.vertices()[0].x, 19.0);
    assert_eq!(destination.vertices()[0].y, 9.0);
    assert_eq!(destination.batches()[0].scissor, source.batches()[0].scissor);
    assert_eq!(destination.indices(), source.indices());
}

#[test]
fn portable_geometry_uses_desktop_vertex_layout_and_batching() {
    assert_eq!(std::mem::size_of::<Vertex>(), 20);
    let mut list = DrawList::default();
    list.add_rounded_rect(Rect::new(0.0, 0.0, 100.0, 50.0), 10.0, Color::WHITE)
        .unwrap();
    list.add_rect(Rect::new(120.0, 0.0, 10.0, 10.0), Color::BLACK)
        .unwrap();
    assert_eq!(list.batches().len(), 1);
    assert!(
        list.indices()
            .iter()
            .all(|&index| (index as usize) < list.vertices().len())
    );
    assert_eq!(list.batches()[0].index_count, list.indices().len() as u32);
    assert_eq!(
        Color::from_rgba8(0x12, 0x34, 0x56, 0x78).rgba8(),
        0x7856_3412
    );
}
