# Test migration partition

Each group is a disjoint set of Swift test sources. Target name = snake_case of the Swift stem.

| group | files | Swift lines |
| --- | ---: | ---: |
| compose-1 | 26 | 5296 |
| compose-2 | 27 | 5347 |
| compose-3 | 27 | 5349 |
| compose-4 | 28 | 5350 |
| compose-5 | 28 | 5350 |
| compose-adaptive | 26 | 5559 |
| compose-misc | 18 | 2860 |
| core-a | 24 | 3013 |
| core-b | 24 | 3021 |
| graphics | 21 | 2070 |
| terminal-hosts | 12 | 1089 |
| sources-examples | 24 | 4309 |
| text-logging-devtools | 9 | 1000 |

## compose-1

- `Tests/CoveUIComposeTests/AdvancedInteractionTests.swift` -> `advanced_interaction_tests`
- `Tests/CoveUIComposeTests/ButtonScrollViewTests.swift` -> `button_scroll_view_tests`
- `Tests/CoveUIComposeTests/ButtonStyleAnimationTests.swift` -> `button_style_animation_tests`
- `Tests/CoveUIComposeTests/CarouselTests.swift` -> `carousel_tests`
- `Tests/CoveUIComposeTests/ControlDensityTests.swift` -> `control_density_tests`
- `Tests/CoveUIComposeTests/CustomLayoutTests.swift` -> `custom_layout_tests`
- `Tests/CoveUIComposeTests/DateRangePickerTests.swift` -> `date_range_picker_tests`
- `Tests/CoveUIComposeTests/DesignSystemTests.swift` -> `design_system_tests`
- `Tests/CoveUIComposeTests/FeedbackComponentTests.swift` -> `feedback_component_tests`
- `Tests/CoveUIComposeTests/ImageFileContentModeTests.swift` -> `image_file_content_mode_tests`
- `Tests/CoveUIComposeTests/LazyGridTests.swift` -> `lazy_grid_tests`
- `Tests/CoveUIComposeTests/ListRowTests.swift` -> `list_row_tests`
- `Tests/CoveUIComposeTests/MenuHierarchyTests.swift` -> `menu_hierarchy_tests`
- `Tests/CoveUIComposeTests/RetainedVisibilityTests.swift` -> `retained_visibility_tests`
- `Tests/CoveUIComposeTests/RichTextTests.swift` -> `rich_text_tests`
- `Tests/CoveUIComposeTests/ScrollOffsetBindingTests.swift` -> `scroll_offset_binding_tests`
- `Tests/CoveUIComposeTests/SingleLineTextTests.swift` -> `single_line_text_tests`
- `Tests/CoveUIComposeTests/SnippetTests.swift` -> `snippet_tests`
- `Tests/CoveUIComposeTests/SplitViewPanelTests.swift` -> `split_view_panel_tests`
- `Tests/CoveUIComposeTests/StatePropagationRegressionTests.swift` -> `state_propagation_regression_tests`
- `Tests/CoveUIComposeTests/StyleIntegrationTests.swift` -> `style_integration_tests`
- `Tests/CoveUIComposeTests/TextFieldNavigationTests.swift` -> `text_field_navigation_tests`
- `Tests/CoveUIComposeTests/TimelineGroupTests.swift` -> `timeline_group_tests`
- `Tests/CoveUIComposeTests/TruncationTests.swift` -> `truncation_tests`
- `Tests/CoveUIComposeTests/ViewportTraitsTests.swift` -> `viewport_traits_tests`
- `Tests/CoveUIComposeTests/VirtualStackVariableRowsTests.swift` -> `virtual_stack_variable_rows_tests`

## compose-2

