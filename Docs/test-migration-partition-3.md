# Remaining test migration partition (wave 3 — final)

2026-11-01：下列 65 个 Rust 测试目标均已落盘并纳入 Cargo。源码路径保留为迁移来源；当前目标位于各 crate 的 `tests/`。全量执行结果见 [验证报告](rust-migration-validation.md)。

Support files are not suites and are not in the groups below:

| Swift file | Covered by |
| --- | --- |
| `Tests/CoveUIComposeTests/TestSupport.swift` | `crates/compose/tests/support/mod.rs` |
| `Tests/CoveUIComposeTests/Mobile/Fixtures.swift` | inline fixture inside `mobile_presentation_tests` |
| `Tests/CoveUIComposeTests/Images/HTTPServer.swift` | `image_pipeline_tests` / `async_image_tests` own their loopback server |
| `Graphics/Tests/NativeRHITests/NativeTestDevice.swift` | `crates/graphics/src/rhi/testing.rs` |
| `Graphics/Tests/NativeRHITests/ShaderFixtureCompiler.swift` | `shader_specialization_tests` helper |
| `Tests/Headless/Package.swift` | not a test |

| group | files | Swift lines |
| --- | ---: | ---: |
| compose-sidebar | 5 | 1731 |
| compose-adaptive-layout | 7 | 1828 |
| compose-adaptive-forms | 8 | 1289 |
| compose-textfield | 4 | 1421 |
| compose-rich-editor | 5 | 1536 |
| compose-scroll | 4 | 1940 |
| compose-list | 5 | 1753 |
| compose-menu-overlay | 5 | 1178 |
| compose-misc | 7 | 1378 |
| compose-regression-motion | 7 | 1886 |
| gallery-and-tails | 8 | 1392 |

## compose-sidebar

- `Tests/CoveUIComposeTests/Adaptive/AdaptiveSidebarTests.swift` -> `adaptive_sidebar_tests`
- `Tests/CoveUIComposeTests/Adaptive/SidebarHierarchyTests.swift` -> `sidebar_hierarchy_tests`
- `Tests/CoveUIComposeTests/Adaptive/SidebarResizeTests.swift` -> `sidebar_resize_tests`
- `Tests/CoveUIComposeTests/SidebarContentTests.swift` -> `sidebar_content_tests`
- `Tests/CoveUIComposeTests/Adaptive/NavigationDrawerTests.swift` -> `navigation_drawer_tests`

## compose-adaptive-layout

- `Tests/CoveUIComposeTests/Adaptive/NavigationRailTests.swift` -> `navigation_rail_tests`
- `Tests/CoveUIComposeTests/Adaptive/SplitPresentationTests.swift` -> `split_presentation_tests`
- `Tests/CoveUIComposeTests/Adaptive/TabOverflowTests.swift` -> `tab_overflow_tests`
- `Tests/CoveUIComposeTests/Adaptive/ToolbarOverflowTests.swift` -> `toolbar_overflow_tests`
- `Tests/CoveUIComposeTests/Adaptive/NarrowReflowTests.swift` -> `narrow_reflow_tests`
- `Tests/CoveUIComposeTests/Adaptive/GridColumnsTests.swift` -> `grid_columns_tests`
- `Tests/CoveUIComposeTests/Adaptive/TableColumnsTests.swift` -> `table_columns_tests`

## compose-adaptive-forms

- `Tests/CoveUIComposeTests/Adaptive/DialogTests.swift` -> `adaptive_dialog_tests`
- `Tests/CoveUIComposeTests/Adaptive/DisclosureTests.swift` -> `adaptive_disclosure_tests`
- `Tests/CoveUIComposeTests/Adaptive/EditorHeightTests.swift` -> `editor_height_tests`
- `Tests/CoveUIComposeTests/Adaptive/JsonEditorTests.swift` -> `json_editor_tests`
- `Tests/CoveUIComposeTests/Adaptive/RowDensityTests.swift` -> `row_density_tests`
- `Tests/CoveUIComposeTests/Adaptive/SelectionFieldTests.swift` -> `selection_field_tests`
- `Tests/CoveUIComposeTests/Adaptive/TextScaleRows.swift` -> `text_scale_rows`
- `Tests/CoveUIComposeTests/Adaptive/TimelineTests.swift` -> `adaptive_timeline_tests`

## compose-textfield

- `Tests/CoveUIComposeTests/TextFieldTests.swift` -> `text_field_tests`
- `Tests/CoveUIComposeTests/TextFieldNavigationTests.swift` -> `text_field_navigation_tests`
- `Tests/CoveUIComposeTests/TextFieldLargeDocumentTests.swift` -> `text_field_large_document_tests`
- `Tests/CoveUIComposeTests/TypographyRasterTests.swift` -> `typography_raster_tests`

## compose-rich-editor

