//! Runs a platform web-view host on the platform thread for a view composed on
//! the UI thread. Calls are posted in order and state changes come back as posted
//! work, so neither thread waits for the other.
use super::{
    WebViewCommand, WebViewGeometry, WebViewHost, WebViewRequest, WebViewState, WebViewSurface,
};
use cove_core::ThreadPoster;
use cove_core::scene::PlatformThread;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

type StateCallback = Rc<dyn Fn(WebViewState)>;

thread_local! {
    // Platform-thread side: hosts and surfaces are bound to this thread.
    static HOSTS: RefCell<HashMap<u64, Box<dyn WebViewHost>>> = RefCell::default();
    static SURFACES: RefCell<HashMap<u64, Box<dyn WebViewSurface>>> = RefCell::default();
    // UI-thread side.
    static CALLBACKS: RefCell<HashMap<u64, StateCallback>> = RefCell::default();
}

fn next_id() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

fn with_surface(id: u64, body: impl FnOnce(&mut dyn WebViewSurface) + Send + 'static) {
    PlatformThread::post(Box::new(move || {
        SURFACES.with(|surfaces| {
            if let Some(surface) = surfaces.borrow_mut().get_mut(&id) {
                body(surface.as_mut());
            }
        });
    }));
}

/// A [`WebViewHost`] whose real host is built by `make` on the platform thread, so
/// it may hold native window references.
pub struct PlatformThreadWebViewHost {
    id: u64,
}

impl PlatformThreadWebViewHost {
    pub fn new(make: impl FnOnce() -> Box<dyn WebViewHost> + Send + 'static) -> Self {
        let id = next_id();
        PlatformThread::post(Box::new(move || {
            HOSTS.with(|hosts| hosts.borrow_mut().insert(id, make()));
        }));
        Self { id }
    }
}

impl Drop for PlatformThreadWebViewHost {
    fn drop(&mut self) {
        let id = self.id;
        PlatformThread::post(Box::new(move || {
            HOSTS.with(|hosts| hosts.borrow_mut().remove(&id));
        }));
    }
}

impl WebViewHost for PlatformThreadWebViewHost {
    fn make_surface(&self) -> Box<dyn WebViewSurface> {
        let id = next_id();
        let host = self.id;
        PlatformThread::post(Box::new(move || {
            let surface = HOSTS.with(|hosts| hosts.borrow().get(&host).map(|h| h.make_surface()));
            if let Some(surface) = surface {
                SURFACES.with(|surfaces| surfaces.borrow_mut().insert(id, surface));
            }
        }));
        Box::new(RemoteSurface { id })
    }
}

struct RemoteSurface {
    id: u64,
}

impl WebViewSurface for RemoteSurface {
    fn set_on_state_change(&mut self, callback: Option<Box<dyn Fn(WebViewState)>>) {
        let id = self.id;
        let forwarded = callback.is_some();
        CALLBACKS.with(|callbacks| match callback {
            Some(callback) => {
                callbacks.borrow_mut().insert(id, Rc::from(callback));
            }
            None => {
                callbacks.borrow_mut().remove(&id);
            }
        });
        let poster = ThreadPoster::current();
        with_surface(id, move |surface| {
            surface.set_on_state_change(forwarded.then(|| {
                Box::new(move |state: WebViewState| {
                    poster.post(move || {
                        let callback =
                            CALLBACKS.with(|callbacks| callbacks.borrow().get(&id).cloned());
                        if let Some(callback) = callback {
                            callback(state);
                        }
                    });
                }) as Box<dyn Fn(WebViewState)>
            }));
        });
    }

    fn load(&mut self, request: &WebViewRequest) {
        let request = request.clone();
        with_surface(self.id, move |surface| surface.load(&request));
    }

    fn perform(&mut self, command: WebViewCommand) {
        with_surface(self.id, move |surface| surface.perform(command));
    }