- `Tests/CoveUIComposeTests/ActionSheetTests.swift` -> `action_sheet_tests`
- `Tests/CoveUIComposeTests/AdaptiveNavigationTests.swift` -> `adaptive_navigation_tests`
- `Tests/CoveUIComposeTests/ButtonPressRecomposeTests.swift` -> `button_press_recompose_tests`
- `Tests/CoveUIComposeTests/ChartAccessibilityTests.swift` -> `chart_accessibility_tests`
- `Tests/CoveUIComposeTests/ComboboxSearchTests.swift` -> `combobox_search_tests`
- `Tests/CoveUIComposeTests/CustomLayoutVisibilityTests.swift` -> `custom_layout_visibility_tests`
- `Tests/CoveUIComposeTests/DemoCrossFadeTests.swift` -> `demo_cross_fade_tests`
- `Tests/CoveUIComposeTests/EditorInteractionTests.swift` -> `editor_interaction_tests`
- `Tests/CoveUIComposeTests/LabelSelectionTests.swift` -> `label_selection_tests`
- `Tests/CoveUIComposeTests/MenuButtonTests.swift` -> `menu_button_tests`
- `Tests/CoveUIComposeTests/MenuPointerRoutingTests.swift` -> `menu_pointer_routing_tests`
- `Tests/CoveUIComposeTests/NavigationStackTests.swift` -> `navigation_stack_tests`
- `Tests/CoveUIComposeTests/PrimitiveLayoutTests.swift` -> `primitive_layout_tests`
- `Tests/CoveUIComposeTests/RefreshTests.swift` -> `refresh_tests`
- `Tests/CoveUIComposeTests/ResponsiveLayoutTests.swift` -> `responsive_layout_tests`
- `Tests/CoveUIComposeTests/SplitButtonTests.swift` -> `split_button_tests`
- `Tests/CoveUIComposeTests/StatusBarPanelTruncationTests.swift` -> `status_bar_panel_truncation_tests`
- `Tests/CoveUIComposeTests/SwiftObservationTests.swift` -> `swift_observation_tests`
- `Tests/CoveUIComposeTests/TextDocumentLayoutTests.swift` -> `text_document_layout_tests`
- `Tests/CoveUIComposeTests/TextFieldLargeDocumentTests.swift` -> `text_field_large_document_tests`
- `Tests/CoveUIComposeTests/TextFieldTests.swift` -> `text_field_tests`
- `Tests/CoveUIComposeTests/ThemeInteractionRegressionTests.swift` -> `theme_interaction_regression_tests`
- `Tests/CoveUIComposeTests/TimePickerTests.swift` -> `time_picker_tests`
- `Tests/CoveUIComposeTests/ToggleTests.swift` -> `toggle_tests`
- `Tests/CoveUIComposeTests/TooltipDeduplicationTests.swift` -> `tooltip_deduplication_tests`
- `Tests/CoveUIComposeTests/TouchDensityTests.swift` -> `touch_density_tests`
- `Tests/CoveUIComposeTests/TypographyRasterTests.swift` -> `typography_raster_tests`

## compose-3

- `Tests/CoveUIComposeTests/AccessibilitySemanticsTests.swift` -> `accessibility_semantics_tests`
- `Tests/CoveUIComposeTests/AnimatedVisibilityTests.swift` -> `animated_visibility_tests`
- `Tests/CoveUIComposeTests/ButtonStyleTests.swift` -> `button_style_tests`
- `Tests/CoveUIComposeTests/ChartOrientationTests.swift` -> `chart_orientation_tests`
- `Tests/CoveUIComposeTests/ChartPerformanceTests.swift` -> `chart_performance_tests`
- `Tests/CoveUIComposeTests/ChartTests.swift` -> `chart_tests`
- `Tests/CoveUIComposeTests/CodeEditingUITests.swift` -> `code_editing_ui_tests`
- `Tests/CoveUIComposeTests/CommandPaletteFooterTests.swift` -> `command_palette_footer_tests`
- `Tests/CoveUIComposeTests/ContainerQueryTests.swift` -> `container_query_tests`
- `Tests/CoveUIComposeTests/ControlMotionTests.swift` -> `control_motion_tests`
- `Tests/CoveUIComposeTests/CursorPipelineTests.swift` -> `cursor_pipeline_tests`
- `Tests/CoveUIComposeTests/DataTableScrollTests.swift` -> `data_table_scroll_tests`
- `Tests/CoveUIComposeTests/ImagePipelineTests.swift` -> `image_pipeline_tests`
- `Tests/CoveUIComposeTests/KeyedReconcilerTests.swift` -> `keyed_reconciler_tests`
- `Tests/CoveUIComposeTests/ListTreeTests.swift` -> `list_tree_tests`
- `Tests/CoveUIComposeTests/MarkdownTests.swift` -> `markdown_tests`
- `Tests/CoveUIComposeTests/NumberFieldTests.swift` -> `number_field_tests`
- `Tests/CoveUIComposeTests/PortalLayoutTests.swift` -> `portal_layout_tests`
- `Tests/CoveUIComposeTests/PortalScopeTests.swift` -> `portal_scope_tests`
- `Tests/CoveUIComposeTests/RangeSliderTests.swift` -> `range_slider_tests`
- `Tests/CoveUIComposeTests/RatingTests.swift` -> `rating_tests`
- `Tests/CoveUIComposeTests/ScrollViewInputTests.swift` -> `scroll_view_input_tests`
- `Tests/CoveUIComposeTests/SectionedListTests.swift` -> `sectioned_list_tests`
- `Tests/CoveUIComposeTests/ShortcutHostTests.swift` -> `shortcut_host_tests`
- `Tests/CoveUIComposeTests/SliderAxisTests.swift` -> `slider_axis_tests`
- `Tests/CoveUIComposeTests/StyleSkeletonTests.swift` -> `style_skeleton_tests`
- `Tests/CoveUIComposeTests/TabViewTests.swift` -> `tab_view_tests`

