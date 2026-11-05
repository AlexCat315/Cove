//! Translation of Compose/Sources/CoveUICompose/Primitives/Overlays/Portal/PortalRegistry.swift.
use super::fit_constraints::PortalFitConstraints;
use super::portal_placement::{PortalPlacement, PortalPlacementMode, portal_window_bounds};
use crate::AnyView;
use crate::adaptive::{SurfaceEnvironment, SurfaceEnvironmentExt};
use crate::animation::{AnimatedVisibility, Transition};
use crate::theme::{
    IconScalingViewExt, ReducedMotionViewExt, SemanticMotionRef, TextScalerViewExt, Theme,
    ThemeViewExt,
};
use cove_core::scene::{CompositionLocal, Node, NodeResource, NodeWeak};
use cove_core::{Point, Rect, Size};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};
use std::rc::{Rc, Weak};

// MARK: - Entry

#[derive(Clone)]
pub struct PortalEntry {
    pub id: String,
    pub position: Point,
    pub width: Option<f32>,
    pub content: AnyView,
    pub fills_window: bool,
    pub placement: PortalPlacementMode,
    pub parent_id: Option<String>,
    pub is_exiting: bool,
    pub fit: PortalFitConstraints,
    /// Resolve against measured content in the same layout pass as the slot.
    pub position_for_size: Option<Rc<dyn Fn(Size) -> Point>>,
}
impl PortalEntry {
    pub fn new(
        id: impl Into<String>,
        position: Point,
        width: Option<f32>,
        content: AnyView,
    ) -> Self {
        Self {
            id: id.into(),
            position,
            width,
            content,
            fills_window: false,
            placement: PortalPlacementMode::BelowAnchor,
            parent_id: None,
            is_exiting: false,
            fit: PortalFitConstraints::default(),
            position_for_size: None,
        }
    }
}

struct Dismissal {
    anchor: Rc<dyn Fn() -> Rect>,
    dismiss: Rc<dyn Fn()>,
}

#[derive(Default)]
struct StoreInner {
    storage: BTreeMap<String, PortalEntry>,
    slot_nodes: HashMap<String, NodeWeak>,
    observers: BTreeMap<u64, Rc<dyn Fn(i64)>>,
    current_revision: i64,
    presentation_order: Vec<String>,
    dismissals: HashMap<String, Dismissal>,
}

/// Per-window store of overlay (popover/menu/tooltip) entries.
///
/// Storage lives on the instance, not in process-global statics, so each window
/// owns its own overlay layer — one window's open dropdown can never paint
/// into, or swallow clicks for, another's. Call sites read
/// `PortalStoreHolder::current()` (the scoped ambient swapped by
/// `set_current`) and entry owners remember their store for cleanup.
#[derive(Default)]
pub struct PortalStore {
    inner: RefCell<StoreInner>,
}

