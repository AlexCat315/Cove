# Remaining test migration partition (wave 2)

| group | files | Swift lines |
| --- | ---: | ---: |
| graphics-rest | 16 | 1790 |
| terminal-hosts | 11 | 985 |
| examples-gallery | 19 | 2370 |
| sources-workspace | 5 | 1939 |
| core-b1 | 16 | 2164 |
| core-b2 | 17 | 2195 |

## graphics-rest

- `Graphics/Tests/NativeRHITests/MacSurfaceTests.swift` -> `mac_surface_tests`
- `Graphics/Tests/NativeRHITests/Metal/MetalBindingLifetimeTests.swift` -> `metal_binding_lifetime_tests`
- `Graphics/Tests/NativeRHITests/Metal/MetalFrameLifecycleTests.swift` -> `metal_frame_lifecycle_tests`
- `Graphics/Tests/NativeRHITests/Metal/MetalRayQueryTests.swift` -> `metal_ray_query_tests`
- `Graphics/Tests/NativeRHITests/Metal/MetalRenderBindingTests.swift` -> `metal_render_binding_tests`
- `Graphics/Tests/NativeRHITests/Metal/MetalSlangTests.swift` -> `metal_slang_tests`
- `Graphics/Tests/NativeRHITests/Metal/MetalStorageHazardTests.swift` -> `metal_storage_hazard_tests`
- `Graphics/Tests/NativeRHITests/NativeGPUContractTests.swift` -> `native_gpu_contract_tests`
- `Graphics/Tests/NativeRHITests/NativeMultisampleTests.swift` -> `native_multisample_tests`
- `Graphics/Tests/NativeRHITests/NativeTestDevice.swift` -> `native_test_device`
- `Graphics/Tests/NativeRHITests/ResourcePreparationTests.swift` -> `resource_preparation_tests`
- `Graphics/Tests/NativeRHITests/ShaderFixtureCompiler.swift` -> `shader_fixture_compiler`
- `Graphics/Tests/NativeRHITests/ShaderSpecializationTests.swift` -> `shader_specialization_tests`
- `Graphics/Tests/NativeRHITests/SwapchainLifetimeTests.swift` -> `swapchain_lifetime_tests`
- `Graphics/Tests/NativeRHITests/Vulkan/VulkanCapabilityProbeTests.swift` -> `vulkan_capability_probe_tests`
- `Graphics/Tests/NativeRHITests/Vulkan/VulkanUploadReadbackTests.swift` -> `vulkan_upload_readback_tests`

## terminal-hosts

- `Hosts/Android/Contract/Tests/CoveAndroidContractTests/FrameClockTests.swift` -> `frame_clock_tests`
- `Hosts/Android/Contract/Tests/CoveAndroidContractTests/LaunchTests.swift` -> `launch_tests`
- `Terminal/Tests/CoveUITerminalTests/EngineTests.swift` -> `engine_tests`
- `Terminal/Tests/CoveUITerminalTests/KeyRoutingTests.swift` -> `key_routing_tests`
- `Terminal/Tests/CoveUITerminalTests/RenderingFeatureTests.swift` -> `rendering_feature_tests`
- `Terminal/Tests/CoveUITerminalTests/SessionTests.swift` -> `session_tests`
- `Terminal/Tests/CoveUITerminalTests/ViewTests.swift` -> `view_tests`
- `Terminal/Tests/CoveUITerminalTests/WindowsCommandLineTests.swift` -> `windows_command_line_tests`
- `Terminal/Tests/CoveUITerminalTests/WindowsSessionTests.swift` -> `windows_session_tests`
- `Tests/CoveUIAndroidTests/SessionTests.swift` -> `session_tests`
- `Tests/CoveUIAndroidTests/SurfaceMetricsTests.swift` -> `surface_metrics_tests`

## examples-gallery

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

## sources-workspace

- `Tests/CoveUIWebViewTests/WebViewGeometryTests.swift` -> `web_view_geometry_tests`
- `Tests/CoveUIWebViewTests/WebViewLifecycleTests.swift` -> `web_view_lifecycle_tests`
- `Tests/CoveUIWorkspaceTests/PanelWorkspaceIntegrationTests.swift` -> `panel_workspace_integration_tests`
- `Tests/CoveUIWorkspaceTests/WorkspaceControllerTests.swift` -> `workspace_controller_tests`
- `Tests/CoveUIWorkspaceTests/WorkspaceViewTests.swift` -> `workspace_view_tests`

