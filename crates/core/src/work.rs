//! Shared worker pool for work that must not run on the UI thread.
//!
//! The UI thread keeps the composition tree, signals and layout (`!Send`).
//! Decoding, shaping and rasterizing run here and hand their results back with
//! [`spawn_then`] or a [`UiSender`]: the result is posted to the originating
//! thread's inbox and applied when that thread drains it at the start of its
//! next frame, so the UI hot path takes no locks.
//!
//! [`par_map`] splits a slice across the pool for per-frame data parallelism.
//! The calling thread works through the queue while it waits, so it is safe to
//! call from inside a worker.
use crate::reactive::ThreadInbox;
use std::any::Any;
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock, PoisonError};
use std::thread;

type Job = Box<dyn FnOnce() + Send + 'static>;

/// Jobs queued or running, for [`wait_idle`].
static OUTSTANDING: AtomicUsize = AtomicUsize::new(0);

struct Queue {
    jobs: Mutex<VecDeque<Job>>,
    available: Condvar,
}

impl Queue {
    fn push(&self, job: Job) {
        self.jobs
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push_back(job);
        self.available.notify_one();
    }

    fn try_pop(&self) -> Option<Job> {
        self.jobs
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop_front()
    }

    fn pop_blocking(&self) -> Job {
        let mut jobs = self.jobs.lock().unwrap_or_else(PoisonError::into_inner);
        loop {
            if let Some(job) = jobs.pop_front() {
                return job;
            }
            jobs = self
                .available
                .wait(jobs)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }
}

/// A fixed set of named worker threads sharing one FIFO queue.
pub struct WorkPool {
    queue: Arc<Queue>,
    threads: usize,
}

impl WorkPool {
    fn new(threads: usize) -> Self {
        let queue = Arc::new(Queue {
            jobs: Mutex::new(VecDeque::new()),
            available: Condvar::new(),
        });
        for index in 0..threads {
            let queue = queue.clone();
            thread::Builder::new()
                .name(format!("cove-worker-{index}"))
                .spawn(move || {
                    loop {
                        let job = queue.pop_blocking();
                        // A panicking job must not take its worker down with it.
                        let _ = catch_unwind(AssertUnwindSafe(job));
                    }
                })
                .expect("the worker pool could not start a thread");
        }
        Self { queue, threads }
    }

    /// Worker threads in the pool.
    pub fn threads(&self) -> usize {
        self.threads
    }

    pub fn spawn(&self, job: impl FnOnce() + Send + 'static) {
        OUTSTANDING.fetch_add(1, Ordering::AcqRel);
        self.queue.push(Box::new(move || {
            struct Finished;
            impl Drop for Finished {
                fn drop(&mut self) {
                    OUTSTANDING.fetch_sub(1, Ordering::AcqRel);
                }
            }
            let _finished = Finished;
            job();
        }));
    }

    /// Run one queued job on the calling thread, if there is one.
    fn help(&self) -> bool {
        match self.queue.try_pop() {
            Some(job) => {
                let _ = catch_unwind(AssertUnwindSafe(job));
                true
            }
            None => false,
        }
    }
}

/// The process-wide pool: one thread per core, leaving one for the UI thread.
pub fn pool() -> &'static WorkPool {
    static POOL: OnceLock<WorkPool> = OnceLock::new();
    POOL.get_or_init(|| {
        let cores = thread::available_parallelism().map_or(2, usize::from);
        WorkPool::new(cores.saturating_sub(1).max(1))
    })
}

/// Run `job` on the pool.
pub fn spawn(job: impl FnOnce() + Send + 'static) {
    pool().spawn(job);
}

/// Block until every job spawned so far has finished, or `timeout` passes.
/// Returns whether the pool went idle. For hosts flushing before teardown
/// and for tests that need background results deterministically.
pub fn wait_idle(timeout: std::time::Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    while OUTSTANDING.load(Ordering::Acquire) > 0 {
        if std::time::Instant::now() >= deadline {
            return false;
        }
        if !pool().help() {
            thread::sleep(std::time::Duration::from_micros(50));
        }
    }
    true
}

// MARK: - Completions on the UI thread

thread_local! {
    static CALLBACKS: RefCell<HashMap<u64, Box<dyn Any>>> = RefCell::new(HashMap::new());
}

