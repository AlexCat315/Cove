//! Translation of CoveUIScene/Recomposer.swift.
//!
//! Batches and deduplicates composition-scope recomposes within a single frame.
//!
//! Data flow:
//! 1. A `@State` write fires `StateStorage.onChange`.
//! 2. The owning scope calls `recomposer.invalidate(scope_id, body)` on the
//!    `Recomposer` instance owned by the host.
//! 3. Further calls with the same `scope_id` coalesce, retaining distinct
//!    causes.
//! 4. The platform host calls `recomposer.commit_all()` at frame start,
//!    executing pending recomposes and any child-scope recomposes they queue
//!    before layout/draw for that frame.
//!
//! Each platform host (and therefore each window) owns its own `Recomposer`
//! instance — see blueprint §9.4 windowing strategy.
//!
//! Swift guards the queue with an `NSLock`; the scene is single-threaded, so the
//! port keeps the same re-entrancy contract with a `RefCell` whose borrow is
//! always released before a scope body runs.
use super::{
    ActiveAnimationContext, Animation, AnimationCompletionGroup, PerformanceTimeline,
    RecompositionReason,
};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// Identity of a composition scope. Swift uses `ObjectIdentifier`; the Rust
/// port lets each scope own a stable token.
pub type ScopeId = usize;

/// Renders one composition scope for the reasons that invalidated it.
pub type ScopeBody = Rc<dyn Fn(&[RecompositionReason])>;

struct PendingScope {
    id: ScopeId,
    body: ScopeBody,
    reasons: Vec<RecompositionReason>,
    /// Animation captured at write time. Re-established by `commit_all` before
    /// invoking `body` so modifier `apply` paths observe the same animation
    /// that the user authored at the call site.
    animation: Option<Animation>,
    /// The `with_animation` completion waiting on that animation, carried the
    /// same way so every controller this scope registers is accounted for by
    /// the same callback.
    completion: Option<Rc<AnimationCompletionGroup>>,
    disables_animations: bool,
}

#[derive(Default)]
struct Queue {
    pending_order: Vec<ScopeId>,
    pending_by_id: HashMap<ScopeId, PendingScope>,
    scan_index: usize,
}

#[derive(Default)]
pub struct Recomposer {
    queue: RefCell<Queue>,
    timeline: RefCell<Option<Rc<RefCell<PerformanceTimeline>>>>,
}