## compose-4

- `Tests/CoveUIComposeTests/AnimationIntegrationTests.swift` -> `animation_integration_tests`
- `Tests/CoveUIComposeTests/BottomSheetTests.swift` -> `bottom_sheet_tests`
- `Tests/CoveUIComposeTests/ChromeRegressionTests.swift` -> `chrome_regression_tests`
- `Tests/CoveUIComposeTests/DisclosureContainerTests.swift` -> `disclosure_container_tests`
- `Tests/CoveUIComposeTests/DisclosureGroupTests.swift` -> `disclosure_group_tests`
- `Tests/CoveUIComposeTests/DragAndDropTests.swift` -> `drag_and_drop_tests`
- `Tests/CoveUIComposeTests/InteractiveScene.swift` -> `interactive_scene`
- `Tests/CoveUIComposeTests/ListMultipleSelectionTests.swift` -> `list_multiple_selection_tests`
- `Tests/CoveUIComposeTests/MasonryTests.swift` -> `masonry_tests`
- `Tests/CoveUIComposeTests/MenuContentTests.swift` -> `menu_content_tests`
- `Tests/CoveUIComposeTests/ModifierReplayTests.swift` -> `modifier_replay_tests`
- `Tests/CoveUIComposeTests/NativeMenuTests.swift` -> `native_menu_tests`
- `Tests/CoveUIComposeTests/ObservableStateTrackingTests.swift` -> `observable_state_tracking_tests`
- `Tests/CoveUIComposeTests/PopconfirmTests.swift` -> `popconfirm_tests`
- `Tests/CoveUIComposeTests/ProductInteractionRegressionTests.swift` -> `product_interaction_regression_tests`
- `Tests/CoveUIComposeTests/RenderTreeMirrorTests.swift` -> `render_tree_mirror_tests`
- `Tests/CoveUIComposeTests/RouteAndPreloadTests.swift` -> `route_and_preload_tests`
- `Tests/CoveUIComposeTests/ScrollbarTests.swift` -> `scrollbar_tests`
- `Tests/CoveUIComposeTests/SheetTests.swift` -> `sheet_tests`
- `Tests/CoveUIComposeTests/SidebarContentTests.swift` -> `sidebar_content_tests`
- `Tests/CoveUIComposeTests/SwipeListTests.swift` -> `swipe_list_tests`
- `Tests/CoveUIComposeTests/TestSupport.swift` -> `test_support`
- `Tests/CoveUIComposeTests/TextBufferTests.swift` -> `text_buffer_tests`
- `Tests/CoveUIComposeTests/TextSelectionScopeTests.swift` -> `text_selection_scope_tests`
- `Tests/CoveUIComposeTests/ToastAnimationTests.swift` -> `toast_animation_tests`
- `Tests/CoveUIComposeTests/VerticalToolbarTests.swift` -> `vertical_toolbar_tests`
- `Tests/CoveUIComposeTests/ViewThatFitsTests.swift` -> `view_that_fits_tests`
- `Tests/CoveUIComposeTests/WithAnimationPropertiesTests.swift` -> `with_animation_properties_tests`

## compose-5

