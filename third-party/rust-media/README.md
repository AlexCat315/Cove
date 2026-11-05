# Official media bindings

These are the previously locked GStreamer / gtk-rs crates, copied from their
published source archives. `upstream.json` records their versions, registry
checksums, source revisions and original licenses. License files remain beside
each crate. No third-party Rust dependency version is introduced here.

The maintained changes are limited to Cargo path dependencies within this set
and one call in each `*-sys/build.rs` to
`cove_native_build::gstreamer::prepare_sys()`. That call builds the pinned native
profile once and re-executes the upstream probe with a target-specific pkg-config
environment. The upstream FFI declarations and high-level implementations remain
unchanged. Path dependencies make this work transitively in external applications;
applications do not need `[patch]` sections or environment configuration.

These bindings are re-exported through `cove_media::gst`. An independently added
registry version is a different Cargo package instance and its Rust wrapper types
are not interchangeable with this source set.
