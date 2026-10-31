use std::ffi::c_void;
unsafe extern "C" {
    fn cove_runtime_display_driver_start(
        tick: unsafe extern "C" fn(*mut c_void) -> bool,
        release: unsafe extern "C" fn(*mut c_void),
        context: *mut c_void,
    ) -> *mut c_void;
    fn cove_runtime_display_driver_stop(driver: *mut c_void);
}
pub(super) struct IOSFrameDriver {
    pointer: *mut c_void,
}
impl IOSFrameDriver {
    pub fn new<F: Fn() -> bool + 'static>(frame: F) -> Self {
        unsafe extern "C" fn tick<F: Fn() -> bool>(context: *mut c_void) -> bool {
            // SAFETY: context is a live Box<F> retained by the native display driver.
            let frame = unsafe { &*context.cast::<F>() };
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(frame)).unwrap_or(false)
        }
        unsafe extern "C" fn release<F>(context: *mut c_void) {
            // SAFETY: native driver calls release exactly once when stopping, after its last callback returns.
            unsafe {
                drop(Box::from_raw(context.cast::<F>()));
            }
        }
        let context = Box::into_raw(Box::new(frame)).cast(); // SAFETY: callbacks and owned context stay valid until native driver relinquishes ownership.
        let pointer =
            unsafe { cove_runtime_display_driver_start(tick::<F>, release::<F>, context) };
        Self { pointer }
    }
}
impl Drop for IOSFrameDriver {
    fn drop(&mut self) {
        if !self.pointer.is_null() {
            // SAFETY: this owner stops/releases the native driver once on the UIKit thread.
            unsafe { cove_runtime_display_driver_stop(self.pointer) };
            self.pointer = std::ptr::null_mut();
        }
    }
}