## core-b1

- `Tests/CoveUIRuntimeTests/ColorGlyphTests.swift` -> `color_glyph_tests`
- `Tests/CoveUIRuntimeTests/DrawBatchCoalescingTests.swift` -> `draw_batch_coalescing_tests`
- `Tests/CoveUIRuntimeTests/ImageAssetRegistryTests.swift` -> `image_asset_registry_tests`
- `Tests/CoveUIRuntimeTests/NativeAccessibilityTests.swift` -> `native_accessibility_tests`
- `Tests/CoveUIRuntimeTests/NativeDrawListBenchmarkTests.swift` -> `native_draw_list_benchmark_tests`
- `Tests/CoveUIRuntimeTests/NativeRoundedFillTests.swift` -> `native_rounded_fill_tests`
- `Tests/CoveUIRuntimeTests/NodeTransformRenderTests.swift` -> `node_transform_render_tests`
- `Tests/CoveUIRuntimeTests/Rendering/BackdropOpacityTests.swift` -> `backdrop_opacity_tests`
- `Tests/CoveUIRuntimeTests/Rendering/CircularRevealPixels.swift` -> `circular_reveal_pixels`
- `Tests/CoveUIRuntimeTests/Rendering/TransientMSAATests.swift` -> `transient_msaa_tests`
- `Tests/CoveUIRuntimeTests/SurfaceRenderingTests.swift` -> `surface_rendering_tests`
- `Tests/CoveUIRuntimeTests/TextLayoutRegressionTests.swift` -> `text_layout_regression_tests`
- `Tests/CoveUIRuntimeTests/TextTests.swift` -> `text_tests`
- `Tests/CoveUIRuntimeTests/WheelInputTests.swift` -> `wheel_input_tests`
- `Tests/Headless/Tests/CoveUISceneTests/Animation/CurveTests.swift` -> `curve_tests`
- `Tests/Headless/Tests/CoveUISceneTests/DiagnosticPresentationTests.swift` -> `diagnostic_presentation_tests`

## core-b2

- `Tests/CoveUIRuntimeTests/NativeAnalyticClipTests.swift` -> `native_analytic_clip_tests`
- `Tests/CoveUIRuntimeTests/NativeDrawListLifecycleTests.swift` -> `native_draw_list_lifecycle_tests`
- `Tests/CoveUIRuntimeTests/NativeDrawListParityTests.swift` -> `native_draw_list_parity_tests`
- `Tests/CoveUIRuntimeTests/NativeImageAssetTests.swift` -> `native_image_asset_tests`
- `Tests/CoveUIRuntimeTests/NativeShaderLibraryTests.swift` -> `native_shader_library_tests`
- `Tests/CoveUIRuntimeTests/NativeUIBackdropTests.swift` -> `native_ui_backdrop_tests`
- `Tests/CoveUIRuntimeTests/NativeUIBindingReuseTests.swift` -> `native_ui_binding_reuse_tests`
- `Tests/CoveUIRuntimeTests/NativeUIDrawTestSupport.swift` -> `native_ui_draw_test_support`
- `Tests/CoveUIRuntimeTests/Rendering/EffectReplayTests.swift` -> `effect_replay_tests`
- `Tests/CoveUIRuntimeTests/Rendering/MSAAEvaluationTests.swift` -> `msaa_evaluation_tests`
- `Tests/CoveUIRuntimeTests/Rendering/RevealRequestTests.swift` -> `reveal_request_tests`
- `Tests/CoveUIRuntimeTests/SurfacePaintingTests.swift` -> `surface_painting_tests`
- `Tests/CoveUIRuntimeTests/Transforms/PaintTests.swift` -> `paint_tests`
- `Tests/Headless/Tests/CoveUISceneTests/Animation/PlaybackTests.swift` -> `playback_tests`
- `Tests/Headless/Tests/CoveUISceneTests/DiagnosticsTests.swift` -> `diagnostics_tests`
- `Tests/Headless/Tests/CoveUISceneTests/Layout/MeasurementLifecycleTests.swift` -> `measurement_lifecycle_tests`
- `Tests/Headless/Tests/CoveUISceneTests/Layout/MinimumSizeTests.swift` -> `minimum_size_tests`
