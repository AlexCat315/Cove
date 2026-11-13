//! Adapter process-tree cleanup.
use crate::process::AdapterProcess;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::{Duration, Instant};

struct Folder(PathBuf);

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn alive(pid: u32) -> bool {
    // SAFETY: signal 0 performs an existence check and sends nothing.
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

fn wait_gone(pid: u32) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while alive(pid) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!alive(pid), "PID {pid} survived adapter cleanup");
}

#[test]
fn terminating_the_adapter_ends_its_children_and_ignores_term_ignoring_helpers() {
    let folder =
        Folder(std::env::temp_dir().join(format!("cove-debugger-process-{}", std::process::id())));
    let _ = fs::create_dir_all(&folder.0);
    let pid_file = folder.0.join("child.pid");
    let script = folder.0.join("adapter.sh");
    fs::write(
        &script,
        format!(
            "#!/bin/sh\ntrap '' TERM\n/bin/sleep 30 &\nchild=$!\necho \"$child\" > '{}'\nwait \"$child\"\n",
            pid_file.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();

    let (mut adapter, io) = AdapterProcess::spawn(&script).expect("spawn the test adapter");
    drop(io);
    let adapter_pid = adapter.pid();
    let deadline = Instant::now() + Duration::from_secs(2);
    let child = loop {
        if let Ok(text) = fs::read_to_string(&pid_file)
            && let Ok(child) = text.trim().parse::<u32>()
        {
            break child;
        }
        assert!(Instant::now() < deadline, "the helper did not start");
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(alive(child));
    adapter.terminate();
    wait_gone(adapter_pid);
    wait_gone(child);
}