- `Tests/CoveUIComposeTests/AnimationValueModifierTests.swift` -> `animation_value_modifier_tests`
- `Tests/CoveUIComposeTests/AssetRefFieldTests.swift` -> `asset_ref_field_tests`
- `Tests/CoveUIComposeTests/AsyncImageTests.swift` -> `async_image_tests`
- `Tests/CoveUIComposeTests/BidiTests.swift` -> `bidi_tests`
- `Tests/CoveUIComposeTests/ButtonIconTests.swift` -> `button_icon_tests`
- `Tests/CoveUIComposeTests/CatalogInteractionTests.swift` -> `catalog_interaction_tests`
- `Tests/CoveUIComposeTests/ChartInteractionTests.swift` -> `chart_interaction_tests`
- `Tests/CoveUIComposeTests/ChartViewportTests.swift` -> `chart_viewport_tests`
- `Tests/CoveUIComposeTests/CompositionLocalTests.swift` -> `composition_local_tests`
- `Tests/CoveUIComposeTests/FloatingSurfaceTests.swift` -> `floating_surface_tests`
- `Tests/CoveUIComposeTests/FocusRingTests.swift` -> `focus_ring_tests`
- `Tests/CoveUIComposeTests/FourTreeConsistencyTests.swift` -> `four_tree_consistency_tests`
- `Tests/CoveUIComposeTests/GridLayoutTests.swift` -> `grid_layout_tests`
- `Tests/CoveUIComposeTests/OverlayScrollInputTests.swift` -> `overlay_scroll_input_tests`
- `Tests/CoveUIComposeTests/PopoverTests.swift` -> `popover_tests`
- `Tests/CoveUIComposeTests/ProductComponentTests.swift` -> `product_component_tests`
- `Tests/CoveUIComposeTests/ReorderableListTests.swift` -> `reorderable_list_tests`
- `Tests/CoveUIComposeTests/ScrolledControlClickTests.swift` -> `scrolled_control_click_tests`
- `Tests/CoveUIComposeTests/SemanticMotionTests.swift` -> `semantic_motion_tests`
- `Tests/CoveUIComposeTests/SidebarSlotTests.swift` -> `sidebar_slot_tests`
- `Tests/CoveUIComposeTests/SidebarTests.swift` -> `sidebar_tests`
- `Tests/CoveUIComposeTests/SpringInterruptionTests.swift` -> `spring_interruption_tests`
- `Tests/CoveUIComposeTests/TextEditorAppearanceTests.swift` -> `text_editor_appearance_tests`
- `Tests/CoveUIComposeTests/TextShapeCacheTests.swift` -> `text_shape_cache_tests`
- `Tests/CoveUIComposeTests/ThemeCrossFadeComponentTests.swift` -> `theme_cross_fade_component_tests`
- `Tests/CoveUIComposeTests/VirtualizationEstimateTests.swift` -> `virtualization_estimate_tests`
- `Tests/CoveUIComposeTests/WithAnimationCompletionTests.swift` -> `with_animation_completion_tests`
- `Tests/CoveUIComposeTests/WithAnimationOpacityTests.swift` -> `with_animation_opacity_tests`

## compose-adaptive

- `Tests/CoveUIComposeTests/Adaptive/AdaptiveSidebarTests.swift` -> `adaptive_sidebar_tests`
- `Tests/CoveUIComposeTests/Adaptive/AlertTests.swift` -> `alert_tests`
- `Tests/CoveUIComposeTests/Adaptive/AppBarTests.swift` -> `app_bar_tests`
- `Tests/CoveUIComposeTests/Adaptive/AssetFieldTests.swift` -> `asset_field_tests`
- `Tests/CoveUIComposeTests/Adaptive/CommandPaletteAdaptiveTests.swift` -> `command_palette_adaptive_tests`
- `Tests/CoveUIComposeTests/Adaptive/CompactLabelTests.swift` -> `compact_label_tests`
- `Tests/CoveUIComposeTests/Adaptive/DialogTests.swift` -> `dialog_tests`
- `Tests/CoveUIComposeTests/Adaptive/DisclosureTests.swift` -> `disclosure_tests`
- `Tests/CoveUIComposeTests/Adaptive/EditorHeightTests.swift` -> `editor_height_tests`
- `Tests/CoveUIComposeTests/Adaptive/FieldLayoutTests.swift` -> `field_layout_tests`
- `Tests/CoveUIComposeTests/Adaptive/GridColumnsTests.swift` -> `grid_columns_tests`
- `Tests/CoveUIComposeTests/Adaptive/JsonEditorTests.swift` -> `json_editor_tests`
- `Tests/CoveUIComposeTests/Adaptive/NarrowReflowTests.swift` -> `narrow_reflow_tests`
- `Tests/CoveUIComposeTests/Adaptive/NavigationDrawerTests.swift` -> `navigation_drawer_tests`
- `Tests/CoveUIComposeTests/Adaptive/NavigationRailTests.swift` -> `navigation_rail_tests`
- `Tests/CoveUIComposeTests/Adaptive/RowDensityTests.swift` -> `row_density_tests`
- `Tests/CoveUIComposeTests/Adaptive/SelectionFieldTests.swift` -> `selection_field_tests`
- `Tests/CoveUIComposeTests/Adaptive/SidebarHierarchyTests.swift` -> `sidebar_hierarchy_tests`
- `Tests/CoveUIComposeTests/Adaptive/SidebarResizeTests.swift` -> `sidebar_resize_tests`
- `Tests/CoveUIComposeTests/Adaptive/SplitPresentationTests.swift` -> `split_presentation_tests`
- `Tests/CoveUIComposeTests/Adaptive/SurfaceRootTests.swift` -> `surface_root_tests`
- `Tests/CoveUIComposeTests/Adaptive/TabOverflowTests.swift` -> `tab_overflow_tests`
- `Tests/CoveUIComposeTests/Adaptive/TableColumnsTests.swift` -> `table_columns_tests`
- `Tests/CoveUIComposeTests/Adaptive/TextScaleRows.swift` -> `text_scale_rows`
- `Tests/CoveUIComposeTests/Adaptive/TimelineTests.swift` -> `timeline_tests`
- `Tests/CoveUIComposeTests/Adaptive/ToolbarOverflowTests.swift` -> `toolbar_overflow_tests`

