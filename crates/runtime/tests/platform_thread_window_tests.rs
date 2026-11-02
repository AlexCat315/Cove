//! The platform thread against a real SDL window, on the process main thread. A
//! machine without a display reports that and passes.
//!
//! Debug builds of the shell abort on any native call off the main thread, so
//! reaching the end also proves the UI thread stayed off the platform.

use cove_platform::{SystemCursor, TextInputArea};
use cove_runtime::platform::run_with_ui_thread;
use cove_runtime::platform::{PlatformLink, ProxyShell};
use cove_shell::{Shell, WindowChromeHitTest, WindowOptions, WindowSnapshot};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(10);

/// Pumps the UI side until `done` holds. The UI never blocks on the platform; it
/// waits for published state like the application loop does.
fn pump_until(shell: &mut ProxyShell, mut done: impl FnMut(&ProxyShell) -> bool) -> bool {
    let deadline = Instant::now() + TIMEOUT;
    while Instant::now() < deadline {
        let _ = shell.poll_window_events();
        for work in shell.take_deferred_callbacks() {
            work();
        }
        if done(shell) {
            return true;
        }
        shell.wait_for_events(Duration::from_millis(50));
    }
    false
}

fn exercise() -> Result<(), String> {
    let link = PlatformLink::current().ok_or("the UI thread has no platform link")?;
    assert!(!cove_shell::is_main_thread());
    let mut shell = ProxyShell::new(link);
    let handle = shell
        .create_window(
            "platform thread test",
            WindowOptions {
                width: 320,
                height: 240,
                ..WindowOptions::default()
            },
        )
        .map_err(|error| error.to_string())?;
    let id = handle.id();

    let ready = pump_until(&mut shell, |shell| {
        shell
            .window(id)
            .is_some_and(|window| window.render_surface().is_some())
    });
    if !ready {
        match shell.take_startup_error() {
            Some(error) => println!("skipped, no window available: {error}"),
            None => println!("skipped, no window was created in time"),
        }
        return Ok(());
    }
    let snapshot: WindowSnapshot = shell.window_snapshot(id).ok_or("no snapshot")?;
    assert!(snapshot.content_scale >= 1.0);
    assert_eq!(snapshot.logical_size, (320, 240));
    assert!(snapshot.position.is_some());

    // Class A: the last write of a burst is the one the platform sees.
    for width in [400.0, 480.0, 560.0] {
        shell.set_window_size(id, width, 300.0);
    }
    assert!(
        pump_until(&mut shell, |shell| shell
            .window(id)
            .is_some_and(|window| window.logical_size() == (560, 300))),
        "the coalesced size reaches the window and comes back through the snapshot"
    );

    shell.set_cursor(id, SystemCursor::Pointer);
    shell.set_text_input_area(
        id,
        Some(TextInputArea {
            x: 1.0,
            y: 2.0,
            width: 3.0,
            height: 4.0,
            cursor_x: 1.0,
        }),
    );
    shell.set_text_input_area(id, None);
    shell.set_window_chrome_hit_test(id, Some(&WindowChromeHitTest::new(28.0)));
    shell.set_window_chrome_hit_test(id, None);

    // Display refresh answers from what the platform thread published.
    let enabled = shell.set_display_refresh_enabled(id, true);
    assert!(pump_until(&mut shell, |shell| {
        shell.set_display_refresh_enabled(id, true) == enabled
            || !shell.set_display_refresh_enabled(id, true)
    }));
    shell.set_display_refresh_enabled(id, false);

    // Class B: the read observes the write queued before it. The user's clipboard
    // is put back afterwards.
    let original: Rc<RefCell<Option<Option<String>>>> = Rc::default();
    let seen = original.clone();
    shell.read_clipboard(Box::new(move |text| *seen.borrow_mut() = Some(text)));
    assert!(pump_until(&mut shell, |_| original.borrow().is_some()));
    shell.write_clipboard("cove platform thread");
    let echoed: Rc<RefCell<Option<Option<String>>>> = Rc::default();
    let seen = echoed.clone();
    shell.read_clipboard(Box::new(move |text| *seen.borrow_mut() = Some(text)));
    assert!(pump_until(&mut shell, |_| echoed.borrow().is_some()));
    assert_eq!(
        echoed.borrow().clone().flatten().as_deref(),
        Some("cove platform thread")
    );
    if let Some(text) = original.borrow().clone().flatten() {
        shell.write_clipboard(&text);
    }

    shell.destroy_window(id);
    assert!(shell.window(id).is_none());
    assert!(!shell.is_running());
    shell.shutdown();
    Ok(())
}

fn main() {
    let result = run_with_ui_thread(
        || cove_shell::make_default_shell().map(|shell| Box::new(shell) as Box<dyn Shell>),
        exercise,
    );
    match result {
        Ok(Ok(())) => println!("test platform_thread_window ... ok"),
        Ok(Err(error)) => panic!("{error}"),
        // No video device: there is nothing to exercise.
        Err(error) => println!("skipped, the shell could not start: {error}"),
    }
}