impl PortalStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn entries(&self) -> Vec<PortalEntry> {
        self.rendered_entries()
            .into_iter()
            .filter(|entry| !entry.is_exiting)
            .collect()
    }

    pub fn rendered_entries(&self) -> Vec<PortalEntry> {
        let inner = self.inner.borrow();
        inner
            .presentation_order
            .iter()
            .filter_map(|id| inner.storage.get(id).cloned())
            .collect()
    }

    pub fn revision(&self) -> i64 {
        self.inner.borrow().current_revision
    }

    pub fn register(
        &self,
        id: Option<String>,
        position: Point,
        width: Option<f32>,
        content: AnyView,
    ) -> String {
        let id = id.unwrap_or_else(new_portal_id);
        {
            let mut inner = self.inner.borrow_mut();
            if !inner.storage.contains_key(&id) {
                inner.presentation_order.push(id.clone());
            }
            inner.storage.insert(
                id.clone(),
                PortalEntry::new(id.clone(), position, width, content),
            );
        }
        self.notify_changed();
        id
    }

    pub fn update_position(&self, id: &str, position: Point) {
        let entry = match self.inner.borrow().storage.get(id).cloned() {
            Some(entry) => entry,
            None => return,
        };
        let slot = {
            let inner = self.inner.borrow();
            match inner.slot_nodes.get(id) {
                Some(weak) => Node::upgrade(weak),
                None => None,
            }
        };
        match slot {
            Some(slot_node) => {
                if slot_node.frame().x != position.x || slot_node.frame().y != position.y {
                    let size = Size {
                        width: slot_node.frame().width,
                        height: slot_node.frame().height,
                    };
                    let fitted_position = entry
                        .position_for_size
                        .as_ref()
                        .map(|resolve| resolve(size))
                        .unwrap_or(position);
                    let fitted = PortalPlacement::fit(
                        fitted_position,
                        size,
                        portal_window_bounds(&slot_node),
                        self.anchor(id),
                        entry.fit.viewport_margin,
                        entry.placement,
                    );
                    slot_node.set_frame(fitted);
                }
            }
            None => {
                self.inner.borrow_mut().slot_nodes.remove(id);
            }
        }
        {
            let mut inner = self.inner.borrow_mut();
            let Some(entry) = inner.storage.get_mut(id) else {
                return;
            };
            if entry.position == position {
                return;
            }
            entry.position = position;
        }
        self.notify_changed();
    }

    pub fn configure(
        &self,
        id: &str,
        fills_window: bool,
        placement: PortalPlacementMode,
        parent_id: Option<String>,
        position_for_size: Option<Rc<dyn Fn(Size) -> Point>>,
    ) {
        let changed = {
            let mut inner = self.inner.borrow_mut();
            let Some(entry) = inner.storage.get_mut(id) else {
                return;
            };
            let changed = entry.fills_window != fills_window
                || entry.placement != placement
                || entry.parent_id != parent_id;
            entry.fills_window = fills_window;
            entry.placement = placement;
            entry.parent_id = parent_id;
            entry.position_for_size = position_for_size;
            changed
        };
        if changed {
            self.notify_changed();
        }
    }

    pub fn configure_fit(&self, id: &str, fit: PortalFitConstraints) {
        let changed = {
            let mut inner = self.inner.borrow_mut();
            let Some(entry) = inner.storage.get_mut(id) else {
                return;
            };
            if entry.fit == fit {
                return;
            }
            entry.fit = fit;
            true
        };
        if changed {
            self.notify_changed();
        }
    }

    pub fn anchor(&self, id: &str) -> Option<Rect> {
        self.inner
            .borrow()
            .dismissals
            .get(id)
            .map(|dismissal| (dismissal.anchor)())
    }

    pub fn frame(&self, id: &str) -> Option<Rect> {
        let weak = self.inner.borrow().slot_nodes.get(id).cloned()?;
        Node::upgrade(&weak).map(|node| node.absolute_frame())
    }

    pub fn update_content(&self, id: &str, content: AnyView) {
        {
            let mut inner = self.inner.borrow_mut();
            let Some(entry) = inner.storage.get_mut(id) else {
                return;
            };
            entry.content = content;
        }
        self.notify_changed();
    }

    pub fn update_presentation(
        &self,
        id: &str,
        position: Point,
        width: Option<f32>,
        content: AnyView,
    ) {
        {
            let mut inner = self.inner.borrow_mut();
            let Some(entry) = inner.storage.get_mut(id) else {
                return;
            };
            entry.position = position;
            entry.width = width;
            entry.content = content;
            entry.is_exiting = false;
        }
        self.notify_changed();
    }

    pub fn contains(&self, id: &str) -> bool {
        self.inner.borrow().storage.contains_key(id)
    }

    pub fn begin_dismissal(&self, id: &str, content: AnyView) {
        {
            let mut inner = self.inner.borrow_mut();
            let Some(entry) = inner.storage.get_mut(id) else {
                return;
            };
            if entry.is_exiting {
                return;
            }
            entry.is_exiting = true;
            entry.content = content;
        }
        self.remove_descendants(id);
        self.notify_changed();
    }

    pub fn unregister(&self, id: &str) {
        if self.inner.borrow_mut().storage.remove(id).is_none() {
            return;
        }
        self.remove_descendants(id);
        {
            let mut inner = self.inner.borrow_mut();
            inner.presentation_order.retain(|candidate| candidate != id);
            inner.dismissals.remove(id);
            inner.slot_nodes.remove(id);
        }
        self.notify_changed();
    }

    fn remove_descendants(&self, id: &str) {
        let children: Vec<String> = {
            let inner = self.inner.borrow();
            inner
                .storage
                .values()
                .filter(|entry| entry.parent_id.as_deref() == Some(id))
                .map(|entry| entry.id.clone())
                .collect()
        };
        for child in children {
            self.unregister(&child);
        }
    }

    pub fn permits_child(&self, id: &str) -> bool {
        self.inner
            .borrow()
            .storage
            .get(id)
            .is_some_and(|entry| !entry.is_exiting)
    }

    pub fn clear(&self) {
        {
            let mut inner = self.inner.borrow_mut();
            if inner.storage.is_empty() {
                return;
            }
            inner.storage.clear();
            inner.presentation_order.clear();
            inner.dismissals.clear();
            inner.slot_nodes.clear();
        }
        self.notify_changed();
    }

    pub fn set_dismissal(&self, id: &str, anchor: Rc<dyn Fn() -> Rect>, dismiss: Rc<dyn Fn()>) {
        self.inner
            .borrow_mut()
            .dismissals
            .insert(id.to_owned(), Dismissal { anchor, dismiss });
    }

    pub fn request_dismissal(&self, id: &str) {
        let dismiss = self
            .inner
            .borrow()
            .dismissals
            .get(id)
            .map(|dismissal| dismissal.dismiss.clone());
        if let Some(dismiss) = dismiss {
            dismiss();
        }
    }

    /// Observe the click before normal routing; leave it available to the
    /// underlying control so switching menus works in one click.
    pub fn dismiss_outside(&self, point: Point) {
        let callbacks = {
            let inner = self.inner.borrow();
            let open_ids: Vec<String> = inner
                .presentation_order
                .iter()
                .filter(|id| {
                    inner.dismissals.contains_key(*id)
                        && inner
                            .storage
                            .get(*id)
                            .is_some_and(|entry| !entry.is_exiting)
                })
                .cloned()
                .collect();
            let hit = open_ids.iter().any(|id| {
                inner
                    .dismissals
                    .get(id)
                    .is_some_and(|dismissal| (dismissal.anchor)().contains(point))
                    || inner
                        .slot_nodes
                        .get(id)
                        .and_then(Node::upgrade)
                        .is_some_and(|node| node.absolute_frame().contains(point))
            });
            if hit {
                return;
            }
            open_ids
                .iter()
                .rev()
                .filter_map(|id| {
                    inner
                        .dismissals
                        .get(id)
                        .map(|dismissal| dismissal.dismiss.clone())
                })
                .collect::<Vec<_>>()
        };
        for dismiss in callbacks {
            dismiss();
        }
    }

    pub fn add_observer(&self, observer: Rc<dyn Fn(i64)>) -> u64 {
        let id = next_observer_id();
        self.inner.borrow_mut().observers.insert(id, observer);
        id
    }

    pub fn remove_observer(&self, id: u64) {
        self.inner.borrow_mut().observers.remove(&id);
    }

    pub fn attach_slot_node(&self, id: &str, node: &Node) {
        self.inner
            .borrow_mut()
            .slot_nodes
            .insert(id.to_owned(), node.downgrade());
    }

    fn notify_changed(&self) {
        let (revision, observers) = {
            let mut inner = self.inner.borrow_mut();
            inner.current_revision = inner.current_revision.wrapping_add(1);
            (
                inner.current_revision,
                inner.observers.values().cloned().collect::<Vec<_>>(),
            )
        };
        for observer in observers {
            observer(revision);
        }
    }
}