## compose-misc

- `Tests/CoveUIComposeTests/Agent/ControlDiscoveryTests.swift` -> `control_discovery_tests`
- `Tests/CoveUIComposeTests/Animation/AppearanceRevealTests.swift` -> `appearance_reveal_tests`
- `Tests/CoveUIComposeTests/Animation/InteractionTests.swift` -> `interaction_tests`
- `Tests/CoveUIComposeTests/Animation/SequenceLifecycleTests.swift` -> `sequence_lifecycle_tests`
- `Tests/CoveUIComposeTests/Calendar/AdaptiveTests.swift` -> `adaptive_tests`
- `Tests/CoveUIComposeTests/Calendar/KeyboardNavigationTests.swift` -> `keyboard_navigation_tests`
- `Tests/CoveUIComposeTests/Calendar/ModesTests.swift` -> `modes_tests`
- `Tests/CoveUIComposeTests/CodeEditing/GutterTests.swift` -> `gutter_tests`
- `Tests/CoveUIComposeTests/Images/HTTPServer.swift` -> `http_server`
- `Tests/CoveUIComposeTests/Mobile/Fixtures.swift` -> `fixtures`
- `Tests/CoveUIComposeTests/Mobile/PresentationTests.swift` -> `presentation_tests`
- `Tests/CoveUIComposeTests/Scrolling/IndicatorTests.swift` -> `indicator_tests`
- `Tests/CoveUIComposeTests/Scrolling/KeyboardFocusTests.swift` -> `keyboard_focus_tests`
- `Tests/CoveUIComposeTests/Scrolling/NestedScrollRoutingTests.swift` -> `nested_scroll_routing_tests`
- `Tests/CoveUIComposeTests/Scrolling/ScrollPhysicsTests.swift` -> `scroll_physics_tests`
- `Tests/CoveUIComposeTests/Transitions/ScaleTests.swift` -> `scale_tests`
- `Tests/CoveUIComposeTests/Transitions/SurfaceRevealTests.swift` -> `surface_reveal_tests`
- `Tests/CoveUIComposeTests/Tree/BehaviorTests.swift` -> `behavior_tests`

## core-a

