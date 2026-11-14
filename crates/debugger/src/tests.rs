mod mock;
mod native;

#[cfg(unix)]
mod process;

#[cfg(target_os = "macos")]
mod real;