thread_local! {
    static NEXT_PORTAL_ID: Cell<u64> = const { Cell::new(1) };
    static NEXT_OBSERVER_ID: Cell<u64> = const { Cell::new(1) };
}
fn new_portal_id() -> String {
    NEXT_PORTAL_ID.with(|next| {
        let id = next.get();
        next.set(id + 1);
        format!("portal-{id}")
    })
}
fn next_observer_id() -> u64 {
    NEXT_OBSERVER_ID.with(|next| {
        let id = next.get();
        next.set(id + 1);
        id
    })
}

/// Holds the portal store made "current" by the active window's
/// `PlatformInputContext`. Defaults to a shared instance so single-window hosts
/// and tests that never install a scope keep working unchanged.
pub struct PortalStoreHolder;
thread_local! {
    static SHARED_STORE: Rc<PortalStore> = Rc::new(PortalStore::new());
    static CURRENT_STORE: RefCell<Option<Rc<PortalStore>>> = const { RefCell::new(None) };
}
impl PortalStoreHolder {
    pub fn shared() -> Rc<PortalStore> {
        SHARED_STORE.with(|store| store.clone())
    }
    pub fn current() -> Rc<PortalStore> {
        CURRENT_STORE
            .with(|current| current.borrow().clone())
            .unwrap_or_else(Self::shared)
    }
    pub fn set_current(store: Option<Rc<PortalStore>>) {
        CURRENT_STORE.with(|current| *current.borrow_mut() = store);
    }
}