- `Tests/CoveUIRuntimeTests/AppKitScrollInputTests.swift` -> `app_kit_scroll_input_tests`
- `Tests/CoveUIRuntimeTests/DisplayRefreshRateTests.swift` -> `display_refresh_rate_tests`
- `Tests/CoveUIRuntimeTests/DrawBatchCoalescingTests.swift` -> `draw_batch_coalescing_tests`
- `Tests/CoveUIRuntimeTests/FallbackStyleTests.swift` -> `fallback_style_tests`
- `Tests/CoveUIRuntimeTests/ImageDecoderTests.swift` -> `image_decoder_tests`
- `Tests/CoveUIRuntimeTests/LayerAwareNodeRendererTests.swift` -> `layer_aware_node_renderer_tests`
- `Tests/CoveUIRuntimeTests/NativeAccessibilityTests.swift` -> `native_accessibility_tests`
- `Tests/CoveUIRuntimeTests/NativeDisplayRefreshTests.swift` -> `native_display_refresh_tests`
- `Tests/CoveUIRuntimeTests/NativeDrawListLifecycleTests.swift` -> `native_draw_list_lifecycle_tests`
- `Tests/CoveUIRuntimeTests/NativeImageAssetTests.swift` -> `native_image_asset_tests`
- `Tests/CoveUIRuntimeTests/NativeShaderLibraryTests.swift` -> `native_shader_library_tests`
- `Tests/CoveUIRuntimeTests/NativeUIBindingReuseTests.swift` -> `native_ui_binding_reuse_tests`
- `Tests/CoveUIRuntimeTests/ProgressiveImageTests.swift` -> `progressive_image_tests`
- `Tests/CoveUIRuntimeTests/Rendering/CompositionPaintTests.swift` -> `composition_paint_tests`
- `Tests/CoveUIRuntimeTests/Rendering/TransientMSAATests.swift` -> `transient_msaa_tests`
- `Tests/CoveUIRuntimeTests/SystemFontInstanceTests.swift` -> `system_font_instance_tests`
- `Tests/CoveUIRuntimeTests/Transforms/HitTestingTests.swift` -> `hit_testing_tests`
- `Tests/CoveUIRuntimeTests/Transforms/ShadowTests.swift` -> `shadow_tests`
- `Tests/Headless/Tests/CoveUISceneTests/DiagnosticPresentationTests.swift` -> `diagnostic_presentation_tests`
- `Tests/Headless/Tests/CoveUISceneTests/DiagnosticsTests.swift` -> `diagnostics_tests`
- `Tests/Headless/Tests/CoveUISceneTests/Input/CaptureCancellationTests.swift` -> `capture_cancellation_tests`
- `Tests/Headless/Tests/CoveUISceneTests/Input/FocusAccessoryTests.swift` -> `focus_accessory_tests`
- `Tests/Headless/Tests/CoveUISceneTests/RenderPipelinePerformanceTests.swift` -> `render_pipeline_performance_tests`
- `Tests/Headless/Tests/CoveUISceneTests/Scrolling/RubberBandTests.swift` -> `rubber_band_tests`

## core-b

- `Tests/CoveUIRuntimeTests/ColorGlyphTests.swift` -> `color_glyph_tests`
- `Tests/CoveUIRuntimeTests/ImageAssetRegistryTests.swift` -> `image_asset_registry_tests`
- `Tests/CoveUIRuntimeTests/NativeAnalyticClipTests.swift` -> `native_analytic_clip_tests`
- `Tests/CoveUIRuntimeTests/NativeDrawListBenchmarkTests.swift` -> `native_draw_list_benchmark_tests`
- `Tests/CoveUIRuntimeTests/NativeDrawListParityTests.swift` -> `native_draw_list_parity_tests`
- `Tests/CoveUIRuntimeTests/NativeRoundedFillTests.swift` -> `native_rounded_fill_tests`
- `Tests/CoveUIRuntimeTests/NativeUIBackdropTests.swift` -> `native_ui_backdrop_tests`
- `Tests/CoveUIRuntimeTests/NativeUIDrawTestSupport.swift` -> `native_ui_draw_test_support`
- `Tests/CoveUIRuntimeTests/NodeTransformRenderTests.swift` -> `node_transform_render_tests`
- `Tests/CoveUIRuntimeTests/Rendering/BackdropOpacityTests.swift` -> `backdrop_opacity_tests`
- `Tests/CoveUIRuntimeTests/Rendering/CircularRevealPixels.swift` -> `circular_reveal_pixels`
- `Tests/CoveUIRuntimeTests/Rendering/EffectReplayTests.swift` -> `effect_replay_tests`
- `Tests/CoveUIRuntimeTests/Rendering/MSAAEvaluationTests.swift` -> `msaa_evaluation_tests`
- `Tests/CoveUIRuntimeTests/Rendering/RevealRequestTests.swift` -> `reveal_request_tests`
- `Tests/CoveUIRuntimeTests/SurfacePaintingTests.swift` -> `surface_painting_tests`
- `Tests/CoveUIRuntimeTests/SurfaceRenderingTests.swift` -> `surface_rendering_tests`
- `Tests/CoveUIRuntimeTests/TextLayoutRegressionTests.swift` -> `text_layout_regression_tests`
- `Tests/CoveUIRuntimeTests/TextTests.swift` -> `text_tests`
- `Tests/CoveUIRuntimeTests/Transforms/PaintTests.swift` -> `paint_tests`
- `Tests/CoveUIRuntimeTests/WheelInputTests.swift` -> `wheel_input_tests`
- `Tests/Headless/Tests/CoveUISceneTests/Animation/CurveTests.swift` -> `curve_tests`
- `Tests/Headless/Tests/CoveUISceneTests/Animation/PlaybackTests.swift` -> `playback_tests`
- `Tests/Headless/Tests/CoveUISceneTests/Layout/MeasurementLifecycleTests.swift` -> `measurement_lifecycle_tests`
- `Tests/Headless/Tests/CoveUISceneTests/Layout/MinimumSizeTests.swift` -> `minimum_size_tests`

