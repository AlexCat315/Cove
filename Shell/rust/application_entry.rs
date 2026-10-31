use crate::ShellError;
pub struct PlatformApplication;
impl PlatformApplication {
    /// Retains the runtime result for the entire native application lifecycle.
    pub fn run<F: FnOnce() -> Result<T, ShellError>, T>(body: F) -> Result<(), ShellError> {
        #[cfg(any(target_os = "ios", target_os = "android"))]
        {
            struct Invocation<F, T> {
                body: Option<F>,
                result: Option<Result<T, ShellError>>,
                panic: Option<Box<dyn std::any::Any + Send>>,
            }
            unsafe extern "C" fn invoke<F: FnOnce() -> Result<T, ShellError>, T>(
                context: *mut std::ffi::c_void,
            ) -> i32 {
                // SAFETY: run passes an exclusive stack Invocation for the synchronous native lifecycle.
                let invocation = unsafe { &mut *context.cast::<Invocation<F, T>>() };
                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    invocation
                        .body
                        .take()
                        .expect("application entry called once")()
                })) {
                    Ok(result) => {
                        let code = if result.is_ok() { 0 } else { 1 };
                        invocation.result = Some(result);
                        code
                    }
                    Err(panic) => {
                        invocation.panic = Some(panic);
                        1
                    }
                }
            }
            let mut invocation = Invocation {
                body: Some(body),
                result: None,
                panic: None,
            };
            // SAFETY: SDL_RunApp synchronously invokes the callback and retains no pointer after return.
            let status = unsafe {
                crate::native::cove_sdl_run_app(
                    invoke::<F, T>,
                    (&mut invocation as *mut Invocation<F, T>).cast(),
                )
            };
            if let Some(panic) = invocation.panic {
                std::panic::resume_unwind(panic);
            }
            if let Some(result) = invocation.result {
                result?;
            }
            if status != 0 {
                return Err(ShellError::InitializationFailed(format!(
                    "SDL application entry failed: {status}"
                )));
            }
            Ok(())
        }
        #[cfg(not(any(target_os = "ios", target_os = "android")))]
        {
            let _runtime = body()?;
            Ok(())
        }
    }
}