    fn update_geometry(&mut self, geometry: WebViewGeometry) {
        with_surface(self.id, move |surface| surface.update_geometry(geometry));
    }

    fn detach(&mut self) {
        let id = self.id;
        CALLBACKS.with(|callbacks| callbacks.borrow_mut().remove(&id));
        PlatformThread::post(Box::new(move || {
            let surface = SURFACES.with(|surfaces| surfaces.borrow_mut().remove(&id));
            if let Some(mut surface) = surface {
                surface.detach();
            }
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct Log {
        calls: Vec<String>,
        threads: Vec<std::thread::ThreadId>,
    }

    struct FakeHost(Arc<Mutex<Log>>);
    impl WebViewHost for FakeHost {
        fn make_surface(&self) -> Box<dyn WebViewSurface> {
            Box::new(FakeSurface(self.0.clone(), None))
        }
    }

    struct FakeSurface(Arc<Mutex<Log>>, Option<Box<dyn Fn(WebViewState)>>);
    impl FakeSurface {
        fn record(&self, call: &str) {
            let mut log = self.0.lock().unwrap();
            log.calls.push(call.to_owned());
            log.threads.push(std::thread::current().id());
        }
    }
    impl WebViewSurface for FakeSurface {
        fn set_on_state_change(&mut self, callback: Option<Box<dyn Fn(WebViewState)>>) {
            self.record("callback");
            self.1 = callback;
        }
        fn load(&mut self, _: &WebViewRequest) {
            self.record("load");
            if let Some(callback) = &self.1 {
                callback(WebViewState {
                    title: Some("loaded".into()),
                    ..WebViewState::default()
                });
            }
        }
        fn perform(&mut self, _: WebViewCommand) {
            self.record("perform");
        }
        fn update_geometry(&mut self, _: WebViewGeometry) {
            self.record("geometry");
        }
        fn detach(&mut self) {
            self.record("detach");
        }
    }

    #[test]
    fn surface_calls_run_in_order_on_the_platform_thread_and_state_comes_back() {
        let (send, receive) = channel::<cove_core::scene::PlatformWork>();
        let platform = std::thread::spawn(move || {
            let id = std::thread::current().id();
            while let Ok(work) = receive.recv() {
                work();
            }
            id
        });
        let send = RefCell::new(send);
        PlatformThread::set_executor(Some(Rc::new(move |work| {
            send.borrow().send(work).unwrap();
        })));

        let log = Arc::new(Mutex::new(Log::default()));
        let host = {
            let log = log.clone();
            PlatformThreadWebViewHost::new(move || Box::new(FakeHost(log)))
        };
        let mut surface = host.make_surface();
        let seen = Rc::new(RefCell::new(None));
        let observed = seen.clone();
        surface.set_on_state_change(Some(Box::new(move |state| {
            *observed.borrow_mut() = state.title;
        })));
        surface.load(&WebViewRequest::url("https://example.com"));
        surface.update_geometry(WebViewGeometry::default());

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while seen.borrow().is_none() && std::time::Instant::now() < deadline {
            cove_core::reactive::drain_posted_signal_updates();
            std::thread::yield_now();
        }
        assert_eq!(seen.borrow().as_deref(), Some("loaded"));

        surface.detach();
        drop(surface);
        drop(host);
        PlatformThread::set_executor(None);
        let platform = platform.join().unwrap();
        let log = log.lock().unwrap();
        assert_eq!(log.calls, ["callback", "load", "geometry", "detach"]);
        assert!(log.threads.iter().all(|thread| *thread == platform));
    }

    #[test]
    fn without_an_executor_calls_run_at_once() {
        let log = Arc::new(Mutex::new(Log::default()));
        let host = {
            let log = log.clone();
            PlatformThreadWebViewHost::new(move || Box::new(FakeHost(log)))
        };
        let mut surface = host.make_surface();
        surface.perform(WebViewCommand::Reload);
        assert_eq!(log.lock().unwrap().calls, ["perform"]);
    }
}
