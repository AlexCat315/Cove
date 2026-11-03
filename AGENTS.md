# AGENTS.md

Use Rust 2024 for all new implementations. Migrate existing Swift capabilities to Rust; do not add Swift code.

Avoid unnecessary supporting text, descriptive copy, and redundant UI microcopy.


Please refer to this for UI design agent/skills/

A changelog must be written for every new version release.

You must not omit the implementation for any platform simply because it cannot be tested; code must be implemented for all platforms. It is mandatory to provide implementations across the board to avoid gaps in the codebase. Should any such issues be discovered, they must be reported immediately.

For performance-critical sections, please inline assembly.

Apply breaking changes without requiring confirmation prior to the release of version 1.0.0.

## DevTools

For live UI inspection and interaction, use the Cove DevTools MCP tools configured in `.mcp.json`. See `crates/devtools/README.md`. Start the app with `COVE_DEVTOOLS=1`; the MCP entry point is `cargo run --quiet -p cove-devtools-cli -- mcp`.

Use `get_ui_session`, `query_ui_nodes`, `click_ui`, `fill_ui`, `wait_for_ui` and `capture_ui` directly. Use `input_ui` for drag/scroll sequences and `start_ui_trace` / `stop_ui_trace` around real operations. Do not write custom WebSocket scripts for operations already exposed as tools. Without an MCP connection, call the same tools using `cove-devtools call NAME JSON`. Check postconditions; input acknowledgements only confirm delivery.
