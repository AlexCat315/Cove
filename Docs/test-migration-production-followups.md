# Production follow-ups found during the test migration

Defects found while porting the Swift test suites. **Fixed** ones already have a
regression test in the same commit; **open** ones are documented here so they are
not lost, and are deliberately not fixed inside the migration because each needs
its own verification pass.

## Fixed (each has a regression test)

| # | Defect | Regression |
| --- | --- | --- |
| 1 | Ten widgets implemented `PrimitiveView` with an empty `impl View`, so `kind()` fell back to `Component` and mounting panicked: `TabViewBody`, `TabStripItem`, `RovingFocusHost`, `BreadcrumbBody`, `Matched`, `SidebarDestination`, `RailDestinationLabel`, `SidebarRowBody`, `AdaptiveShell`, `LazyGridCell` | `primitive_view_kind_regression` |
| 2 | `LayoutNode::calculate_layout` never stretch-fit the root, so every viewport-filling root collapsed to height 0 and `flex_grow` had no free space | `adaptive_surface_root`, `interactive_scene` |
| 3 | `AccessibilityTree::collect` returned early on a zero-size or clipped-out node, truncating the whole subtree (VoiceOver / Android a11y lost nearly every control tree) | `accessibility_subtree_regression` |
| 4 | Logging `prune` used a blocking `flock` where Swift used `LOCK_NB`, deadlocking retention forever on a live session | `storage_tests` |
| 5 | Minimum-size floor was read back at the wrong type (`Size` vs `Option<Size>`), so the downcast always failed and the floor was silently ignored | `adaptive_environment` |
| 6 | Clearing the floor could not restore the authored size: `apply_minimum_size` overwrote `style.width`/`height`. Floor is now clamped in `measure`/`arrange` only | `adaptive_environment`, `minimum_size_tests` |
| 7 | `has_measure_func` ignored custom layouts, making `mark_dirty()` a silent no-op for them | `measurement_lifecycle_tests` |
| 8 | `measure()` dispatch preferred the plain callback over `custom`, so a custom-layout node measured 0x0 | `measurement_lifecycle_tests` |
| 9 | Select/Combobox wrapper nodes set semantics on themselves (a fresh node has `accessibility() == None`), so both published no role and no name | `select_combo_role_tests` |
| 10 | `Popover` never provided `ActionControlAccessibility::expanded()`, so `is_expanded` was permanently `None` | `select_combo_role_tests` |
| 11 | Metal `encode_compute_pass` installed the encoder only after encoding, so a failing command leaked a live encoder and aborted the process at dealloc | `metal_encoder_teardown_regression` |
| 12 | `MetalRenderBindings::index` used `SLOTS = 128` as both array length and group stride, indexing past the array for every non-vertex binding | in-crate `render_bindings` tests |
| 13 | `LayoutNode::arrange` passed the container's *height* as a child's available *width* in a column, so text in every column shaped against the wrong bound | `text_divider` |
| 14 | `layout_result` treated a zero width as unbounded, so a truncating label at width 0 shaped and painted the whole string | `single_line_text_tests` |
| 15 | `BlinkTimer` cloned the `Cell<bool>` instead of sharing it, so the terminal cursor never visibly blinked | `SessionTests` port |
| 16 | `AndroidAdaptiveSurface` declared no `dynamic_properties`, so `update_surface` never recomposed and Android safe-area/keyboard avoidance froze at the first frame | `surface_metrics_tests` |
| 17 | Calendar used the authored 34pt cell size on touch surfaces instead of `minimum_control_extent`, putting cells below the 44pt target | `calendar_adaptive_tests` |
| 18 | Toggle's progress/fill writers ran outside any animation context (both branches byte-identical), so the switch jumped instead of travelling; missing `.with_overshoot(true)` also broke velocity inheritance on reversal | `control_motion_tests` |
| 19 | `AnimatedVisibility` fed `(from_progress, target)` into a controller whose closure treats its argument as a 0..1 fraction, interpolating twice — reversals overshot and a full exit never retired the subtree | `scale_tests` |
| 20 | Nested `with_frame_session` self-deadlocked: Swift used `NSRecursiveLock`, the port a non-recursive `Mutex` | `swapchain_lifetime_tests` |
| 21 | `SwapchainResource::close` set `closed` before the attempt, so a close refused mid-frame made retries silent no-ops and `Drop` skipped `destroy_swapchain`, leaking the native swapchain | `swapchain_lifetime_tests` |
| 22 | `flash_touch_indicator` snapped 1→0 with no animation context, so a touch scroll indicator was never visible | `indicator_tests` |
| 23 | `SceneEditor::intercept` held a `RefMut` across a `borrow_mut()` (Rust 2024 temporary scope), panicking on every second mouse-up while picking | `editor_pick_borrow_regression` |
| 24 | `PortalStoreAmbient` had no `ScopedAmbient` impl and was not re-exported, so no caller could scope a portal store through the runtime ambient; `Examples` had reimplemented it locally | `portal_scope_tests` |
| `EmptyDashboardSpace` was the last empty `impl View` on a plain struct, panicking on mounting the Introduction page | `cargo test -p cove-examples --lib` |
| `StoryDispatch::new` allowlisted only 15 pages while `view()` implemented 58, so **43 fully ported gallery stories were unreachable dead code** and rendered the "not migrated" notice | `every_implemented_view_arm_is_reachable` |
| `SwapchainImage` equality was field-based with a per-window `generation`, so a stale acquisition ticket compared equal to the current one and could consume the frame; Swift used reference identity | `swapchain_lifetime_tests` |

