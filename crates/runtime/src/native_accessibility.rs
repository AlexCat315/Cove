use crate::{PlatformWindowSession, Sdl3PlatformHost};
use cove_core::scene::*;
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};
pub struct NativeAccessibility;
impl NativeAccessibility {
    pub fn synchronize(
        host: &Sdl3PlatformHost,
        session: &Rc<PlatformWindowSession>,
        root: &Node,
        timeline: &RefCell<PerformanceTimeline>,
    ) {
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        {
            let native = match host.native_window_reference(session.id) {
                Some(cove_shell::NativeWindowReference::AppKit(raw))
                | Some(cove_shell::NativeWindowReference::UIKit(raw)) => raw,
                _ => return,
            };
            let bridge = if let Some(bridge) = root.first_resource::<AppleAccessibilityBridge>() {
                bridge
            } else {
                let bridge = AppleAccessibilityBridge::new(native, session.id);
                root.add_resource(bridge.clone());
                bridge
            };
            bridge.update(host, session, root, timeline);
        }
        #[cfg(not(any(target_os = "macos", target_os = "ios")))]
        {
            let _ = (host, session, root, timeline);
        }
    }
    /// Runs the actions assistive technology requested since the last call.
    pub fn perform_pending() {
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        apple::perform_pending();
    }
}
#[cfg(any(target_os = "macos", target_os = "ios"))]
mod apple {
    use super::*;
    use crate::platform::{
        AccessibilityItem, AccessibilitySnapshot, PlatformCommand, PlatformLink,
        accessibility::{Presenter, take_actions},
    };
    use cove_platform::WindowId;
    use std::{any::Any, collections::HashMap, ffi::CString, sync::Arc};
    thread_local! {
        static BRIDGES: RefCell<HashMap<u64, Weak<AppleAccessibilityBridge>>> = RefCell::default();
    }
    pub(super) fn perform_pending() {
        for action in take_actions() {
            let bridge = BRIDGES
                .with(|bridges| bridges.borrow().get(&action.bridge).and_then(Weak::upgrade));
            if let Some(bridge) = bridge {
                bridge.perform(action.element, action.kind, action.value.as_deref());
            }
        }
    }
    /// Where a window's snapshots go: a presenter on this thread when it owns the
    /// platform, otherwise the platform thread.
    enum Target {
        Direct {
            window: usize,
            presenter: RefCell<Option<Presenter>>,
        },
        Remote(Arc<PlatformLink>),
    }
    #[derive(Clone, PartialEq)]
    struct Signature {
        roots: Vec<ElementId>,
        focused: Option<ElementId>,
        frame: cove_core::Rect,
        native: [f64; 4],
    }
    pub(super) struct AppleAccessibilityBridge {
        key: u64,
        window: WindowId,
        target: Target,
        closed: Cell<bool>,
        nodes: RefCell<HashMap<u64, NodeWeak>>,
        session: RefCell<Weak<PlatformWindowSession>>,
        last: RefCell<Option<Signature>>,
        structure: Cell<u64>,
        geometry: Cell<u64>,
        last_time: Cell<f64>,
        /// Provider values re-read only when their revision moves.
        provided_values: RefCell<HashMap<u64, (u64, CString)>>,
    }
    impl AppleAccessibilityBridge {
        pub fn new(window: usize, id: WindowId) -> Rc<Self> {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
            let key = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let bridge = Rc::new(Self {
                key,
                window: id,
                target: match PlatformLink::current() {
                    Some(link) => Target::Remote(link),
                    None => Target::Direct {
                        window,
                        presenter: RefCell::new(None),
                    },
                },
                closed: Cell::new(false),
                nodes: RefCell::new(HashMap::new()),
                session: RefCell::new(Weak::new()),
                last: RefCell::new(None),
                structure: Cell::new(u64::MAX),
                geometry: Cell::new(u64::MAX),
                last_time: Cell::new(0.0),
                provided_values: RefCell::default(),
            });
            BRIDGES.with(|bridges| {
                bridges.borrow_mut().insert(key, Rc::downgrade(&bridge));
            });
            bridge
        }
        fn perform(&self, id: u64, kind: u32, value: Option<&str>) {
            let Some(session) = self.session.borrow().upgrade() else {
                return;
            };
            let weak = self.nodes.borrow().get(&id).cloned();
            let Some(node) = weak.as_ref().and_then(Node::upgrade) else {
                return;
            };
            if !node.accepts_subtree_input()
                || !node.accessibility().is_some_and(|s| s.state.is_enabled)
                || !session.input_context.focus_chain.permits_input(&node)
            {
                return;
            }
            let actions = node.accessibility_actions();
            let handled = session.with_current(|| match kind {
                1 => actions.activate.map(|f| f()).is_some(),
                2 => match (actions.set_value, value) {
                    (Some(f), Some(text)) => {
                        f(text);
                        true
                    }
                    _ => false,
                },
                3 => actions.increment.map(|f| f()).is_some(),
                4 => actions.decrement.map(|f| f()).is_some(),
                5 => {
                    session.input_context.focus_chain.focus(Some(&node), true);
                    true
                }
                _ => false,
            });
            if handled {
                session.with_current(|| {
                    session.recomposer.commit_all();
                });
                session.request_display();
            }
        }
        fn deliver(&self, snapshot: AccessibilitySnapshot) {
            match &self.target {
                Target::Direct { window, presenter } => {
                    presenter
                        .borrow_mut()
                        .get_or_insert_with(|| Presenter::new(*window, self.key))
                        .apply(&snapshot);
                }
                Target::Remote(link) => {
                    link.bridge
                        .post(PlatformCommand::UpdateAccessibility(self.window, snapshot));
                }
            }
        }
        pub fn update(
            &self,
            host: &Sdl3PlatformHost,
            session: &Rc<PlatformWindowSession>,
            root: &Node,
            timeline: &RefCell<PerformanceTimeline>,
        ) {
            *self.session.borrow_mut() = Rc::downgrade(session);
            if self.closed.get() {
                return;
            }
            let focus = &session.input_context.focus_chain;
            let roots = if let Some(modal) = focus.modal_root() {
                let mut roots = vec![modal];
                roots.extend(focus.accessory_roots());
                roots
            } else {
                vec![root.clone()]
            };
            // Screen-space frames follow the window, so its placement is part of
            // what makes a committed tree stale.
            let position = host.window_position(session.id).unwrap_or_default();
            let logical = session.logical_size();
            let native = [
                f64::from(position.0),
                f64::from(position.1),
                f64::from(logical.0),
                f64::from(logical.1),
            ];
            let signature = Signature {
                roots: roots.iter().map(Node::id).collect(),
                focused: focus.focused().map(|n| n.id()),
                frame: root.frame(),
                native,
            };
            let structure = AccessibilityRevision::structure();
            let geometry = AccessibilityRevision::geometry();
            let now = cove_shell::monotonic_seconds();
            let changed = structure != self.structure.get()
                || self.last.borrow().as_ref() != Some(&signature);
            if !changed
                && (geometry == self.geometry.get() || now - self.last_time.get() < 1.0 / 30.0)
            {
                return;
            }
            self.structure.set(structure);
            self.geometry.set(geometry);
            self.last_time.set(now);
            *self.last.borrow_mut() = Some(signature);
            let trace = timeline.borrow().begin();
            let snapshots = roots
                .iter()
                .flat_map(|node| {
                    AccessibilityTree::snapshot_in(node, Some(root.absolute_frame()), false)
                })
                .collect::<Vec<_>>();
            timeline
                .borrow_mut()
                .end(trace, "frame", "Accessibility snapshot", None, &[]);
            fn text(value: &str) -> CString {
                CString::new(value.replace('\0', "")).unwrap_or_default()
            }
            fn flatten(
                elements: &[AccessibilityElement],
                parent: u64,
                items: &mut Vec<AccessibilityItem>,
                nodes: &mut HashMap<u64, NodeWeak>,
                provided: &mut HashMap<u64, (u64, CString)>,
                retained: &mut HashMap<u64, (u64, CString)>,
                focus: &FocusChain,
            ) {
                for item in elements {
                    let id = item.id().0;
                    nodes.insert(id, item.node.downgrade());
                    let s = &item.semantics;
                    let st = &s.state;
                    let actions = item.node.accessibility_actions();
                    let interactive = item.node.is_focusable()
                        || actions.activate.is_some()
                        || actions.set_value.is_some()
                        || actions.increment.is_some()
                        || actions.decrement.is_some();
                    let enabled = st.is_enabled && (!interactive || item.accepts_input);
                    let flags = u32::from(enabled)
                        | u32::from(st.is_selected) << 1
                        | u32::from(st.is_expanded.is_some()) << 2
                        | u32::from(st.is_expanded == Some(true)) << 3
                        | u32::from(st.is_required) << 4
                        | u32::from(st.is_invalid) << 5
                        | u32::from(st.is_read_only) << 6
                        | u32::from(st.is_secure) << 7
                        | u32::from(focus.focused().is_some_and(|n| n.id() == item.id())) << 8
                        | u32::from(item.accepts_input) << 9
                        | u32::from(item.node.is_focusable()) << 10;
                    let action_flags = u32::from(actions.activate.is_some())
                        | u32::from(actions.set_value.is_some()) << 1
                        | u32::from(actions.increment.is_some()) << 2
                        | u32::from(actions.decrement.is_some()) << 3
                        | u32::from(item.node.is_focusable()) << 4;
                    let static_text = matches!(
                        s.role,
                        AccessibilityRole::StaticText | AccessibilityRole::Status
                    );
                    let bare_text = static_text && s.value.is_empty();
                    let provider = item.node.accessibility_value_provider();
                    let revision = provider.as_ref().map_or(0, |p| p.revision);
                    let live_value = provider.map(|provider| {
                        let value = match provided.remove(&id) {
                            Some((seen, value)) if seen == revision => value,
                            _ => text(&(provider.read)()),
                        };
                        retained.insert(id, (revision, value.clone()));
                        value
                    });
                    items.push(AccessibilityItem {
                        id,
                        parent,
                        role: role_id(s.role),
                        flags,
                        actions: action_flags,
                        frame: [
                            item.frame.x,
                            item.frame.y,
                            item.frame.width,
                            item.frame.height,
                        ],
                        label: text(if bare_text { "" } else { &s.label }),
                        value: text(if bare_text { &s.label } else { &s.value }),
                        live_value,
                        help: text(&s.help),
                        identifier: text(&s.identifier),
                        revision,
                    });
                    flatten(&item.children, id, items, nodes, provided, retained, focus);
                }
            }
            let commit = timeline.borrow().begin();
            let mut items = Vec::new();
            let mut nodes = HashMap::new();
            let mut retained = HashMap::new();
            flatten(
                &snapshots,
                0,
                &mut items,
                &mut nodes,
                &mut self.provided_values.borrow_mut(),
                &mut retained,
                focus,
            );
            *self.provided_values.borrow_mut() = retained;
            *self.nodes.borrow_mut() = nodes;
            self.deliver(AccessibilitySnapshot {
                bridge: self.key,
                items,
            });
            timeline
                .borrow_mut()
                .end(commit, "frame", "Accessibility bridge commit", None, &[]);
        }
        fn invalidate(&self) {
            if self.closed.replace(true) {
                return;
            }
            BRIDGES.with(|bridges| {
                bridges.borrow_mut().remove(&self.key);
            });
            match &self.target {
                Target::Direct { presenter, .. } => drop(presenter.borrow_mut().take()),
                Target::Remote(link) => {
                    link.bridge
                        .post(PlatformCommand::CloseAccessibility(self.window, self.key));
                }
            }
            self.nodes.borrow_mut().clear();
        }
    }
    impl NodeResource for AppleAccessibilityBridge {
        fn mount(&self, _: &Node) {}
        fn unmount(&self, _: &Node) {
            self.invalidate();
        }
        fn as_any_rc(self: Rc<Self>) -> Rc<dyn Any> {
            self
        }
    }
    impl Drop for AppleAccessibilityBridge {
        fn drop(&mut self) {
            self.invalidate();
        }
    }
    fn role_id(role: AccessibilityRole) -> u32 {
        use AccessibilityRole::*;
        match role {
            Group => 0,
            Toolbar => 1,
            Button => 2,
            StaticText => 3,
            Heading => 4,
            TextField => 5,
            Checkbox => 6,
            Toggle => 7,
            Slider => 8,
            Radio => 9,
            Tab => 10,
            List => 11,
            ListItem => 12,
            Tree => 13,
            TreeItem => 14,
            Table => 15,
            Row => 16,
            Cell => 17,
            Dialog => 18,
            Status => 19,
            Progress => 20,
            Image => 21,
            Link => 22,
            ScrollArea => 23,
            ComboBox => 24,
            TabList => 25,
            TabPanel => 26,
        }
    }
}
#[cfg(any(target_os = "macos", target_os = "ios"))]
use apple::AppleAccessibilityBridge;
