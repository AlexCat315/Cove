//! inotify (Linux and Android). Watches are per directory, so every folder in
//! the tree gets one, and folders created later are added as they appear.

use std::collections::HashMap;
use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;

const MASK: u32 = libc::IN_CREATE
    | libc::IN_DELETE
    | libc::IN_MODIFY
    | libc::IN_CLOSE_WRITE
    | libc::IN_MOVED_FROM
    | libc::IN_MOVED_TO
    | libc::IN_DELETE_SELF
    | libc::IN_MOVE_SELF
    | libc::IN_ATTRIB;

pub struct NativeWatch {
    stop: Arc<AtomicBool>,
}

impl NativeWatch {
    pub fn start(root: &Path, wake: Sender<()>, stop: Arc<AtomicBool>) -> Option<Self> {
        // SAFETY: plain syscall; the descriptor is owned by the thread below.
        let fd = unsafe { libc::inotify_init1(libc::IN_CLOEXEC | libc::IN_NONBLOCK) };
        if fd < 0 {
            return None;
        }
        let mut watches = HashMap::new();
        add_tree(fd, root, &mut watches);
        let thread_stop = stop.clone();
        let spawned = std::thread::Builder::new()
            .name("cove-watch-inotify".to_owned())
            .spawn(move || run(fd, watches, wake, thread_stop));
        if spawned.is_err() {
            // SAFETY: the thread did not start, so the descriptor is unused.
            unsafe { libc::close(fd) };
            return None;
        }
        Some(Self { stop })
    }
}

impl Drop for NativeWatch {
    fn drop(&mut self) {
        // The reading thread polls with a timeout and closes the descriptor
        // when it sees the flag.
        self.stop.store(true, Ordering::Release);
    }
}

fn add_tree(fd: i32, directory: &Path, watches: &mut HashMap<i32, PathBuf>) {
    let Ok(path) = CString::new(directory.as_os_str().as_bytes()) else {
        return;
    };
    // SAFETY: `path` is a valid C string for the duration of the call.
    let wd = unsafe { libc::inotify_add_watch(fd, path.as_ptr(), MASK) };
    if wd >= 0 {
        watches.insert(wd, directory.to_path_buf());
    }
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            add_tree(fd, &entry.path(), watches);
        }
    }
}

fn run(fd: i32, mut watches: HashMap<i32, PathBuf>, wake: Sender<()>, stop: Arc<AtomicBool>) {
    let mut buffer = vec![0_u8; 64 * 1024];
    let header = std::mem::size_of::<libc::inotify_event>();
    while !stop.load(Ordering::Acquire) {
        let mut poll = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: one valid pollfd for the duration of the call.
        let ready = unsafe { libc::poll(&mut poll, 1, 500) };
        if ready <= 0 {
            continue;
        }
        // SAFETY: the buffer is writable for its whole length.
        let read = unsafe { libc::read(fd, buffer.as_mut_ptr().cast(), buffer.len()) };
        if read <= 0 {
            continue;
        }
        let read = read as usize;
        let mut offset = 0;
        let mut new_directories = Vec::new();
        while offset + header <= read {
            // SAFETY: the kernel writes whole events; the header fits.
            let event: libc::inotify_event =
                unsafe { std::ptr::read_unaligned(buffer[offset..].as_ptr().cast()) };
            let name_start = offset + header;
            let name_end = (name_start + event.len as usize).min(read);
            if event.mask & libc::IN_ISDIR != 0
                && event.mask & (libc::IN_CREATE | libc::IN_MOVED_TO) != 0
                && let Some(parent) = watches.get(&event.wd)
            {
                let name = &buffer[name_start..name_end];
                let name = name.split(|byte| *byte == 0).next().unwrap_or_default();
                new_directories.push(parent.join(std::ffi::OsStr::from_bytes(name)));
            }
            if event.mask & libc::IN_IGNORED != 0 {
                watches.remove(&event.wd);
            }
            offset = name_end;
        }
        for directory in new_directories {
            add_tree(fd, &directory, &mut watches);
        }
        if wake.send(()).is_err() {
            break;
        }
    }
    // SAFETY: this thread owns the descriptor.
    unsafe { libc::close(fd) };
}
