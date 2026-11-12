# AGENTS.md

Use Rust 2024 for all new implementations. Migrate existing Swift capabilities to Rust; do not add Swift code.

Avoid unnecessary supporting text, descriptive copy, and redundant UI microcopy.


For UI design decisions, follow `agent/skills/emil-design-eng/SKILL.md`; the runtime skill registry lives in `crates/agent/src/skills.rs`.

A changelog must be written for every new version release.

You must not omit the implementation for any platform simply because it cannot be tested; code must be implemented for all platforms. It is mandatory to provide implementations across the board to avoid gaps in the codebase. Should any such issues be discovered, they must be reported immediately.

For performance-critical sections, please inline assembly.

Apply breaking changes without requiring confirmation prior to the release of version 1.0.0.

## DevTools

For live UI inspection and interaction, use the Cove DevTools MCP tools configured in `.mcp.json`. See `crates/devtools/README.md`. Start the controller with `cargo run -p cove-devtools-cli -- serve`, then launch the app with `COVE_DEVTOOLS=1`. The MCP entry point is `cargo run --quiet -p cove-devtools-cli -- mcp`.

Use `get_ui_session`, `query_ui_nodes`, `click_ui`, `type_ui_text`, `wait_for_ui` and `capture_ui` directly. Use `input_ui` for drag/scroll sequences and `start_ui_trace` / `stop_ui_trace` around real operations. Use the capture ID and returned coordinates for pointer actions. Use `query_native_ui`, `find_native_ui` and `activate_ui` for system controls. Without an MCP connection, call the same tools using `cove-devtools call NAME JSON`. Check postconditions; input acknowledgements only confirm delivery.

## Gallery verification

Track per-page implementation, visual and interaction acceptance in `Docs/gallery-audit.md`. For each component/page repaired or expanded, add or update its user-facing Rust documentation under `Website/src/content/components/` and register it in the documentation navigation. Distinguish structural test coverage from native visual/interaction acceptance; direct DevTools text injection does not validate native keyboard or IME activation.