## graphics

- `Graphics/Tests/NativeRHITests/MacSurfaceTests.swift` -> `mac_surface_tests`
- `Graphics/Tests/NativeRHITests/Metal/MetalBindingLifetimeTests.swift` -> `metal_binding_lifetime_tests`
- `Graphics/Tests/NativeRHITests/Metal/MetalCapabilityProbeTests.swift` -> `metal_capability_probe_tests`
- `Graphics/Tests/NativeRHITests/Metal/MetalClearReadbackTests.swift` -> `metal_clear_readback_tests`
- `Graphics/Tests/NativeRHITests/Metal/MetalFrameLifecycleTests.swift` -> `metal_frame_lifecycle_tests`
- `Graphics/Tests/NativeRHITests/Metal/MetalRayQueryTests.swift` -> `metal_ray_query_tests`
- `Graphics/Tests/NativeRHITests/Metal/MetalRenderBindingTests.swift` -> `metal_render_binding_tests`
- `Graphics/Tests/NativeRHITests/Metal/MetalSlangTests.swift` -> `metal_slang_tests`
- `Graphics/Tests/NativeRHITests/Metal/MetalStorageHazardTests.swift` -> `metal_storage_hazard_tests`
- `Graphics/Tests/NativeRHITests/NativeGPUContractTests.swift` -> `native_gpu_contract_tests`
- `Graphics/Tests/NativeRHITests/NativeMultisampleTests.swift` -> `native_multisample_tests`
- `Graphics/Tests/NativeRHITests/NativeRHISmokeTests.swift` -> `native_rhi_smoke_tests`
- `Graphics/Tests/NativeRHITests/NativeTestDevice.swift` -> `native_test_device`
- `Graphics/Tests/NativeRHITests/ResourcePreparationTests.swift` -> `resource_preparation_tests`
- `Graphics/Tests/NativeRHITests/ShaderFixtureCompiler.swift` -> `shader_fixture_compiler`
- `Graphics/Tests/NativeRHITests/ShaderSpecializationTests.swift` -> `shader_specialization_tests`
- `Graphics/Tests/NativeRHITests/SwapchainLifetimeTests.swift` -> `swapchain_lifetime_tests`
- `Graphics/Tests/NativeRHITests/Vulkan/PresentationPolicyTests.swift` -> `presentation_policy_tests`
- `Graphics/Tests/NativeRHITests/Vulkan/VulkanCapabilityProbeTests.swift` -> `vulkan_capability_probe_tests`
- `Graphics/Tests/NativeRHITests/Vulkan/VulkanSlangTests.swift` -> `vulkan_slang_tests`
- `Graphics/Tests/NativeRHITests/Vulkan/VulkanUploadReadbackTests.swift` -> `vulkan_upload_readback_tests`

## terminal-hosts

> Note: `Tests/CoveUITests/NativeWindowLifecycleTests.swift` and
> `Tests/CoveUITests/WindowChromeInteractionTests.swift` target crate dir `Hosts`
> (package `cove-hosts`), not `Runtime`. The code they exercise lives in
> `Hosts/rust/desktop/window/native_renderer.rs` and the shell chrome host.

- `Hosts/Android/Contract/Tests/CoveAndroidContractTests/FrameClockTests.swift` -> `frame_clock_tests`
- `Hosts/Android/Contract/Tests/CoveAndroidContractTests/LaunchTests.swift` -> `launch_tests`
- `Terminal/Tests/CoveUITerminalTests/EngineTests.swift` -> `engine_tests`
- `Terminal/Tests/CoveUITerminalTests/InteractionTests.swift` -> `interaction_tests`
- `Terminal/Tests/CoveUITerminalTests/KeyRoutingTests.swift` -> `key_routing_tests`
- `Terminal/Tests/CoveUITerminalTests/RenderingFeatureTests.swift` -> `rendering_feature_tests`
- `Terminal/Tests/CoveUITerminalTests/SessionTests.swift` -> `session_tests`
- `Terminal/Tests/CoveUITerminalTests/ViewTests.swift` -> `view_tests`
- `Terminal/Tests/CoveUITerminalTests/WindowsCommandLineTests.swift` -> `windows_command_line_tests`
- `Terminal/Tests/CoveUITerminalTests/WindowsSessionTests.swift` -> `windows_session_tests`
- `Tests/CoveUIAndroidTests/SessionTests.swift` -> `session_tests`
- `Tests/CoveUIAndroidTests/SurfaceMetricsTests.swift` -> `surface_metrics_tests`

