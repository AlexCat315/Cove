#[cfg(any(target_os = "macos", target_os = "ios"))]
unsafe extern "C" {
    fn cove_runtime_with_pool(
        callback: unsafe extern "C" fn(*mut std::ffi::c_void),
        context: *mut std::ffi::c_void,
    );
}
pub(super) fn with_autorelease_pool<F: FnOnce()>(body: F) {
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        struct Invocation<F> {
            body: Option<F>,
            panic: Option<Box<dyn std::any::Any + Send>>,
        }
        unsafe extern "C" fn invoke<F: FnOnce()>(context: *mut std::ffi::c_void) {
            // SAFETY: caller passes an exclusive stack Invocation for the synchronous pool scope.
            let invocation = unsafe { &mut *context.cast::<Invocation<F>>() };
            if let Err(panic) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                invocation.body.take().unwrap()()
            })) {
                invocation.panic = Some(panic);
            }
        }
        let mut invocation = Invocation {
            body: Some(body),
            panic: None,
        }; // SAFETY: native pool invokes callback synchronously and retains neither pointer nor callback afterward.
        unsafe {
            cove_runtime_with_pool(invoke::<F>, (&mut invocation as *mut Invocation<F>).cast())
        };
        if let Some(panic) = invocation.panic {
            std::panic::resume_unwind(panic);
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    {
        body();
    }
}
pub(super) struct TimerResolution;
#[cfg(target_os = "windows")]
#[link(name = "winmm")]
unsafe extern "system" {
    fn timeBeginPeriod(period: u32) -> u32;
    fn timeEndPeriod(period: u32) -> u32;
}
impl TimerResolution {
    pub fn new() -> Self {
        #[cfg(target_os = "windows")]
        // SAFETY: Windows accepts a one-millisecond timer request, balanced in Drop.
        unsafe {
            timeBeginPeriod(1);
        }
        Self
    }
}
impl Drop for TimerResolution {
    fn drop(&mut self) {
        #[cfg(target_os = "windows")] // SAFETY: balances this scope's timeBeginPeriod request.
        unsafe {
            timeEndPeriod(1);
        }
    }
}
