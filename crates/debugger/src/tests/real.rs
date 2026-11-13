//! Real `lldb-dap` discovery, initialize and denied-attach checks.
use crate::adapter::Availability;
use crate::session::DapSession;
use std::collections::BTreeSet;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

struct Disposable(Child);

impl Drop for Disposable {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn debugserver_pids() -> BTreeSet<u32> {
    let output = crate::process::run_capture(
        std::path::Path::new("ps"),
        &["-axo", "pid=,comm="],
        Duration::from_secs(2),
    )
    .expect("ps runs");
    output
        .stdout
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let pid: u32 = parts.next()?.parse().ok()?;
            let command: String = parts.collect::<Vec<_>>().join(" ");
            command
                .rsplit('/')
                .next()
                .is_some_and(|name| name.contains("debugserver"))
                .then_some(pid)
        })
        .collect()
}

fn process_alive(pid: u32) -> bool {
    if pid == 0 || pid > i32::MAX as u32 {
        return false;
    }
    // SAFETY: signal 0 performs an existence check and sends nothing.
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

fn wait_gone(pid: u32) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while process_alive(pid) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!process_alive(pid), "adapter PID {pid} is still alive");
}

#[test]
fn discovers_and_initializes_the_real_lldb_dap() {
    let availability = Availability::discover();
    let Some(adapter) = availability.adapter else {
        eprintln!("lldb-dap is not installed; skipping the real initialize check");
        return;
    };
    assert!(std::path::Path::new(&adapter.path).is_absolute());
    assert!(
        adapter.version.to_lowercase().contains("lldb"),
        "version: {}",
        adapter.version
    );

    let session = DapSession::spawn(&adapter).expect("spawn lldb-dap");
    let adapter_pid = session.adapter_pid();
    let info = session.initialize().expect("initialize lldb-dap");
    let version = info.adapter_version.expect("adapter version");
    assert!(
        version.to_lowercase().contains("lldb"),
        "adapter reported: {version}"
    );
    session.abort();
    wait_gone(adapter_pid);
}

#[test]
fn a_real_attach_is_denied_without_developer_mode_and_leaves_no_debugserver() {
    let availability = Availability::discover();
    let Some(adapter) = availability.adapter.clone() else {
        eprintln!("lldb-dap is not installed; skipping the attach check");
        return;
    };

    let before = debugserver_pids();
    let mut target = Disposable(Command::new("/bin/sleep").arg("30").spawn().expect("sleep"));
    let target_pid = target.0.id();
    let session = DapSession::spawn(&adapter).expect("spawn lldb-dap");
    let adapter_pid = session.adapter_pid();
    session.initialize().expect("initialize lldb-dap");
    let result = session.attach(target_pid);
    match result {
        Ok(_) => {
            assert_ne!(
                availability.developer_mode,
                Some(false),
                "attach succeeded while Developer mode is disabled"
            );
            session.detach().expect("detach the disposable target");
        }
        Err(error) => {
            assert_eq!(error.code(), "unavailable", "{}", error.message());
            assert!(
                error.message().to_lowercase().contains("attach"),
                "explicit attach reason: {}",
                error.message()
            );
        }
    }
    session.abort();
    wait_gone(adapter_pid);
    assert!(
        target.0.try_wait().expect("target status").is_none(),
        "the attach attempt must not terminate the target"
    );

    std::thread::sleep(Duration::from_millis(300));
    let after = debugserver_pids();
    let leaked: Vec<_> = after.difference(&before).copied().collect();
    assert!(
        leaked.is_empty(),
        "attach failure left debugserver processes: {leaked:?}"
    );
}
