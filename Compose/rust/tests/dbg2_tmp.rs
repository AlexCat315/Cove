mod support;
use std::cell::RefCell;
use std::rc::Rc;
use cove_composition::{AnyView, View, ViewExt};
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::{ScrollView};

#[test]
fn dbg(){
 support::GlobalTestLock::locked(||{
  let log: Rc<RefCell<Vec<(f32,f32,f32,f32)>>> = Rc::new(RefCell::new(Vec::new()));
  let g=support::graph();
  let content = AnyView::new(cove_ui_compose::BoxView::new(
      cove_core::scene::FlexDirection::Column, cove_core::scene::Align::Stretch,
      cove_core::scene::Justify::FlexStart, cove_core::scene::Wrap::NoWrap, 0.0,
      AnyView::new(cove_composition::EmptyView)))
      .frame_width(200.0).frame_height(400.0);
  let l = log.clone();
  let sv = ScrollView::new(cove_ui_compose::ScrollAxis::Vertical, content)
      .on_geometry_change(move |geo| l.borrow_mut().push((geo.offset.x,geo.offset.y,geo.viewport_size.width,geo.viewport_size.height)));
  g.install_view(AnyView::new(AnyView::new(sv).frame_width(200.0).frame_height(50.0)), Default::default());
  for pass in 0..4 { g.recomposer.commit_all(); g.compute_layout(200.0,50.0); println!("pass {pass}: {:?}", log.borrow()); }
 });
}