/// Tree-scoped portal ownership. The application provides one store at each
/// window root, letting presenters and the matching host resolve the same store
/// from node ancestry without depending on global ambient timing.
pub struct PortalStoreEnvironment;
impl PortalStoreEnvironment {
    pub fn key() -> Rc<CompositionLocal<Option<Rc<PortalStore>>>> {
        thread_local! {
            static KEY: Rc<CompositionLocal<Option<Rc<PortalStore>>>> =
                Rc::new(CompositionLocal::constant(None));
        }
        KEY.with(|key| key.clone())
    }
}

/// Adapts a per-window `PortalStore` to the runtime's scoped ambient hook so it
/// is swapped in lockstep with that window's input registries.
pub struct PortalStoreAmbient {
    pub store: Rc<PortalStore>,
}
impl PortalStoreAmbient {
    pub fn new(store: Rc<PortalStore>) -> Self {
        Self { store }
    }
    pub fn activate(&self) -> Rc<dyn Fn()> {
        let previous = PortalStoreHolder::current();
        PortalStoreHolder::set_current(Some(self.store.clone()));
        Rc::new(move || PortalStoreHolder::set_current(Some(previous.clone())))
    }
}

/// The runtime swaps scoped ambients through this trait, so the portal store can
/// travel with a window's input context. Without this impl the store was only
/// swappable by hand, and callers had to reimplement it.
impl cove_core::scene::ScopedAmbient for PortalStoreAmbient {
    fn activate(&self) -> Box<dyn FnOnce()> {
        let previous = PortalStoreHolder::current();
        PortalStoreHolder::set_current(Some(self.store.clone()));
        Box::new(move || PortalStoreHolder::set_current(Some(previous)))
    }
}

fn animated_visibility(
    is_visible: bool,
    transition: Transition,
    animate_on_mount: bool,
    on_visibility_settled: Option<Rc<dyn Fn(bool)>>,
    content: AnyView,
) -> AnyView {
    AnyView::new(AnimatedVisibility::with_motion(
        is_visible,
        transition,
        Some(SemanticMotionRef::entering()),
        animate_on_mount,
        on_visibility_settled,
        content,
    ))
}

/// A portal-store entry owned by the lifetime of the node that presents it.
/// `present` registers or updates the entry, and `unmount` — invoked by
/// `Node::remove_child` when the presenter's subtree leaves the tree for ANY
/// reason — unregisters it. Cleanup never depends on a modifier side-effect
/// running, so a torn-down-while-open popover cannot leak a phantom overlay
/// that swallows later clicks.
pub struct PortalResource {
    /// Weak handle to this resource for exit-animation callbacks. A weak self
    /// reference cannot form a cycle.
    me: RefCell<Weak<PortalResource>>,
    entry_id: RefCell<Option<String>>,
    /// Weak so a lingering resource never keeps a torn-down window's store
    /// alive; cleanup always targets the store it registered with.
    store: RefCell<Option<Weak<PortalStore>>>,
    content: RefCell<Option<AnyView>>,
    transition: RefCell<Option<Transition>>,
    is_dismissing: Cell<bool>,
    generation: Cell<u64>,
    owner: RefCell<Option<NodeWeak>>,
}
impl Default for PortalResource {
    fn default() -> Self {
        Self {
            me: RefCell::new(Weak::new()),
            entry_id: RefCell::new(None),
            store: RefCell::new(None),
            content: RefCell::new(None),
            transition: RefCell::new(None),
            is_dismissing: Cell::new(false),
            generation: Cell::new(0),
            owner: RefCell::new(None),
        }
    }
}
impl PortalResource {
    pub fn new() -> Rc<Self> {
        let resource = Rc::new(Self::default());
        *resource.me.borrow_mut() = Rc::downgrade(&resource);
        resource
    }
    pub fn entry_id(&self) -> Option<String> {
        self.entry_id.borrow().clone()
    }
    fn owner_node(&self) -> Option<Node> {
        self.owner.borrow().as_ref().and_then(Node::upgrade)
    }
    fn store(&self) -> Option<Rc<PortalStore>> {
        self.store.borrow().as_ref().and_then(Weak::upgrade)
    }