impl Recomposer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_timeline(&self, timeline: Option<Rc<RefCell<PerformanceTimeline>>>) {
        *self.timeline.borrow_mut() = timeline;
    }

    // MARK: - Invalidation

    /// Schedule a recompose for `scope_id`.
    ///
    /// If `scope_id` is already queued the first body/animation is retained.
    /// Queued callbacks commit at most once per frame regardless of how many
    /// state writes occur (parent reconciliation can also evaluate child
    /// bodies).
    pub fn invalidate(
        &self,
        scope_id: ScopeId,
        animation: Option<Animation>,
        completion: Option<Rc<AnimationCompletionGroup>>,
        body: impl Fn() + 'static,
    ) {
        self.invalidate_tracked(
            scope_id,
            animation,
            completion,
            false,
            RecompositionReason::new("manual"),
            move |_| body(),
        );
    }

    /// Preserve up to 16 distinct causes while keeping existing scheduling and
    /// first-animation semantics. Self-invalidations retain next-frame causes.
    pub fn invalidate_tracked(
        &self,
        scope_id: ScopeId,
        animation: Option<Animation>,
        completion: Option<Rc<AnimationCompletionGroup>>,
        disables_animations: bool,
        reason: RecompositionReason,
        body: impl Fn(&[RecompositionReason]) + 'static,
    ) {
        let mut queue = self.queue.borrow_mut();
        if let Some(queued) = queue.pending_by_id.get_mut(&scope_id) {
            if queued.reasons.len() < 16 && !queued.reasons.contains(&reason) {
                queued.reasons.push(reason);
            }
            return;
        }
        queue.pending_by_id.insert(
            scope_id,
            PendingScope {
                id: scope_id,
                body: Rc::new(body),
                reasons: vec![reason],
                animation,
                completion,
                disables_animations,
            },
        );
        queue.pending_order.push(scope_id);
    }

    // MARK: - Commit

    /// Execute all pending recomposes in registration order, draining child
    /// invalidations queued by those recomposes before returning.
    ///
    /// A scope that invalidates itself while it is already being committed is
    /// left queued for the next frame; different child scopes are committed in
    /// the current frame so parent-driven environment changes settle before
    /// layout/draw.
    pub fn commit_all(&self) -> bool {
        let timeline = self.timeline.borrow().clone();
        let trace = timeline
            .as_ref()
            .and_then(|timeline| timeline.borrow().begin());
        let mut committed: HashSet<ScopeId> = HashSet::new();
        let mut did_commit = false;

        loop {
            // The queue borrow is released before the body runs, so a body may
            // invalidate child scopes (or itself) re-entrantly.
            let scope = {
                let mut queue = self.queue.borrow_mut();
                loop {
                    if queue.scan_index >= queue.pending_order.len() {
                        break None;
                    }
                    let id = queue.pending_order[queue.scan_index];
                    queue.scan_index += 1;
                    if !queue.pending_by_id.contains_key(&id) {
                        continue;
                    }
                    if committed.contains(&id) {
                        continue;
                    }
                    break queue.pending_by_id.remove(&id);
                }
            };
            let Some(scope) = scope else { break };
            did_commit = true;
            committed.insert(scope.id);
            let body = scope.body.clone();
            let reasons = scope.reasons.clone();
            ActiveAnimationContext::with(
                scope.animation,
                scope.completion.clone(),
                Some(scope.disables_animations),
                || {
                    body(&reasons);
                },
            );
        }

        self.compact_pending_queue();
        if did_commit && let Some(timeline) = timeline {
            timeline
                .borrow_mut()
                .end(trace, "recomposition", "Commit pending scopes", None, &[]);
        }
        did_commit
    }

    /// `true` when there are pending recomposes queued for the next
    /// `commit_all()`.
    pub fn has_pending(&self) -> bool {
        !self.queue.borrow().pending_by_id.is_empty()
    }

    /// Number of scopes currently queued.
    pub fn pending_count(&self) -> usize {
        self.queue.borrow().pending_by_id.len()
    }

    fn compact_pending_queue(&self) {
        let mut queue = self.queue.borrow_mut();
        if queue.scan_index == 0 {
            return;
        }
        let pending = std::mem::take(&mut queue.pending_by_id);
        queue.pending_order.retain(|id| pending.contains_key(id));
        queue.pending_by_id = pending;
        queue.scan_index = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn duplicate_invalidations_coalesce_but_keep_causes() {
        let recomposer = Recomposer::new();
        let runs = Rc::new(Cell::new(0));
        for index in 0..3 {
            let counter = runs.clone();
            recomposer.invalidate_tracked(
                7,
                None,
                None,
                false,
                RecompositionReason::new(format!("cause{index}")),
                move |_| counter.set(counter.get() + 1),
            );
        }
        assert_eq!(recomposer.pending_count(), 1);
        assert!(recomposer.commit_all());
        assert_eq!(runs.get(), 1);
        assert!(!recomposer.has_pending());
        assert!(!recomposer.commit_all());
    }

    #[test]
    fn child_scopes_commit_in_the_same_frame() {
        let recomposer = Rc::new(Recomposer::new());
        let child_ran = Rc::new(Cell::new(false));
        let queueing = recomposer.clone();
        let child = child_ran.clone();
        recomposer.invalidate(1, None, None, move || {
            let child = child.clone();
            queueing.invalidate(2, None, None, move || child.set(true));
        });
        assert!(recomposer.commit_all());
        assert!(child_ran.get());
        assert!(!recomposer.has_pending());
    }

    #[test]
    fn self_invalidation_is_deferred_to_the_next_frame() {
        let recomposer = Rc::new(Recomposer::new());
        let runs = Rc::new(Cell::new(0));
        let requeue = recomposer.clone();
        let counter = runs.clone();
        recomposer.invalidate(1, None, None, move || {
            counter.set(counter.get() + 1);
            if counter.get() == 1 {
                let counter = counter.clone();
                requeue.invalidate(1, None, None, move || counter.set(counter.get() + 1));
            }
        });
        assert!(recomposer.commit_all());
        assert_eq!(runs.get(), 1);
        assert!(recomposer.has_pending());
        assert!(recomposer.commit_all());
        assert_eq!(runs.get(), 2);
    }
}
