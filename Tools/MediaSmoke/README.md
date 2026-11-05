# Independent media consumer

This package is outside the Cove workspace. It declares only `cove-media` with
`gst`; it has no build script, `[patch]`, pkg-config configuration, or SDK paths.

Run `cargo cove setup --media` once for the host. Outside the Cove checkout the
same command is available after `cargo install --path xtask --bin cargo-cove`,
which installs the `cargo cove` subcommand; the checked-in `cargo cove` alias
only resolves inside this repository. Then, from this directory:

```sh
cargo build
cargo test
cargo run
```

The application runs a three-frame source through the public retained endpoint
and reads the retained pixels after EOS. CI copies it outside the Cove checkout
before building, so ancestor Cargo configuration cannot hide missing integration.