## sources-examples

- `Examples/Counter/Tests/CounterExampleTests/ComponentAnalysisTests.swift` -> `component_analysis_tests`
- `Examples/Counter/Tests/CounterExampleTests/InspectionTests.swift` -> `inspection_tests`
- `Examples/Counter/Tests/CounterExampleTests/SharedSceneTests.swift` -> `shared_scene_tests`
- `Examples/Mobile/Tests/AppContentTests/Model.swift` -> `model`
- `Examples/Tests/GalleryContentTests/AdaptiveShellTests.swift` -> `adaptive_shell_tests`
- `Examples/Tests/GalleryContentTests/AnimationSystemTests.swift` -> `animation_system_tests`
- `Examples/Tests/GalleryContentTests/AnimationTests.swift` -> `animation_tests`
- `Examples/Tests/GalleryContentTests/AppBarTests.swift` -> `app_bar_tests`
- `Examples/Tests/GalleryContentTests/DrawerTests.swift` -> `drawer_tests`
- `Examples/Tests/GalleryContentTests/GalleryTests.swift` -> `gallery_tests`
- `Examples/Tests/GalleryContentTests/ImageLayoutTests.swift` -> `image_layout_tests`
- `Examples/Tests/GalleryContentTests/ListRowLayoutTests.swift` -> `list_row_layout_tests`
- `Examples/Tests/GalleryContentTests/NavigationStateTests.swift` -> `navigation_state_tests`
- `Examples/Tests/GalleryContentTests/RailTests.swift` -> `rail_tests`
- `Examples/Tests/GalleryContentTests/RatingLayoutTests.swift` -> `rating_layout_tests`
- `Examples/Tests/GalleryContentTests/SelectionShellTests.swift` -> `selection_shell_tests`
- `Examples/Tests/GalleryContentTests/SplitTests.swift` -> `split_tests`
- `Examples/Tests/GalleryContentTests/SurfaceTraitsTests.swift` -> `surface_traits_tests`
- `Examples/Tests/GalleryContentTests/TextEditorLayoutTests.swift` -> `text_editor_layout_tests`
- `Tests/CoveUIWebViewTests/WebViewGeometryTests.swift` -> `web_view_geometry_tests`
- `Tests/CoveUIWebViewTests/WebViewLifecycleTests.swift` -> `web_view_lifecycle_tests`
- `Tests/CoveUIWorkspaceTests/PanelWorkspaceIntegrationTests.swift` -> `panel_workspace_integration_tests`
- `Tests/CoveUIWorkspaceTests/WorkspaceControllerTests.swift` -> `workspace_controller_tests`
- `Tests/CoveUIWorkspaceTests/WorkspaceViewTests.swift` -> `workspace_view_tests`

## text-logging-devtools

- `Logging/Tests/CoveLoggingTests/StartupTests.swift` -> `startup_tests`
- `Logging/Tests/CoveLoggingTests/StorageTests.swift` -> `storage_tests`
- `Tests/CoveUIDevToolsTests/DevServerInputAndStateTests.swift` -> `dev_server_input_and_state_tests`
- `Tests/CoveUIDevToolsTests/DevServerSmokeTests.swift` -> `dev_server_smoke_tests`
- `Tests/CoveUIDevToolsTests/MirrorBackdropTests.swift` -> `mirror_backdrop_tests`
- `Tests/CoveUIDevToolsTests/TimingPublisherTests.swift` -> `timing_publisher_tests`
- `Tests/CoveUITests/NativeWindowLifecycleTests.swift` -> `native_window_lifecycle_tests`
- `Tests/CoveUITests/WindowChromeInteractionTests.swift` -> `window_chrome_interaction_tests`
- `Text/Tests/CoveUITextTests/FontPipelineTests.swift` -> `font_pipeline_tests`
