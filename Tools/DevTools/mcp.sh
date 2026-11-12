#!/bin/sh
# MCP hosts spawn stdio servers with the GUI environment, whose PATH has no
# Rust toolchain or sccache. Add them, then hand stdio to the cargo-run server.
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"
cd "$(dirname "$0")/../.." || exit 1
exec cargo run --quiet --package cove-devtools-cli -- mcp