## Open (each needs its own change + verification)

| Defect | Why not fixed here |
| --- | --- |
| **`Binding` has no observability at all.** `DynamicField::state` takes `&State<T>`, but `Popover`, `AssetRefField`, `MenuButton`, `Menu` and `SplitButton` are driven by a `Binding`, and `Binding::new` stores only get/set closures — it discards the `State` handle, so a scope can never subscribe. Result: a `Binding`-driven view is invisible to recomposition, so `Popover::body` is never re-entered and **no popover, menu or asset picker can open**. Attempted fixes reach into `State::subscribe_local` lifetimes and the hot `State` path, so this needs a designed change (a `Binding` that carries an invalidation hook, or a `DynamicField` variant that owns its subscription) plus the full composition suite as a gate — not a drive-by | |
| `WorkspaceView` has no view reactivity: `dispatch` bumps `version` but nothing invalidates the scope, so an installed tree never re-renders | Sources-owned; a live host would show a stale workspace forever |
| `WorkspaceDocument` cannot be serialized: `WorkspaceLayoutNode` derives `Serialize` with `#[serde(tag = "kind")]` on a newtype `Group` variant | Persistence is impossible; decoding works |
| `GalleryView` never applies `navigation_width`, so the docked Sidebar stretches to the full window and `gallery-content` resolves to 0pt | Examples-owned; changes the gallery layout surface |
| `LayoutNode::arrange` resolves percentage padding against the node's own width; Yoga (`node/Node.cpp:341`) uses `ownerWidth`, the containing block | Requires threading the containing-block width through 6 `arrange` call sites in the hot layout path |
| `Diagnostics::capture_trace` is synchronous (`thread::sleep` on the only thread that could emit), so a capture can never observe a span; Swift's is async | Needs an async capture or frame-inbox bracket — a DevTools design change |
| `TextField` has no error state at all (`isError` hardcoded false); `NumberField`'s `NumericPrefixFieldStyle` is inert because `TextFieldHost` never reads `TextFieldStyleEnvironment` | Missing feature, and wiring the style path would also change appearance (adds unported error state + disabled alpha) |
| **`TextFieldCodeEditing`'s four request callbacks are declared and never called**: `on_request_completion`, `on_resolve_completion`, `on_accept_completion`, `on_request_hover`. Zero call sites repo-wide — no attachment install, no `handle_key` trigger. So code completion and hover docs cannot fire at all; the whole request side is unimplemented, not merely unwired | Needs the trigger path designed (key handling + attachment plumbing), not a one-line fix |
| `SceneEditor::handle` does not run `InspectionValidation`, so invalid inspect commands mutate the scene instead of returning `.err` | DevTools-owned; needs its own validation-order change |
| `DemoInsets` is `scope_apply`/`is_scope_applying`, so it gets a synthetic anchor with no layout node and silently drops its padding | Examples-owned; changes the example app's surface |
| `PipelineLease` has no `Drop`, so discarding one leaks a reference silently | Ownership-semantics change for every caller |
| `cove_graphics::ui` does not re-export `FontProvider`/`FontAtlas`, so the font half of `nativeUIScene` cannot be built from Graphics | API surface decision |
| `AppKitScrollInput` has no pure decode seam; `AppleAccessibilityBridge` is `pub(super)` and needs `objc2` in `cove-runtime` | Testability seams, not defects |
