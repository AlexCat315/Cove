use cove_composition::{AnyView, LayoutDebugExt, View};
use cove_core::State;
use cove_hosts::android::integration::{AndroidContentSession, Rendering};
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::controls::Button;
use cove_ui_compose::primitives::text::Text;

struct C { count: State<i32> }
impl View for C {
    fn body(&self) -> AnyView {
        let count = self.count.clone();
        let bump = self.count.clone();
        AnyView::new(Button::new(Text::new(format!("Count {}", count.get())), move || bump.set(bump.get() + 1)).debug_name("increment").frame_size(120.0, 48.0))
    }
}

#[test]
fn probe() {
    let count = State::new(0);
    let mut session = AndroidContentSession::new(1.0, Rendering::Layers, AnyView::new(C { count: count.clone() }));
    session.prepare((200, 100), 1.0, 0.0);
    session.did_present();
    let b = session.inspect().controls.get("increment").copied().unwrap();
    session.pointer(0, b.x + 1.0, b.y + 1.0);
    session.pointer(1, b.x + 1.0, b.y + 1.0);
    eprintln!("count={}", count.get());
    for f in 0..3 {
        let list = session.prepare((200, 100), 1.0, 1.0/60.0);
        session.did_present();
        eprintln!("frame {f} vertices={} controls={:?}", list.vertices().len(), session.inspect().controls.keys().collect::<Vec<_>>());
    }
}