fn next_callback_id() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// Run `job` on the pool and `then` with its result on the calling thread,
/// when that thread next drains its inbox.
pub fn spawn_then<R: Send + 'static>(
    job: impl FnOnce() -> R + Send + 'static,
    then: impl FnOnce(R) + 'static,
) {
    let id = next_callback_id();
    let callback: Box<dyn FnOnce(R)> = Box::new(then);
    CALLBACKS.with(|callbacks| callbacks.borrow_mut().insert(id, Box::new(callback)));
    let inbox = ThreadInbox::current();
    spawn(move || {
        let result = job();
        inbox.post(Box::new(move || {
            let callback = CALLBACKS.with(|callbacks| callbacks.borrow_mut().remove(&id));
            if let Some(callback) = callback
                && let Ok(callback) = callback.downcast::<Box<dyn FnOnce(R)>>()
            {
                callback(result);
            }
        }));
    });
}

/// A `Send` handle that delivers values to a callback living on the thread
/// that created it. Dropping the last [`UiReceiver`] releases the callback.
pub struct UiSender<T: Send + 'static> {
    id: u64,
    inbox: ThreadInbox,
    _value: std::marker::PhantomData<fn(T)>,
}

impl<T: Send + 'static> Clone for UiSender<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            inbox: self.inbox.clone(),
            _value: std::marker::PhantomData,
        }
    }
}

impl<T: Send + 'static> UiSender<T> {
    pub fn send(&self, value: T) {
        let id = self.id;
        self.inbox.post(Box::new(move || {
            let callback = CALLBACKS.with(|callbacks| {
                callbacks
                    .borrow()
                    .get(&id)
                    .and_then(|callback| callback.downcast_ref::<std::rc::Rc<dyn Fn(T)>>())
                    .cloned()
            });
            if let Some(callback) = callback {
                callback(value);
            }
        }));
    }
}

/// Keeps a [`UiSender`]'s callback registered; dropping it ignores later sends.
pub struct UiReceiver {
    id: u64,
}

impl Drop for UiReceiver {
    fn drop(&mut self) {
        let _ = CALLBACKS.try_with(|callbacks| callbacks.borrow_mut().remove(&self.id));
    }
}

/// Register `callback` on the calling thread and return a sender for it.
pub fn ui_channel<T: Send + 'static>(callback: impl Fn(T) + 'static) -> (UiSender<T>, UiReceiver) {
    let id = next_callback_id();
    let callback: std::rc::Rc<dyn Fn(T)> = std::rc::Rc::new(callback);
    CALLBACKS.with(|callbacks| callbacks.borrow_mut().insert(id, Box::new(callback)));
    (
        UiSender {
            id,
            inbox: ThreadInbox::current(),
            _value: std::marker::PhantomData,
        },
        UiReceiver { id },
    )
}

// MARK: - Data parallelism

/// Below this many items the work runs inline: queueing would cost more.
const PARALLEL_THRESHOLD: usize = 4;