    /// Register the overlay entry, or update it in place if already live.
    /// Re-registers if a prior entry was unregistered (for example this
    /// resource's node was reused after a close), so the overlay reliably
    /// reappears. The store is the scoped ambient at present time.
    pub fn present(
        &self,
        resolved_store: Option<Rc<PortalStore>>,
        position: Point,
        width: Option<f32>,
        content: AnyView,
        fills_window: bool,
        placement: PortalPlacementMode,
        position_for_size: Option<Rc<dyn Fn(Size) -> Point>>,
        transition: Option<Transition>,
    ) {
        let current = resolved_store.unwrap_or_else(PortalStoreHolder::current);
        let owner = self.owner_node();
        let mut parent_id = None;
        let mut ancestor = owner.as_ref().and_then(Node::parent);
        while let Some(candidate) = ancestor {
            if let Some(id) = candidate.attachment::<String>(PortalOwnership::ENTRY_KEY) {
                parent_id = Some(id);
                break;
            }
            ancestor = candidate.parent();
        }
        if let Some(parent_id) = parent_id.as_deref()
            && !current.permits_child(parent_id)
        {
            if let Some(owner) = owner {
                self.unmount(&owner);
            }
            return;
        }
        if let Some(previous) = self.store()
            && !Rc::ptr_eq(&previous, &current)
            && let Some(id) = self.entry_id()
        {
            // The owning node moved to a different window's tree (for example a
            // panel re-hosted): release the entry from the old store first.
            previous.unregister(&id);
            *self.entry_id.borrow_mut() = None;
        }
        *self.store.borrow_mut() = Some(Rc::downgrade(&current));

        let theme = owner
            .as_ref()
            .map(crate::theme::theme_for_node)
            .unwrap_or_else(Theme::default_dark);
        let scaler = owner
            .as_ref()
            .map(crate::theme::node_text_scaler)
            .unwrap_or_default();
        let environment = owner
            .as_ref()
            .map(|node| node.composition_value(&SurfaceEnvironment::key()))
            .unwrap_or_default();
        let reduced_motion = owner.as_ref().is_some_and(Node::prefers_reduced_motion);
        let scaling = owner
            .as_ref()
            .and_then(crate::theme::node_icon_scaling_override)
            .unwrap_or(theme.icons.scaling);
        let content = AnyView::new(
            content
                .theme(theme.clone())
                .text_scaler(scaler)
                .surface_environment(environment)
                .icon_scaling(scaling)
                .reduced_motion(reduced_motion),
        );
        *self.content.borrow_mut() = Some(content.clone());
        *self.transition.borrow_mut() = transition;
        self.is_dismissing.set(false);
        self.generation.set(self.generation.get().wrapping_add(1));
        let presented = match transition {
            Some(effect) => AnyView::new(
                animated_visibility(true, effect, true, None, content)
                    .reduced_motion(reduced_motion),
            ),
            None => content,
        };
        let existing = self.entry_id.borrow().clone();
        match existing {
            Some(id) if current.contains(&id) => {
                current.update_presentation(&id, position, width, presented);
            }
            _ => {
                *self.entry_id.borrow_mut() =
                    Some(current.register(None, position, width, presented));
            }
        }
        if let Some(id) = self.entry_id.borrow().clone() {
            current.configure(&id, fills_window, placement, parent_id, position_for_size);
        }
    }

