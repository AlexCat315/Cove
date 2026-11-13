mod mock;

#[cfg(unix)]
mod process;

#[cfg(target_os = "macos")]
mod real;