/// Map `items` through `transform` across the pool, preserving order.
///
/// Runners claim small ranges as they go rather than owning a fixed share, so
/// a thread on a slower core or one that starts late does not hold up the rest.
pub fn par_map<T: Sync, R: Send, F: Fn(&T) -> R + Sync>(items: &[T], transform: F) -> Vec<R> {
    let pool = pool();
    if items.len() < PARALLEL_THRESHOLD || pool.threads() < 2 {
        return items.iter().map(transform).collect();
    }
    let runners = (pool.threads() + 1).min(items.len());
    let grain = (items.len() / (runners * 8)).max(1);

    struct Shared<'a, T, R, F> {
        items: &'a [T],
        results: &'a [Mutex<Option<R>>],
        transform: &'a F,
        grain: usize,
        next: AtomicUsize,
        pending: AtomicUsize,
        done: (Mutex<()>, Condvar),
        panic: Mutex<Option<Box<dyn Any + Send>>>,
    }
    let slots: Vec<Mutex<Option<R>>> = (0..items.len()).map(|_| Mutex::new(None)).collect();
    let shared = Shared {
        items,
        results: &slots,
        transform: &transform,
        grain,
        next: AtomicUsize::new(0),
        pending: AtomicUsize::new(runners),
        done: (Mutex::new(()), Condvar::new()),
        panic: Mutex::new(None),
    };

    fn run<T: Sync, R: Send, F: Fn(&T) -> R + Sync>(shared: &Shared<'_, T, R, F>) {
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            loop {
                let start = shared.next.fetch_add(shared.grain, Ordering::Relaxed);
                if start >= shared.items.len() {
                    break;
                }
                let end = (start + shared.grain).min(shared.items.len());
                for index in start..end {
                    let value = (shared.transform)(&shared.items[index]);
                    *shared.results[index]
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner) = Some(value);
                }
            }
        }));
        if let Err(payload) = outcome {
            shared
                .panic
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .get_or_insert(payload);
        }
        if shared.pending.fetch_sub(1, Ordering::AcqRel) == 1 {
            let _guard = shared.done.0.lock().unwrap_or_else(PoisonError::into_inner);
            shared.done.1.notify_all();
        }
    }

    // The calling thread is one runner; the rest go to the pool.
    for _ in 1..runners {
        let address = &shared as *const Shared<'_, T, R, F> as usize;
        let job: Box<dyn FnOnce() + Send + '_> = Box::new(move || {
            // SAFETY: `shared` lives on the caller's stack until `pending`
            // reaches zero, and the caller does not return before that, so the
            // pointer stays valid for the whole job.
            let shared = unsafe { &*(address as *const Shared<'_, T, R, F>) };
            run(shared);
        });
        // SAFETY: the job only borrows data the caller keeps alive until every
        // runner has finished (the wait below), so extending its lifetime to
        // `'static` for the queue cannot let it outlive what it borrows.
        let job: Job = unsafe { std::mem::transmute::<Box<dyn FnOnce() + Send + '_>, Job>(job) };
        pool.queue.push(job);
    }
    run(&shared);
    while shared.pending.load(Ordering::Acquire) > 0 {
        // Keep the queue moving instead of idling; this also makes nested
        // calls from inside a worker safe.
        if !pool.help() {
            let guard = shared.done.0.lock().unwrap_or_else(PoisonError::into_inner);
            if shared.pending.load(Ordering::Acquire) > 0 {
                let _ = shared
                    .done
                    .1
                    .wait_timeout(guard, std::time::Duration::from_micros(200));
            }
        }
    }
    if let Some(payload) = shared
        .panic
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take()
    {
        resume_unwind(payload);
    }
    slots
        .into_iter()
        .map(|slot| {
            slot.into_inner()
                .unwrap_or_else(PoisonError::into_inner)
                .expect("every item has a result")
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reactive::drain_posted_signal_updates;
    use std::cell::Cell;
    use std::rc::Rc;
    use std::time::{Duration, Instant};

    fn drain_until(done: impl Fn() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !done() {
            assert!(Instant::now() < deadline, "the posted work never arrived");
            drain_posted_signal_updates();
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    #[test]
    fn spawn_then_delivers_on_the_calling_thread() {
        let caller = std::thread::current().id();
        let seen = Rc::new(Cell::new(None));
        let sink = seen.clone();
        spawn_then(
            || (std::thread::current().id(), 21 * 2),
            move |(worker, value)| {
                assert_eq!(std::thread::current().id(), caller);
                sink.set(Some((worker != caller, value)));
            },
        );
        drain_until(|| seen.get().is_some());
        assert_eq!(seen.get(), Some((true, 42)));
    }

    #[test]
    fn a_ui_sender_stops_after_its_receiver_drops() {
        let total = Rc::new(Cell::new(0));
        let sink = total.clone();
        let (sender, receiver) = ui_channel(move |value: i32| sink.set(sink.get() + value));
        let worker = sender.clone();
        std::thread::spawn(move || worker.send(5)).join().unwrap();
        drain_until(|| total.get() == 5);
        drop(receiver);
        sender.send(7);
        drain_posted_signal_updates();
        assert_eq!(total.get(), 5);
    }

    #[test]
    fn par_map_preserves_order_and_uses_workers() {
        let items: Vec<u64> = (0..1_000).collect();
        let squares = par_map(&items, |value| value * value);
        assert_eq!(squares, items.iter().map(|value| value * value).collect::<Vec<_>>());
        let threads = par_map(&items, |_| std::thread::current().id());
        let distinct: std::collections::HashSet<_> = threads.into_iter().collect();
        assert!(distinct.len() > 1 || pool().threads() < 2);
    }

    #[test]
    fn nested_par_map_does_not_deadlock() {
        let outer: Vec<usize> = (0..16).collect();
        let sums = par_map(&outer, |&row| {
            let inner: Vec<usize> = (0..64).collect();
            par_map(&inner, |&column| row * column).into_iter().sum::<usize>()
        });
        assert_eq!(sums[2], (0..64).map(|column| 2 * column).sum::<usize>());
    }

    #[test]
    fn a_panicking_chunk_propagates_to_the_caller() {
        let items: Vec<u32> = (0..64).collect();
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            par_map(&items, |&value| {
                assert!(value != 40, "boom");
                value
            })
        }));
        assert!(outcome.is_err());
        assert_eq!(par_map(&items, |&value| value + 1)[0], 1, "the pool survives");
    }
}
