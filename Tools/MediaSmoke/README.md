# Independent media consumer

This package is outside the Cove workspace. It declares only `cove-media` with
`ffmpeg`; it has no build script, `[patch]`, pkg-config configuration, or SDK paths.

Run `cargo cove setup --media` once for the host. Outside the Cove checkout the
same command is available after `cargo install --path xtask --bin cargo-cove`,
which installs the `cargo cove` subcommand; the checked-in `cargo cove` alias
only resolves inside this repository. Then, from this directory:

```sh
cargo build
cargo test
cargo run
```

The application creates a context and player through the public API and checks
that the bundled FFmpeg decoder set is reachable. CI copies it outside the Cove
checkout before building, so ancestor Cargo configuration cannot hide missing
integration.
