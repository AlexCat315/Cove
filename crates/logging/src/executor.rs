//! Translation of `Logging/Sources/CoveLogging/Runtime/Executor.swift`.
//!
//! `DispatchQueue` becomes one worker thread with a queue and condvar. Logging
//! from inside the executor runs inline, which is what `DispatchSpecificKey`
//! provides in Swift.
use std::cell::Cell;
use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;

type Job = Box<dyn FnOnce() + Send + 'static>;

thread_local! {
    static IS_CURRENT: Cell<bool> = const { Cell::new(false) };
}

struct Shared {
    queue: Mutex<VecDeque<Job>>,
    signal: Condvar,
    stopped: Mutex<bool>,
}

pub struct LogExecutor {
    shared: Arc<Shared>,
    worker: Mutex<Option<JoinHandle<()>>>,
}
impl LogExecutor {
    pub fn new() -> Self {
        let shared = Arc::new(Shared {
            queue: Mutex::new(VecDeque::new()),
            signal: Condvar::new(),
            stopped: Mutex::new(false),
        });
        let worker_shared = shared.clone();
        let worker = std::thread::Builder::new()
            .name("com.cove.logging".into())
            .spawn(move || {
                loop {
                    let job = {
                        let mut queue = worker_shared
                            .queue
                            .lock()
                            .unwrap_or_else(|e| e.into_inner());
                        while queue.is_empty() {
                            if *worker_shared
                                .stopped
                                .lock()
                                .unwrap_or_else(|e| e.into_inner())
                            {
                                return;
                            }
                            queue = worker_shared
                                .signal
                                .wait(queue)
                                .unwrap_or_else(|e| e.into_inner());
                        }
                        queue.pop_front()
                    };
                    if let Some(job) = job {
                        IS_CURRENT.with(|current| current.set(true));
                        job();
                        IS_CURRENT.with(|current| current.set(false));
                    }
                }
            })
            .ok();
        Self {
            shared,
            worker: Mutex::new(worker),
        }
    }

    pub fn is_current(&self) -> bool {
        IS_CURRENT.with(Cell::get)
    }

    /// Runs inline on the worker or submits and waits for completion.
    pub fn sync<T, F>(&self, body: F) -> T
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        if self.is_current() {
            return body();
        }
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        self.enqueue(Box::new(move || {
            let _ = sender.send(body());
        }));
        receiver
            .recv()
            .expect("log executor stopped before completing work")
    }

    /// Fire-and-forget. The closure must not borrow the executor.
    pub fn r#async<F>(&self, body: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.enqueue(Box::new(body));
    }

    fn enqueue(&self, job: Job) {
        if self.is_current() {
            job();
            return;
        }
        self.shared
            .queue
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push_back(job);
        self.shared.signal.notify_one();
    }
}

impl Default for LogExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for LogExecutor {
    fn drop(&mut self) {
        // The worker drains jobs queued before teardown; the runtime owns this
        // executor, so no caller can enqueue concurrently after Drop starts.
        *self
            .shared
            .stopped
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = true;
        self.shared.signal.notify_all();
        if let Some(worker) = self.worker.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = worker.join();
        }
    }
}