    /// Shared reveal size for menus and popovers. The caller supplies the
    /// trigger corner; transition scale leaves elevation lengths unchanged.
    pub fn surface_reveal(anchor: Point, theme: &Theme) -> Transition {
        let scale = theme.motion.surface.reveal_scale;
        let resolved = if scale.is_finite() {
            scale.clamp(0.0, 1.0)
        } else {
            crate::theme::SurfaceMotion::default().reveal_scale
        };
        Transition::scale(resolved, anchor)
    }

    /// Logical dismissal is immediate. Only noninteractive paint survives the
    /// short exit; node teardown still unregisters synchronously.
    pub fn dismiss(&self, node: &Node) {
        let Some(id) = self.entry_id.borrow().clone() else {
            return;
        };
        let Some(store) = self.store() else { return };
        if self.is_dismissing.get() {
            return;
        }
        let (Some(content), Some(transition)) =
            (self.content.borrow().clone(), *self.transition.borrow())
        else {
            self.unmount(node);
            return;
        };
        if node.prefers_reduced_motion() {
            self.unmount(node);
            return;
        }
        self.is_dismissing.set(true);
        self.generation.set(self.generation.get().wrapping_add(1));
        let expected = self.generation.get();
        let owner = self.owner.borrow().clone();
        // The settled callback holds the resource weakly so a torn-down window
        // cannot keep it (or its store) alive.
        let weak_self = self.self_weak();
        let settled: Rc<dyn Fn(bool)> = Rc::new(move |visible: bool| {
            if visible {
                return;
            }
            let Some(resource) = weak_self.upgrade() else {
                return;
            };
            if resource.generation.get() != expected {
                return;
            }
            let Some(node) = owner.as_ref().and_then(Node::upgrade) else {
                return;
            };
            resource.unmount(&node);
        });
        let hidden = AnyView::new(
            animated_visibility(false, transition, true, Some(settled), content)
                .reduced_motion(node.prefers_reduced_motion()),
        );
        store.begin_dismissal(&id, hidden);
    }

    /// Resources cannot produce a weak handle to themselves; callers that need
    /// the settled callback use this hook, set by `new`.
    fn self_weak(&self) -> Weak<PortalResource> {
        self.me.borrow().clone()
    }

    pub fn update_position(&self, position: Point) {
        let Some(owner) = self.owner_node() else {
            return;
        };
        if !owner.accepts_subtree_input() {
            if let Some(id) = self.entry_id.borrow().clone()
                && let Some(store) = self.store()
            {
                store.request_dismissal(&id);
            }
            self.unmount(&owner);
            return;
        }
        if let Some(id) = self.entry_id.borrow().clone()
            && let Some(store) = self.store()
        {
            store.update_position(&id, position);
        }
    }

    pub fn update_fit(&self, fit: PortalFitConstraints) {
        if let Some(id) = self.entry_id.borrow().clone()
            && let Some(store) = self.store()
        {
            store.configure_fit(&id, fit);
        }
    }

    pub fn frame(&self) -> Option<Rect> {
        let id = self.entry_id.borrow().clone()?;
        self.store().and_then(|store| store.frame(&id))
    }

    pub fn set_dismissal(&self, anchor: Rc<dyn Fn() -> Rect>, dismiss: Rc<dyn Fn()>) {
        if let Some(id) = self.entry_id.borrow().clone()
            && let Some(store) = self.store()
        {
            store.set_dismissal(&id, anchor, dismiss);
        }
    }
}
impl NodeResource for PortalResource {
    fn mount(&self, node: &Node) {
        *self.owner.borrow_mut() = Some(node.downgrade());
    }

    /// Idempotent: safe to call more than once during teardown.
    fn unmount(&self, _node: &Node) {
        if let Some(id) = self.entry_id.borrow_mut().take()
            && let Some(store) = self.store()
        {
            store.unregister(&id);
        }
        *self.store.borrow_mut() = None;
        *self.content.borrow_mut() = None;
        self.generation.set(self.generation.get().wrapping_add(1));
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn std::any::Any> {
        self
    }
}

pub struct PortalOwnership;
impl PortalOwnership {
    pub const ENTRY_KEY: &'static str = "__portal.entry";
}