- `Tests/CoveUIComposeTests/RichTextTests.swift` -> `rich_text_tests`
- `Tests/CoveUIComposeTests/MarkdownTests.swift` -> `markdown_tests`
- `Tests/CoveUIComposeTests/CodeEditing/GutterTests.swift` -> `gutter_tests`
- `Tests/CoveUIComposeTests/CodeEditingUITests.swift` -> `code_editing_ui_tests`
- `Tests/CoveUIComposeTests/EditorInteractionTests.swift` -> `editor_interaction_tests`

## compose-scroll

- `Tests/CoveUIComposeTests/ButtonScrollViewTests.swift` -> `button_scroll_view_tests`
- `Tests/CoveUIComposeTests/ScrollbarTests.swift` -> `scrollbar_tests`
- `Tests/CoveUIComposeTests/ScrollOffsetBindingTests.swift` -> `scroll_offset_binding_tests`
- `Tests/CoveUIComposeTests/Scrolling/ScrollPhysicsTests.swift` -> `scroll_physics_tests`

## compose-list

- `Tests/CoveUIComposeTests/ListTreeTests.swift` -> `list_tree_tests`
- `Tests/CoveUIComposeTests/ListRowTests.swift` -> `list_row_tests`
- `Tests/CoveUIComposeTests/SectionedListTests.swift` -> `sectioned_list_tests`
- `Tests/CoveUIComposeTests/SwipeListTests.swift` -> `swipe_list_tests`
- `Tests/CoveUIComposeTests/WheelPickerTests.swift` -> `wheel_picker_tests`

## compose-menu-overlay

- `Tests/CoveUIComposeTests/MenuHierarchyTests.swift` -> `menu_hierarchy_tests`
- `Tests/CoveUIComposeTests/MenuPointerRoutingTests.swift` -> `menu_pointer_routing_tests`
- `Tests/CoveUIComposeTests/NativeMenuTests.swift` -> `native_menu_tests`
- `Tests/CoveUIComposeTests/PopconfirmTests.swift` -> `popconfirm_tests`
- `Tests/CoveUIComposeTests/RatingTests.swift` -> `rating_tests`

## compose-misc

- `Tests/CoveUIComposeTests/AdvancedInteractionTests.swift` -> `advanced_interaction_tests`
- `Tests/CoveUIComposeTests/Agent/ControlDiscoveryTests.swift` -> `control_discovery_tests`
- `Tests/CoveUIComposeTests/Calendar/KeyboardNavigationTests.swift` -> `calendar_keyboard_navigation_tests`
- `Tests/CoveUIComposeTests/Calendar/ModesTests.swift` -> `calendar_modes_tests`
- `Tests/CoveUIComposeTests/DateRangeRulesTests.swift` -> `date_range_rules_tests`
- `Tests/CoveUIComposeTests/ObservableStateTrackingTests.swift` -> `observable_state_tracking_tests`
- `Tests/CoveUIComposeTests/RenderTreeMirrorTests.swift` -> `render_tree_mirror_tests`

## compose-regression-motion

- `Tests/CoveUIComposeTests/ProductInteractionRegressionTests.swift` -> `product_interaction_regression_tests`
- `Tests/CoveUIComposeTests/RouteAndPreloadTests.swift` -> `route_and_preload_tests`
- `Tests/CoveUIComposeTests/StyleIntegrationTests.swift` -> `style_integration_tests`
- `Tests/CoveUIComposeTests/ThemeInteractionRegressionTests.swift` -> `theme_interaction_regression_tests`
- `Tests/CoveUIComposeTests/Transitions/SurfaceRevealTests.swift` -> `surface_reveal_tests`
- `Tests/CoveUIComposeTests/WithAnimationPropertiesTests.swift` -> `with_animation_properties_tests`
- `Tests/CoveUIComposeTests/Mobile/PresentationTests.swift` -> `mobile_presentation_tests`

## gallery-and-tails

- `Examples/Tests/GalleryContentTests/AdaptiveShellTests.swift` -> `adaptive_shell_tests` (Examples)
- `Examples/Tests/GalleryContentTests/AppearanceTests.swift` -> `gallery_appearance_tests` (Examples)
- `Examples/Tests/GalleryContentTests/DrawerTests.swift` -> `drawer_tests` (Examples)
- `Examples/Tests/GalleryContentTests/RailTests.swift` -> `rail_tests` (Examples)
- `Examples/Tests/GalleryContentTests/SelectionShellTests.swift` -> `selection_shell_tests` (Examples)
- `Terminal/Tests/CoveUITerminalTests/InteractionTests.swift` -> `interaction_tests` (Terminal)
- `Tests/CoveUITests/PackageResourceBundleTests.swift` -> `ui_resource_bundle_tests` (Sources)
- `Tests/CoveUIWorkspaceTests/PackageResourceBundleTests.swift` -> `workspace_resource_bundle_tests` (Sources)
