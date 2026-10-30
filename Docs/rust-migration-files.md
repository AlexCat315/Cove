# Rust 迁移逐文件映射表

由 `Docs/rust-migration-files.json` 生成，状态必须与该 JSON 一致。`（计划）` 表示 Rust 目标路径尚未创建，只是登记的目标位置。

## 口径

- 生产源码：排除测试目录、SwiftPM 清单、CLI 项目模板与所有构建/依赖产物。
- 排除路径：`**/.build/**`、`third-party/**`、`vendor/**`、`target/**`、`dist/**`、`Artifacts/**`、`build/**`、`node_modules/**`、`.git/**`。
- `Tests/**` 与 `**/Tests/**` 归为测试源码（kind=`test`）；`Package.swift` 归为清单（kind=`manifest`）；`**/Templates/**` 归为模板（kind=`template`）。
- 状态：`未开始` / `进行中（有缺项）` / `已写入（未验证）` / `Cargo 清单已建立`。`已写入` 不代表通过编译、行为对照或平台验收。

## 汇总

| 范围 | 文件数 | 已写入 | 进行中 | 未开始 |
| --- | ---: | ---: | ---: | ---: |
| 生产源码 | 984 | 460 | 47 | 477 |
| 测试源码 | 424 | 0 | 0 | 424 |
| SwiftPM 清单 | 25 | 19 | 0 | 6 |
| 项目模板 | 5 | 0 | 0 | 5 |
| 合计 | 1438 | — | — | — |

## 生产源码逐文件映射

### Core

共 97 个文件：已写入 97，进行中 0，未开始 0。

| 源文件 | 行 | 状态 | Rust/C 目标 | 依赖或阻塞 |
| --- | ---: | --- | --- | --- |
| `Core/Sources/CoveUICore/Adaptive/Environment.swift` | 82 | 已写入（未验证） | `Core/rust/adaptive_environment.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/Adaptive/TextScale.swift` | 44 | 已写入（未验证） | `Core/rust/text_scale.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/AnalyticClipper.swift` | 62 | 已写入（未验证） | `Core/rust/analytic_clipper.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/AnalyticStroke.swift` | 80 | 已写入（未验证） | `Core/rust/analytic_stroke.rs`<br>`Core/rust/shape_kernels.c` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/Bidi.swift` | 267 | 已写入（未验证） | `Core/rust/bidi.rs`<br>`Core/rust/text_kernels.c` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/Binding.swift` | 32 | 已写入（未验证） | `Core/rust/binding.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/Bitmap.swift` | 21 | 已写入（未验证） | `Core/rust/bitmap.rs`<br>`Core/rust/draw_list_resources.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/Color.swift` | 56 | 已写入（未验证） | `Core/rust/geometry.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/ContentScale.swift` | 7 | 已写入（未验证） | `Core/rust/content_scale.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/DrawList.swift` | 1026 | 已写入（未验证） | `Core/rust/draw.rs`<br>`Core/rust/draw_primitives.rs`<br>`Core/rust/shape_kernels.c` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/DrawListResources.swift` | 25 | 已写入（未验证） | `Core/rust/draw_list_resources.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/DynamicProperty.swift` | 5 | 已写入（未验证） | `Core/rust/dynamic_property.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/Font.swift` | 50 | 已写入（未验证） | `Core/rust/font.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/GeometryKernels.swift` | 62 | 已写入（未验证） | `Core/rust/geometry_kernels.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/GlyphMetrics.swift` | 66 | 已写入（未验证） | `Core/rust/glyph_metrics.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/LineBreaks.swift` | 45 | 已写入（未验证） | `Core/rust/line_breaks.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/LinearGradient.swift` | 53 | 已写入（未验证） | `Core/rust/linear_gradient.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/Mipmaps.swift` | 52 | 已写入（未验证） | `Core/rust/bitmap.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/RoundedClip.swift` | 124 | 已写入（未验证） | `Core/rust/rounded_clip.rs`<br>`Core/rust/shape_kernels.c` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/ShapedGlyph.swift` | 27 | 已写入（未验证） | `Core/rust/shaped_glyph.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/State.swift` | 154 | 已写入（未验证） | `Core/rust/state.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/StateReadTracking.swift` | 431 | 已写入（未验证） | `Core/rust/state_read_tracking.rs`<br>`Core/rust/state.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/SurfaceShadow.swift` | 55 | 已写入（未验证） | `Core/rust/surface_shadow.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/TextLayout.swift` | 381 | 已写入（未验证） | `Core/rust/text_layout.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/TextMeasureSlot.swift` | 67 | 已写入（未验证） | `Core/rust/text_measure_slot.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/UIReveal.swift` | 43 | 已写入（未验证） | `Core/rust/ui_reveal.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/UIShader.swift` | 107 | 已写入（未验证） | `Core/rust/ui_shader.rs`<br>`Core/rust/ui.wgsl` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/UIVertex.swift` | 41 | 已写入（未验证） | `Core/rust/draw.rs`<br>`Core/rust/geometry.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/Accessibility.swift` | 252 | 已写入（未验证） | `Core/rust/scene/accessibility.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/AmbientSlot.swift` | 17 | 已写入（未验证） | `Core/rust/scene/ambient_slot.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/Animation/ActiveAnimationContext.swift` | 166 | 已写入（未验证） | `Core/rust/scene/animation/context.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/Animation/AnimatableProperty.swift` | 118 | 已写入（未验证） | `Core/rust/scene/animatable_property.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/Animation/AnimatableWrite.swift` | 193 | 已写入（未验证） | `Core/rust/scene/animatable_write.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/Animation/Animation.swift` | 249 | 已写入（未验证） | `Core/rust/scene/animation/animation.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/Animation/AnimationController.swift` | 147 | 已写入（未验证） | `Core/rust/scene/animation/controller.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/Animation/Curves/Linear.swift` | 38 | 已写入（未验证） | `Core/rust/scene/animation/linear.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/Animation/Curves/Oscillation.swift` | 23 | 已写入（未验证） | `Core/rust/scene/animation/animation.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/Animation/Curves/Steps.swift` | 19 | 已写入（未验证） | `Core/rust/scene/animation/animation.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/Animation/Interpolatable.swift` | 83 | 已写入（未验证） | `Core/rust/scene/animation/interpolatable.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/Animation/Keyframes/Timeline.swift` | 41 | 已写入（未验证） | `Core/rust/scene/animation/timeline.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/Animation/MotionVector.swift` | 13 | 已写入（未验证） | `Core/rust/scene/animation/motion_vector.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/Animation/Playback.swift` | 30 | 已写入（未验证） | `Core/rust/scene/animation/playback.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/Animation/Scheduler.swift` | 96 | 已写入（未验证） | `Core/rust/scene/animation/scheduler.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/Animation/SpringSolution.swift` | 142 | 已写入（未验证） | `Core/rust/scene/animation/spring_solution.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/ComponentDiagnostics.swift` | 42 | 已写入（未验证） | `Core/rust/scene/component_diagnostics.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/CompositionLocal.swift` | 39 | 已写入（未验证） | `Core/rust/scene/composition_local.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/ElementID.swift` | 44 | 已写入（未验证） | `Core/rust/scene/element_id.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/Exports.swift` | 3 | 已写入（未验证） | `Core/rust/scene/mod.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/FlexProps.swift` | 238 | 已写入（未验证） | `Core/rust/scene/flex_props.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/Input/EventDispatcher.swift` | 622 | 已写入（未验证） | `Core/rust/scene/input/event_dispatcher.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/Input/FocusAccessoryResource.swift` | 17 | 已写入（未验证） | `Core/rust/scene/input/focus_accessory_resource.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/Input/FocusChain.swift` | 266 | 已写入（未验证） | `Core/rust/scene/input/focus_chain.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/Input/HitTester.swift` | 97 | 已写入（未验证） | `Core/rust/scene/input/hit_tester.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/Input/InteractionRegistry.swift` | 442 | 已写入（未验证） | `Core/rust/scene/input/interaction_registry.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/Input/ModalFocusResource.swift` | 24 | 已写入（未验证） | `Core/rust/scene/input/modal_focus_resource.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/Input/PlatformInputContext.swift` | 131 | 已写入（未验证） | `Core/rust/scene/input/platform_input_context.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/Input/PointerCapture.swift` | 43 | 已写入（未验证） | `Core/rust/scene/input/pointer_capture.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/Input/TabStop.swift` | 21 | 已写入（未验证） | `Core/rust/scene/input/tab_stop.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/Input/WheelGesture.swift` | 49 | 已写入（未验证） | `Core/rust/scene/input/wheel_gesture.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/Input/WheelRoutingAttachment.swift` | 42 | 已写入（未验证） | `Core/rust/scene/input/wheel_routing_attachment.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/InvalidationLog.swift` | 126 | 已写入（未验证） | `Core/rust/scene/invalidation_log.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/LayerAwareNodeRenderer.swift` | 303 | 已写入（未验证） | `Core/rust/scene/layer_aware_node_renderer.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/Layout/CustomLayout.swift` | 174 | 已写入（未验证） | `Core/rust/scene/layout_node.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/Layout/LayoutDirection.swift` | 13 | 已写入（未验证） | `Core/rust/scene/environment_hooks.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/Layout/Node+LayoutVisibility.swift` | 21 | 已写入（未验证） | `Core/rust/scene/layout_node.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/LayoutNode.swift` | 636 | 已写入（未验证） | `Core/rust/scene/layout_node.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/LayoutPass.swift` | 40 | 已写入（未验证） | `Core/rust/scene/layout_pass.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/Node.swift` | 835 | 已写入（未验证） | `Core/rust/scene/node.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/NodeGeometry.swift` | 57 | 已写入（未验证） | `Core/rust/scene/node_geometry.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/NodeRenderer.swift` | 159 | 已写入（未验证） | `Core/rust/scene/node_renderer.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/NodeResource.swift` | 23 | 已写入（未验证） | `Core/rust/scene/node_resource.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/NodeTransform.swift` | 173 | 已写入（未验证） | `Core/rust/scene/node_transform.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/NodeTree.swift` | 104 | 已写入（未验证） | `Core/rust/scene/node_tree.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/ObservableObject.swift` | 29 | 已写入（未验证） | `Core/rust/scene/observable_object.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/ObservableStateTracking.swift` | 184 | 已写入（未验证） | `Core/rust/scene/observable_state_tracking.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/PerformanceTimeline.swift` | 75 | 已写入（未验证） | `Core/rust/scene/performance_timeline.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/Recomposer.swift` | 138 | 已写入（未验证） | `Core/rust/scene/recomposer.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/ReducedMotion.swift` | 13 | 已写入（未验证） | `Core/rust/scene/environment_hooks.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/RenderCulling.swift` | 17 | 已写入（未验证） | `Core/rust/scene/render_culling.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/RenderTree.swift` | 328 | 已写入（未验证） | `Core/rust/scene/render_tree.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/Rendering/ContentMask.swift` | 13 | 已写入（未验证） | `Core/rust/scene/rendering/content_mask.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/Rendering/ScaledPainting.swift` | 36 | 已写入（未验证） | `Core/rust/scene/rendering/scaled_painting.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/Rendering/Snapshot.swift` | 54 | 已写入（未验证） | `Core/rust/scene/rendering/snapshot.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/Scrolling/AxisMotion.swift` | 50 | 已写入（未验证） | `Core/rust/scene/scrolling/animator.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/Scrolling/Reveal.swift` | 22 | 已写入（未验证） | `Core/rust/scene/scrolling/reveal.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/Scrolling/RubberBand.swift` | 70 | 已写入（未验证） | `Core/rust/scene/scrolling/physics.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/Scrolling/ScrollAnimator.swift` | 573 | 已写入（未验证） | `Core/rust/scene/scrolling/animator.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/Scrolling/ScrollPhysics.swift` | 197 | 已写入（未验证） | `Core/rust/scene/scrolling/physics.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/Scrolling/ScrollSpring.swift` | 65 | 已写入（未验证） | `Core/rust/scene/scrolling/physics.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/Scrolling/ScrollVelocityTracker.swift` | 112 | 已写入（未验证） | `Core/rust/scene/scrolling/physics.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/StateRegistry.swift` | 50 | 已写入（未验证） | `Core/rust/scene/state_registry.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/SurfacePainting.swift` | 237 | 已写入（未验证） | `Core/rust/scene/surface_painting.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/TextInputAttachment.swift` | 43 | 已写入（未验证） | `Core/rust/scene/input/text_input.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/TextInputClient.swift` | 26 | 已写入（未验证） | `Core/rust/scene/input/text_input.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/TimingTrace.swift` | 54 | 已写入（未验证） | `Core/rust/scene/timing_trace.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/UIWorkScheduler.swift` | 11 | 已写入（未验证） | `Core/rust/scene/environment_hooks.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUIScene/WindowReveal.swift` | 26 | 已写入（未验证） | `Core/rust/scene/environment_hooks.rs` | 未编译、未行为对照 |

### PlatformCore

共 6 个文件：已写入 6，进行中 0，未开始 0。

| 源文件 | 行 | 状态 | Rust/C 目标 | 依赖或阻塞 |
| --- | ---: | --- | --- | --- |
| `PlatformCore/Sources/CovePlatformCore/Cursor.swift` | 25 | 已写入（未验证） | `PlatformCore/rust/lib.rs` | 未编译、未行为对照 |
| `PlatformCore/Sources/CovePlatformCore/InputEvent.swift` | 284 | 已写入（未验证） | `PlatformCore/rust/lib.rs` | 未编译、未行为对照 |
| `PlatformCore/Sources/CovePlatformCore/NativeMenuSnapshot.swift` | 65 | 已写入（未验证） | `PlatformCore/rust/lib.rs` | 未编译、未行为对照 |
| `PlatformCore/Sources/CovePlatformCore/PackageResourceBundle.swift` | 110 | 已写入（未验证） | `PlatformCore/rust/resources.rs` | 未编译、未行为对照 |
| `PlatformCore/Sources/CovePlatformCore/ScrollInput.swift` | 46 | 已写入（未验证） | `PlatformCore/rust/lib.rs` | 未编译、未行为对照 |
| `PlatformCore/Sources/CovePlatformCore/WindowID.swift` | 10 | 已写入（未验证） | `PlatformCore/rust/lib.rs` | 未编译、未行为对照 |

### System

共 17 个文件：已写入 16，进行中 0，未开始 1。

| 源文件 | 行 | 状态 | Rust/C 目标 | 依赖或阻塞 |
| --- | ---: | --- | --- | --- |
| `System/Sources/CoveFileSystem/AtomicFile.swift` | 27 | 已写入（未验证） | `System/rust/files.rs`<br>`System/rust/atomic_file.rs` | 未编译、未行为对照 |
| `System/Sources/CoveFileSystem/FileLock.swift` | 20 | 已写入（未验证） | `System/rust/files.rs`<br>`System/rust/file_lock.rs` | 未编译、未行为对照 |
| `System/Sources/CoveFileSystem/Paths.swift` | 140 | 已写入（未验证） | `System/rust/files.rs`<br>`System/rust/paths.rs` | 未编译、未行为对照 |
| `System/Sources/CoveFileSystem/Permissions.swift` | 28 | 已写入（未验证） | `System/rust/files.rs`<br>`System/rust/permissions.rs` | 未编译、未行为对照 |
| `System/Sources/CoveFileSystem/Replacement.swift` | 40 | 已写入（未验证） | `System/rust/files.rs`<br>`System/rust/replacement.rs` | 未编译、未行为对照 |
| `System/Sources/CoveFileSystem/Trash.swift` | 20 | 已写入（未验证） | `System/rust/files.rs`<br>`System/rust/trash.rs` | 未编译、未行为对照 |
| `System/Sources/CoveProcess/Command.swift` | 65 | 已写入（未验证） | `System/rust/files.rs`<br>`System/rust/command.rs` | 未编译、未行为对照 |
| `System/Sources/CoveProcess/EventBuffer.swift` | 98 | 已写入（未验证） | `System/rust/files.rs`<br>`System/rust/event_buffer.rs` | 未编译、未行为对照 |
| `System/Sources/CoveProcess/IOState.swift` | 29 | 已写入（未验证） | `System/rust/files.rs`<br>`System/rust/io_state.rs` | 未编译、未行为对照 |
| `System/Sources/CoveProcess/NativeProcess.swift` | 94 | 已写入（未验证） | `System/rust/files.rs`<br>`System/rust/native_process.rs` | 未编译、未行为对照 |
| `System/Sources/CoveProcess/PipeRead.swift` | 48 | 已写入（未验证） | `System/rust/files.rs`<br>`System/rust/pipe_read.rs` | 未编译、未行为对照 |
| `System/Sources/CoveProcess/Platform.swift` | 34 | 已写入（未验证） | `System/rust/files.rs`<br>`System/rust/platform.rs` | 未编译、未行为对照 |
| `System/Sources/CoveProcess/Run.swift` | 55 | 已写入（未验证） | `System/rust/files.rs`<br>`System/rust/run.rs` | 未编译、未行为对照 |
| `System/Sources/CoveProcess/Session.swift` | 155 | 已写入（未验证） | `System/rust/files.rs`<br>`System/rust/session.rs` | 未编译、未行为对照 |
| `System/Sources/CoveProcess/ToolLocator.swift` | 53 | 已写入（未验证） | `System/rust/files.rs`<br>`System/rust/tool_locator.rs` | 未编译、未行为对照 |
| `System/Sources/CoveProcess/WindowsLaunch.swift` | 57 | 已写入（未验证） | `System/rust/files.rs`<br>`System/rust/windows_launch.rs` | 未编译、未行为对照 |
| `System/Sources/CoveProcessFixture/main.swift` | 41 | 未开始 | — | 测试辅助进程，测试迁移阶段重建 |

### Logging

共 15 个文件：已写入 15，进行中 0，未开始 0。

| 源文件 | 行 | 状态 | Rust/C 目标 | 依赖或阻塞 |
| --- | ---: | --- | --- | --- |
| `Logging/Sources/CoveLogCore/Configuration/Configuration.swift` | 117 | 已写入（未验证） | `Logging/rust/configuration.rs` | 未编译、未行为对照 |
| `Logging/Sources/CoveLogCore/Configuration/Environment.swift` | 35 | 已写入（未验证） | `Logging/rust/environment.rs` | 未编译、未行为对照 |
| `Logging/Sources/CoveLogCore/Exports.swift` | 1 | 已写入（未验证） | `Logging/rust/exports.rs` | 未编译、未行为对照 |
| `Logging/Sources/CoveLogCore/Records/Context.swift` | 12 | 已写入（未验证） | `Logging/rust/context.rs` | 未编译、未行为对照 |
| `Logging/Sources/CoveLogCore/Records/Record.swift` | 79 | 已写入（未验证） | `Logging/rust/record.rs` | 未编译、未行为对照 |
| `Logging/Sources/CoveLogging/Exports.swift` | 1 | 已写入（未验证） | `Logging/rust/logging_exports.rs` | 未编译、未行为对照 |
| `Logging/Sources/CoveLogging/Outputs/File.swift` | 66 | 已写入（未验证） | `Logging/rust/file.rs` | 未编译、未行为对照 |
| `Logging/Sources/CoveLogging/Outputs/Formatting.swift` | 23 | 已写入（未验证） | `Logging/rust/formatting.rs` | 未编译、未行为对照 |
| `Logging/Sources/CoveLogging/Runtime/Executor.swift` | 19 | 已写入（未验证） | `Logging/rust/executor.rs` | 未编译、未行为对照 |
| `Logging/Sources/CoveLogging/Runtime/Handler.swift` | 25 | 已写入（未验证） | `Logging/rust/handler.rs` | 未编译、未行为对照 |
| `Logging/Sources/CoveLogging/Runtime/History.swift` | 27 | 已写入（未验证） | `Logging/rust/history.rs` | 未编译、未行为对照 |
| `Logging/Sources/CoveLogging/Runtime/Observation.swift` | 9 | 已写入（未验证） | `Logging/rust/observation.rs` | 未编译、未行为对照 |
| `Logging/Sources/CoveLogging/Runtime/Runtime.swift` | 180 | 已写入（未验证） | `Logging/rust/runtime.rs` | 未编译、未行为对照 |
| `Logging/Sources/CoveLogging/Runtime/Sanitizer.swift` | 46 | 已写入（未验证） | `Logging/rust/sanitizer.rs` | 未编译、未行为对照 |
| `Logging/Sources/CoveLogging/System.swift` | 65 | 已写入（未验证） | `Logging/rust/system.rs` | 未编译、未行为对照 |

### Text

共 14 个文件：已写入 14，进行中 0，未开始 0。

| 源文件 | 行 | 状态 | Rust/C 目标 | 依赖或阻塞 |
| --- | ---: | --- | --- | --- |
| `Text/Sources/CoveUIText/ColorGlyphAtlas.swift` | 61 | 已写入（未验证） | `Text/rust/color_glyph_atlas.rs` | 未编译、未行为对照 |
| `Text/Sources/CoveUIText/ColorGlyphSource.swift` | 26 | 已写入（未验证） | `Text/rust/color_glyph_source.rs` | 未编译、未行为对照 |
| `Text/Sources/CoveUIText/DrawList+Text.swift` | 42 | 已写入（未验证） | `Text/rust/draw_list_text.rs` | 未编译、未行为对照 |
| `Text/Sources/CoveUIText/Exports.swift` | 1 | 已写入（未验证） | `Text/rust/lib.rs` | 未编译、未行为对照 |
| `Text/Sources/CoveUIText/FontAtlas.swift` | 441 | 已写入（未验证） | `Text/rust/font_atlas.rs` | 未编译、未行为对照 |
| `Text/Sources/CoveUIText/FontCollection.swift` | 99 | 已写入（未验证） | `Text/rust/font_collection.rs` | 未编译、未行为对照 |
| `Text/Sources/CoveUIText/FontFace.swift` | 546 | 已写入（未验证） | `Text/rust/font_face.rs` | 未编译、未行为对照 |
| `Text/Sources/CoveUIText/FontProvider.swift` | 613 | 已写入（未验证） | `Text/rust/font_provider.rs` | 未编译、未行为对照 |
| `Text/Sources/CoveUIText/FreeTypeColorGlyphs.swift` | 140 | 已写入（未验证） | `Text/rust/free_type_color_glyphs.rs` | 未编译、未行为对照 |
| `Text/Sources/CoveUIText/NativeColorGlyphFont.swift` | 101 | 已写入（未验证） | `Text/rust/native_color_glyph_font.rs` | 未编译、未行为对照 |
| `Text/Sources/CoveUIText/TextCache.swift` | 77 | 已写入（未验证） | `Text/rust/text_cache.rs` | 未编译、未行为对照 |
| `Text/Sources/CoveUIText/TextEnvironment.swift` | 209 | 已写入（未验证） | `Text/rust/text_environment.rs` | 未编译、未行为对照 |
| `Text/Sources/CoveUIText/TextShaper.swift` | 54 | 已写入（未验证） | `Text/rust/text_shaper.rs` | 未编译、未行为对照 |
| `Text/Sources/CoveUIText/TextStyle.swift` | 166 | 已写入（未验证） | `Text/rust/text_style.rs` | 未编译、未行为对照 |

### Imaging

共 9 个文件：已写入 9，进行中 0，未开始 0。

| 源文件 | 行 | 状态 | Rust/C 目标 | 依赖或阻塞 |
| --- | ---: | --- | --- | --- |
| `Imaging/Sources/CoveUIImaging/Animation.swift` | 74 | 已写入（未验证） | `Imaging/rust/animation.rs` | 未编译、未行为对照 |
| `Imaging/Sources/CoveUIImaging/BundleImageResource.swift` | 47 | 已写入（未验证） | `Imaging/rust/bundle_image_resource.rs` | 未编译、未行为对照 |
| `Imaging/Sources/CoveUIImaging/Exports.swift` | 1 | 已写入（未验证） | `Imaging/rust/lib.rs` | 未编译、未行为对照 |
| `Imaging/Sources/CoveUIImaging/ImageAssetRegistry.swift` | 175 | 已写入（未验证） | `Imaging/rust/image_asset_registry.rs` | 未编译、未行为对照 |
| `Imaging/Sources/CoveUIImaging/ImageDecoder.swift` | 194 | 已写入（未验证） | `Imaging/rust/image_decoder.rs` | 未编译、未行为对照 |
| `Imaging/Sources/CoveUIImaging/Orientation.swift` | 84 | 已写入（未验证） | `Imaging/rust/orientation.rs` | 未编译、未行为对照 |
| `Imaging/Sources/CoveUIImaging/Progressive.swift` | 42 | 已写入（未验证） | `Imaging/rust/progressive.rs` | 未编译、未行为对照 |
| `Imaging/Sources/CoveUIImaging/Rasterization.swift` | 50 | 已写入（未验证） | `Imaging/rust/rasterization.rs` | 未编译、未行为对照 |
| `Imaging/Sources/CoveUIImaging/TextureImage.swift` | 29 | 已写入（未验证） | `Imaging/rust/texture_image.rs` | 未编译、未行为对照 |

### Graphics

共 107 个文件：已写入 58，进行中 0，未开始 49。

| 源文件 | 行 | 状态 | Rust/C 目标 | 依赖或阻塞 |
| --- | ---: | --- | --- | --- |
| `Graphics/Sources/CoveUIGraphics/Exports.swift` | 2 | 已写入（未验证） | `Graphics/rust/ui/mod.rs` | 未编译、未行为对照 |
| `Graphics/Sources/CoveUIGraphics/Images/TextureImage.swift` | 26 | 已写入（未验证） | `Graphics/rust/ui/texture_image.rs` | 未编译、未行为对照 |
| `Graphics/Sources/CoveUIGraphics/Images/UIAssetResidency.swift` | 33 | 已写入（未验证） | `Graphics/rust/ui/asset_residency.rs` | 未编译、未行为对照 |
| `Graphics/Sources/CoveUIGraphics/NativeUIViewport.swift` | 44 | 已写入（未验证） | `Graphics/rust/ui/viewport.rs` | 未编译、未行为对照 |
| `Graphics/Sources/CoveUIGraphics/Rendering/DrawGeometry.swift` | 99 | 已写入（未验证） | `Graphics/rust/ui/draw_geometry.rs` | 未编译、未行为对照 |
| `Graphics/Sources/CoveUIGraphics/Rendering/NativeDrawListRenderer.swift` | 473 | 已写入（未验证） | `Graphics/rust/ui/draw_list_renderer.rs` | 未编译、未行为对照 |
| `Graphics/Sources/CoveUIGraphics/Rendering/NativeUIBackdropPass.swift` | 531 | 已写入（未验证） | `Graphics/rust/ui/backdrop_pass.rs` | 未编译、未行为对照 |
| `Graphics/Sources/CoveUIGraphics/Rendering/NativeUIRasterWorkspace.swift` | 59 | 已写入（未验证） | `Graphics/rust/ui/raster_workspace.rs` | 未编译、未行为对照 |
| `Graphics/Sources/CoveUIGraphics/Rendering/NativeUIShaderResources.swift` | 75 | 已写入（未验证） | `Graphics/rust/ui/shader_resources.rs` | 未编译、未行为对照 |
| `Graphics/Sources/CoveUIGraphics/Rendering/NativeUITextureStore.swift` | 379 | 已写入（未验证） | `Graphics/rust/ui/texture_store.rs` | 未编译、未行为对照 |
| `Graphics/Sources/CoveUIGraphics/Rendering/Resources.swift` | 9 | 已写入（未验证） | `Graphics/rust/ui/resources.rs` | 未编译、未行为对照 |
| `Graphics/Sources/CoveUIGraphics/Text/AtlasUpload.swift` | 27 | 已写入（未验证） | `Graphics/rust/ui/atlas_upload.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/BackendFactory.swift` | 32 | 已写入（未验证） | `Graphics/rust/rhi/backend_factory.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/DX12/DX12Backend.swift` | 15 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/DX12/DX12Device.swift` | 94 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/DX12/DX12Formats.swift` | 32 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/DX12/Resources/DX12Compilation.swift` | 24 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/DX12/Resources/DX12Pipelines.swift` | 74 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/DX12/Resources/DX12Resources.swift` | 92 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/DX12/Submission/DX12Submission.swift` | 144 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/DX12/Submission/DX12Uploader.swift` | 34 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/Device/AsyncResources.swift` | 26 | 已写入（未验证） | `Graphics/rust/rhi/resource_preparation.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Device/Capabilities.swift` | 45 | 已写入（未验证） | `Graphics/rust/rhi/capabilities.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Device/Commands.swift` | 353 | 已写入（未验证） | `Graphics/rust/rhi/commands.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Device/Descriptors.swift` | 507 | 已写入（未验证） | `Graphics/rust/rhi/descriptors.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Device/Device.swift` | 937 | 已写入（未验证） | `Graphics/rust/rhi/device.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Device/Handles.swift` | 25 | 已写入（未验证） | `Graphics/rust/rhi/handles.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Device/PipelineCache.swift` | 295 | 已写入（未验证） | `Graphics/rust/rhi/pipeline_cache.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Device/Queue.swift` | 24 | 已写入（未验证） | `Graphics/rust/rhi/queue.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Device/ResourcePreparation.swift` | 120 | 已写入（未验证） | `Graphics/rust/rhi/resource_preparation.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Metal/Bindings/MetalBindingRegistry.swift` | 33 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/Metal/Bindings/MetalLibraryCache.swift` | 62 | 已写入（未验证） | `Graphics/rust/metal/library_cache.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Metal/Bindings/MetalRegistries.swift` | 129 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/Metal/Bindings/MetalRenderBindings.swift` | 46 | 已写入（未验证） | `Graphics/rust/metal/render_bindings.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Metal/Device/MetalCompilation.swift` | 12 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/Metal/Device/MetalDeviceResources.swift` | 275 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/Metal/Device/MetalDeviceTransfer.swift` | 299 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/Metal/Device/MetalUploadRing.swift` | 55 | 已写入（未验证） | `Graphics/rust/metal/upload_ring.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Metal/MetalAccelerationStructure.swift` | 97 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/Metal/MetalBackend.swift` | 16 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/Metal/MetalDevice.swift` | 208 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/Metal/MetalFormats.swift` | 232 | 已写入（未验证） | `Graphics/rust/metal/formats.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Metal/Pipelines/MetalMeshPipeline.swift` | 86 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/Metal/Pipelines/MetalSubmissionStatus.swift` | 20 | 已写入（未验证） | `Graphics/rust/metal/submission_status.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Metal/Pipelines/MetalSubmitEncoder.swift` | 610 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/NativeRHI.swift` | 61 | 已写入（未验证） | `Graphics/rust/rhi/mod.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/RHIBackend.swift` | 133 | 已写入（未验证） | `Graphics/rust/rhi/backend.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/RHIError.swift` | 39 | 已写入（未验证） | `Graphics/rust/rhi/error.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/RHITypes.swift` | 229 | 已写入（未验证） | `Graphics/rust/rhi/types.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Resources/AccelerationStructure.swift` | 40 | 已写入（未验证） | `Graphics/rust/rhi/resources.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Resources/BackgroundCompilation.swift` | 33 | 已写入（未验证） | `Graphics/rust/rhi/resources.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Resources/MeshPipeline.swift` | 23 | 已写入（未验证） | `Graphics/rust/rhi/descriptors.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Resources/PipelineInterface.swift` | 66 | 已写入（未验证） | `Graphics/rust/rhi/pipeline_interface.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Resources/PresentationMode.swift` | 16 | 已写入（未验证） | `Graphics/rust/rhi/presentation.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Resources/ShaderArtifact.swift` | 113 | 已写入（未验证） | `Graphics/rust/rhi/shader_artifact.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Resources/ShaderArtifactLoading.swift` | 47 | 已写入（未验证） | `Graphics/rust/rhi/shader_artifact_loading.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Resources/ShaderSpecialization.swift` | 48 | 已写入（未验证） | `Graphics/rust/rhi/specialization.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Resources/ShaderVisibility.swift` | 16 | 已写入（未验证） | `Graphics/rust/rhi/visibility.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Resources/SwapchainResource.swift` | 19 | 已写入（未验证） | `Graphics/rust/rhi/resource_ownership.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Resources/TextureAllocation.swift` | 15 | 已写入（未验证） | `Graphics/rust/rhi/resources.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Resources/TextureBufferUpload.swift` | 14 | 已写入（未验证） | `Graphics/rust/rhi/commands.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Resources/TextureResource.swift` | 17 | 已写入（未验证） | `Graphics/rust/rhi/resource_ownership.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Resources/TextureUploadRegion.swift` | 7 | 已写入（未验证） | `Graphics/rust/rhi/resources.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Submission/FrameRing.swift` | 128 | 已写入（未验证） | `Graphics/rust/rhi/submission.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Submission/GPUTimestampProfile.swift` | 75 | 已写入（未验证） | `Graphics/rust/rhi/submission.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Submission/Plan.swift` | 103 | 已写入（未验证） | `Graphics/rust/rhi/submission_planner.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Submission/SubmissionCPUProfile.swift` | 63 | 已写入（未验证） | `Graphics/rust/rhi/submission.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Submission/SubmissionPlanner.swift` | 786 | 已写入（未验证） | `Graphics/rust/rhi/submission_planner.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Submission/UploadRing.swift` | 109 | 已写入（未验证） | `Graphics/rust/rhi/submission.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Validation/AccessTracker.swift` | 207 | 已写入（未验证） | `Graphics/rust/rhi/access_tracker.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Validation/Caches.swift` | 249 | 已写入（未验证） | `Graphics/rust/rhi/caches.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Validation/PipelineValidation.swift` | 101 | 已写入（未验证） | `Graphics/rust/rhi/pipeline_validation.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Validation/ReflectedBindingLayout.swift` | 24 | 已写入（未验证） | `Graphics/rust/rhi/shader_artifact.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Validation/RenderAttachmentValidation.swift` | 82 | 已写入（未验证） | `Graphics/rust/rhi/validation.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Validation/ResourceValidation.swift` | 62 | 已写入（未验证） | `Graphics/rust/rhi/validation.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Validation/StateTracker.swift` | 108 | 已写入（未验证） | `Graphics/rust/rhi/state_tracker.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Vulkan/Backend/Bindings.swift` | 147 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/Vulkan/Backend/Encoder.swift` | 324 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/Vulkan/Backend/Pipelines.swift` | 179 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/Vulkan/Backend/Submit.swift` | 405 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/Vulkan/Backend/Timestamps.swift` | 149 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/Vulkan/Backend/VulkanBackendResources.swift` | 339 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/Vulkan/Backend/VulkanBackendTransfer.swift` | 258 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/Vulkan/Backend/VulkanCompilation.swift` | 12 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/Vulkan/Presentation/PresentMode.swift` | 11 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/Vulkan/Presentation/SurfaceState.swift` | 7 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/Vulkan/Presentation/VulkanPresentation.swift` | 24 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/Vulkan/Presentation/VulkanSwapchain.swift` | 319 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/Vulkan/Presentation/VulkanSynchronization.swift` | 107 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/Vulkan/Resources/VulkanAcceleration.swift` | 154 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/Vulkan/Resources/VulkanMemory.swift` | 206 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/Vulkan/Resources/VulkanMesh.swift` | 44 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/Vulkan/Resources/VulkanResources.swift` | 100 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/Vulkan/Setup/VulkanDeviceSetup.swift` | 373 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/Vulkan/Setup/VulkanInstanceSetup.swift` | 84 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/Vulkan/Submission/CommandPool.swift` | 32 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/Vulkan/Submission/VulkanCleanup.swift` | 32 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/Vulkan/Submission/VulkanScratch.swift` | 28 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/Vulkan/Submission/VulkanSubmissionStatus.swift` | 10 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/Vulkan/Submission/VulkanUploadRing.swift` | 32 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/Vulkan/VulkanAdvanced.swift` | 37 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/Vulkan/VulkanAuxiliary.swift` | 52 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/Vulkan/VulkanBackend.swift` | 175 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/Vulkan/VulkanFormats.swift` | 148 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/Vulkan/VulkanFunctions.swift` | 687 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/Vulkan/VulkanRendering.swift` | 149 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |
| `Graphics/Sources/NativeRHI/Vulkan/VulkanRuntime.swift` | 67 | 未开始 | — | 依赖 RHI 前端；原生 GPU 后端（Metal/Vulkan/DX12） |

### Shell

共 13 个文件：已写入 13，进行中 0，未开始 0。

| 源文件 | 行 | 状态 | Rust/C 目标 | 依赖或阻塞 |
| --- | ---: | --- | --- | --- |
| `Shell/Sources/PlatformShell/ApplicationEntry.swift` | 34 | 已写入（未验证） | `Shell/rust/application_entry.rs` | 未编译、未行为对照 |
| `Shell/Sources/PlatformShell/Cursor.swift` | 2 | 已写入（未验证） | `Shell/rust/cursor.rs` | 未编译、未行为对照 |
| `Shell/Sources/PlatformShell/Diagnostics/SDL.swift` | 18 | 已写入（未验证） | `Shell/rust/diagnostics/mod.rs` | 未编译、未行为对照 |
| `Shell/Sources/PlatformShell/DisplayRefresh/CoreVideo.swift` | 62 | 已写入（未验证） | `Shell/rust/display_refresh/mod.rs`<br>`Shell/rust/apple.m` | 未编译、未行为对照 |
| `Shell/Sources/PlatformShell/DisplayRefresh/MacDisplayLink.swift` | 29 | 已写入（未验证） | `Shell/rust/display_refresh/mod.rs`<br>`Shell/rust/apple.m` | 未编译、未行为对照 |
| `Shell/Sources/PlatformShell/DisplayRefresh/Signal.swift` | 50 | 已写入（未验证） | `Shell/rust/display_refresh/signal.rs` | 未编译、未行为对照 |
| `Shell/Sources/PlatformShell/DisplayRefresh/WindowDisplayLink.swift` | 45 | 已写入（未验证） | `Shell/rust/display_refresh/mod.rs`<br>`Shell/rust/apple.m` | 未编译、未行为对照 |
| `Shell/Sources/PlatformShell/Log.swift` | 5 | 已写入（未验证） | `Shell/rust/log.rs` | 未编译、未行为对照 |
| `Shell/Sources/PlatformShell/NativeMenuPopover.swift` | 100 | 已写入（未验证） | `Shell/rust/native_menu_popover.rs` | 未编译、未行为对照 |
| `Shell/Sources/PlatformShell/PlatformShell.swift` | 579 | 已写入（未验证） | `Shell/rust/platform_shell.rs`<br>`Shell/rust/sdl3_shell.rs`<br>`Shell/rust/apple.m` | 未编译、未行为对照 |
| `Shell/Sources/PlatformShell/SDL3Clipboard.swift` | 22 | 已写入（未验证） | `Shell/rust/sdl3_clipboard.rs` | 未编译、未行为对照 |
| `Shell/Sources/PlatformShell/SDL3Shell.swift` | 1510 | 已写入（未验证） | `Shell/rust/sdl3_shell.rs`<br>`Shell/rust/native.c`<br>`Shell/rust/apple.m` | 未编译、未行为对照 |
| `Shell/Sources/PlatformShell/Scroll/AppKitInput.swift` | 73 | 已写入（未验证） | `Shell/rust/scroll/mod.rs`<br>`Shell/rust/apple.m` | 未编译、未行为对照 |

### Runtime

共 11 个文件：已写入 11，进行中 0，未开始 0。

| 源文件 | 行 | 状态 | Rust/C 目标 | 依赖或阻塞 |
| --- | ---: | --- | --- | --- |
| `Runtime/Sources/CoveUIRuntime/Core/AppBridge.swift` | 34 | 已写入（未验证） | `Runtime/rust/app_bridge.rs` | 未编译、未行为对照 |
| `Runtime/Sources/CoveUIRuntime/Core/CoveUIRuntime.swift` | 17 | 已写入（未验证） | `Runtime/rust/lib.rs` | 未编译、未行为对照 |
| `Runtime/Sources/CoveUIRuntime/Core/FrameDeadline.swift` | 28 | 已写入（未验证） | `Runtime/rust/frame_deadline.rs` | 未编译、未行为对照 |
| `Runtime/Sources/CoveUIRuntime/Core/FrameDrive.swift` | 24 | 已写入（未验证） | `Runtime/rust/frame_drive.rs` | 未编译、未行为对照 |
| `Runtime/Sources/CoveUIRuntime/Core/Log.swift` | 9 | 已写入（未验证） | `Runtime/rust/log.rs` | 未编译、未行为对照 |
| `Runtime/Sources/CoveUIRuntime/Core/NativeAccessibility.swift` | 303 | 已写入（未验证） | `Runtime/rust/native_accessibility.rs`<br>`Runtime/rust/accessibility.m` | 未编译、未行为对照 |
| `Runtime/Sources/CoveUIRuntime/Core/PlatformHost.swift` | 17 | 已写入（未验证） | `Runtime/rust/platform_host.rs` | 未编译、未行为对照 |
| `Runtime/Sources/CoveUIRuntime/Core/SDL3PlatformHost.swift` | 993 | 已写入（未验证） | `Runtime/rust/sdl3_platform_host.rs` | 未编译、未行为对照 |
| `Runtime/Sources/CoveUIRuntime/Core/iOS/DisplayDriver.swift` | 22 | 已写入（未验证） | `Runtime/rust/ios/mod.rs`<br>`Runtime/rust/lifecycle.m` | 未编译、未行为对照 |
| `Runtime/Sources/CoveUIRuntime/Exports.swift` | 4 | 已写入（未验证） | `Runtime/rust/lib.rs` | 未编译、未行为对照 |
| `Runtime/Sources/CoveUIRuntime/Rendering/SurfaceFactory.swift` | 34 | 已写入（未验证） | `Runtime/rust/surface_factory.rs` | 未编译、未行为对照 |

### Compose

共 414 个文件：已写入 191，进行中 6，未开始 217。

| 源文件 | 行 | 状态 | Rust/C 目标 | 依赖或阻塞 |
| --- | ---: | --- | --- | --- |
| `Compose/Composition/Sources/CoveUIComposeCore/Adaptive/ContainerQuery.swift` | 108 | 已写入（未验证） | `Compose/Composition/rust/adaptive/container_query.rs` | 未编译、未行为对照 |
| `Compose/Composition/Sources/CoveUIComposeCore/Adaptive/Surface.swift` | 22 | 已写入（未验证） | `Compose/Composition/rust/adaptive/surface.rs` | 未编译、未行为对照 |
| `Compose/Composition/Sources/CoveUIComposeCore/Adaptive/ViewportTraits.swift` | 26 | 已写入（未验证） | `Compose/Composition/rust/adaptive/viewport_traits.rs` | 未编译、未行为对照 |
| `Compose/Composition/Sources/CoveUIComposeCore/Adaptive/WidthClass.swift` | 13 | 已写入（未验证） | `Compose/Composition/rust/adaptive/width_class.rs` | 未编译、未行为对照 |
| `Compose/Composition/Sources/CoveUIComposeCore/AnyView.swift` | 26 | 已写入（未验证） | `Compose/Composition/rust/any_view.rs` | 未编译、未行为对照 |
| `Compose/Composition/Sources/CoveUIComposeCore/AroundApplyingModifier.swift` | 17 | 已写入（未验证） | `Compose/Composition/rust/around_applying_modifier.rs` | 未编译、未行为对照 |
| `Compose/Composition/Sources/CoveUIComposeCore/Box.swift` | 253 | 已写入（未验证） | `Compose/Composition/rust/box_view.rs` | 未编译、未行为对照 |
| `Compose/Composition/Sources/CoveUIComposeCore/CompositionLocalProvider.swift` | 46 | 已写入（未验证） | `Compose/Composition/rust/composition_local_provider.rs` | 未编译、未行为对照 |
| `Compose/Composition/Sources/CoveUIComposeCore/Exports.swift` | 1 | 已写入（未验证） | `Compose/Composition/rust/lib.rs` | 未编译、未行为对照 |
| `Compose/Composition/Sources/CoveUIComposeCore/IdentifiedView.swift` | 42 | 已写入（未验证） | `Compose/Composition/rust/identified_view.rs` | 未编译、未行为对照 |
| `Compose/Composition/Sources/CoveUIComposeCore/Layout/Direction.swift` | 14 | 已写入（未验证） | `Compose/Composition/rust/layout/direction.rs` | 未编译、未行为对照 |
| `Compose/Composition/Sources/CoveUIComposeCore/Layout/Layout.swift` | 91 | 已写入（未验证） | `Compose/Composition/rust/layout/mod.rs` | 未编译、未行为对照 |
| `Compose/Composition/Sources/CoveUIComposeCore/Layout/LayoutValue.swift` | 30 | 已写入（未验证） | `Compose/Composition/rust/layout/value.rs` | 未编译、未行为对照 |
| `Compose/Composition/Sources/CoveUIComposeCore/LayoutDebugModifier.swift` | 48 | 已写入（未验证） | `Compose/Composition/rust/layout_debug_modifier.rs` | 未编译、未行为对照 |
| `Compose/Composition/Sources/CoveUIComposeCore/LayoutTree.swift` | 30 | 已写入（未验证） | `Compose/Composition/rust/layout_tree.rs` | 未编译、未行为对照 |
| `Compose/Composition/Sources/CoveUIComposeCore/MemoizedView.swift` | 23 | 已写入（未验证） | `Compose/Composition/rust/memoized_view.rs` | 未编译、未行为对照 |
| `Compose/Composition/Sources/CoveUIComposeCore/ModifiedContent+Materialise.swift` | 196 | 已写入（未验证） | `Compose/Composition/rust/modified_content_materialise.rs` | 未编译、未行为对照 |
| `Compose/Composition/Sources/CoveUIComposeCore/ModifierReplay.swift` | 34 | 已写入（未验证） | `Compose/Composition/rust/modifier_replay.rs` | 未编译、未行为对照 |
| `Compose/Composition/Sources/CoveUIComposeCore/PrimitiveView.swift` | 39 | 已写入（未验证） | `Compose/Composition/rust/primitive_view.rs` | 未编译、未行为对照 |
| `Compose/Composition/Sources/CoveUIComposeCore/PrimitiveViews.swift` | 36 | 已写入（未验证） | `Compose/Composition/rust/primitive_views.rs` | 未编译、未行为对照 |
| `Compose/Composition/Sources/CoveUIComposeCore/Scancode.swift` | 64 | 已写入（未验证） | `Compose/Composition/rust/scancode.rs` | 未编译、未行为对照 |
| `Compose/Composition/Sources/CoveUIComposeCore/SourceLocation.swift` | 31 | 已写入（未验证） | `Compose/Composition/rust/source_location.rs` | 未编译、未行为对照 |
| `Compose/Composition/Sources/CoveUIComposeCore/StructuralView.swift` | 46 | 已写入（未验证） | `Compose/Composition/rust/structural_view.rs` | 未编译、未行为对照 |
| `Compose/Composition/Sources/CoveUIComposeCore/TrackStateReads.swift` | 20 | 已写入（未验证） | `Compose/Composition/rust/track_state_reads.rs` | 未编译、未行为对照 |
| `Compose/Composition/Sources/CoveUIComposeCore/View.swift` | 25 | 已写入（未验证） | `Compose/Composition/rust/view.rs` | 未编译、未行为对照 |
| `Compose/Composition/Sources/CoveUIComposeCore/ViewBuilder.swift` | 75 | 已写入（未验证） | `Compose/Composition/rust/view_builder.rs` | 未编译、未行为对照 |
| `Compose/Composition/Sources/CoveUIComposeCore/ViewGraph.swift` | 1028 | 已写入（未验证） | `Compose/Composition/rust/view_graph.rs` | 未编译、未行为对照 |
| `Compose/Composition/Sources/CoveUIComposeCore/ViewInputComparison.swift` | 71 | 已写入（未验证） | `Compose/Composition/rust/view_input_comparison.rs` | 未编译、未行为对照 |
| `Compose/Composition/Sources/CoveUIComposeCore/ViewModifier.swift` | 71 | 已写入（未验证） | `Compose/Composition/rust/view_modifier.rs` | 未编译、未行为对照 |
| `Compose/Composition/Sources/CoveUIReadMacros/Plugin.swift` | 7 | 已写入（未验证） | `Compose/CompositionMacros/rust/lib.rs` | 未编译、未行为对照 |
| `Compose/Composition/Sources/CoveUIReadMacros/TrackStateReadsMacro.swift` | 210 | 已写入（未验证） | `Compose/CompositionMacros/rust/lib.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Animation/AnimatedVisibility.swift` | 238 | 已写入（未验证） | `Compose/rust/animation/content/animated_visibility.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Animation/AnimationValueModifier.swift` | 74 | 已写入（未验证） | `Compose/rust/modifiers/animation_value.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Animation/Content/NumericText.swift` | 78 | 已写入（未验证） | `Compose/rust/animation/content/numeric_text.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Animation/EdgeInsets+Interpolatable.swift` | 14 | 已写入（未验证） | `Compose/rust/modifiers/layout/mod.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Animation/Geometry/Matched.swift` | 146 | 未开始 | `Compose/rust/animation/geometry/matched.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Animation/Geometry/Transforms.swift` | 40 | 已写入（未验证） | `Compose/rust/animation/geometry/transforms.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Animation/Interaction/SpringDrag.swift` | 126 | 未开始 | `Compose/rust/animation/interaction/spring_drag.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Animation/Keyframes/Track.swift` | 86 | 已写入（未验证） | `Compose/rust/animation/keyframes/track.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Animation/Phases/Sequence.swift` | 107 | 已写入（未验证） | `Compose/rust/animation/phases/sequence.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Animation/Transition.swift` | 111 | 已写入（未验证） | `Compose/rust/animation/transition.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Animation/TransitionView.swift` | 33 | 已写入（未验证） | `Compose/rust/animation/transition_view.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Animation/WithAnimation.swift` | 52 | 已写入（未验证） | `Compose/rust/lib.rs`<br>`Core/rust/scene/animation/context.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/AppStorage.swift` | 318 | 已写入（未验证） | `Compose/rust/app_storage.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Core/Log.swift` | 4 | 已写入（未验证） | `Compose/rust/core/log.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/CoreExports.swift` | 3 | 已写入（未验证） | `Compose/rust/core_exports.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/CoveUICompose.swift` | 7 | 已写入（未验证） | `Compose/rust/lib.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/CoveUIComposeResourceBundle.swift` | 6 | 已写入（未验证） | `Compose/rust/resource_bundle.rs`<br>`PlatformCore/rust/resources.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Foundation/Color+Adjust.swift` | 34 | 已写入（未验证） | `Compose/rust/foundation/color_adjust.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/I18n/LocalizedStringKey.swift` | 35 | 已写入（未验证） | `Compose/rust/i18n/localized_string_key.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Modifiers/Interaction/AccessibilityModifier.swift` | 33 | 已写入（未验证） | `Compose/rust/modifiers/interaction/mod.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Modifiers/Interaction/CursorModifier.swift` | 29 | 已写入（未验证） | `Compose/rust/modifiers/interaction/mod.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Modifiers/Interaction/FocusRequestModifier.swift` | 16 | 已写入（未验证） | `Compose/rust/modifiers/interaction/mod.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Modifiers/Interaction/HitTestingModifier.swift` | 21 | 已写入（未验证） | `Compose/rust/modifiers/interaction/mod.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Modifiers/Interaction/HoverModifier.swift` | 28 | 已写入（未验证） | `Compose/rust/modifiers/interaction/mod.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Modifiers/Layout/AbsolutePositionModifier.swift` | 57 | 已写入（未验证） | `Compose/rust/modifiers/layout/mod.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Modifiers/Layout/FlexModifier.swift` | 35 | 已写入（未验证） | `Compose/rust/modifiers/layout/mod.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Modifiers/Layout/FrameCaptureModifier.swift` | 59 | 已写入（未验证） | `Compose/rust/modifiers/layout/frame_capture.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Modifiers/Layout/FrameModifier.swift` | 182 | 已写入（未验证） | `Compose/rust/modifiers/layout/mod.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Modifiers/Layout/LetterSpacingModifier.swift` | 21 | 已写入（未验证） | `Compose/rust/modifiers/style/style_modifiers.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Modifiers/Layout/PaddingModifier.swift` | 58 | 已写入（未验证） | `Compose/rust/modifiers/layout/mod.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Modifiers/Layout/PaddingSugar.swift` | 14 | 已写入（未验证） | `Compose/rust/modifiers/layout/mod.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Modifiers/Style/StyleModifiers.swift` | 247 | 已写入（未验证） | `Compose/rust/modifiers/style/style_modifiers.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Modifiers/Style/SurfaceFinishModifier.swift` | 82 | 已写入（未验证） | `Compose/rust/modifiers/style/surface.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Observed.swift` | 59 | 已写入（未验证） | `Compose/rust/observed.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Pet/PetAnchor.swift` | 60 | 已写入（未验证） | `Compose/rust/pet/pet_anchor.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Pet/PetScene.swift` | 43 | 已写入（未验证） | `Compose/rust/pet/pet_scene.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Pet/PetTypes.swift` | 181 | 已写入（未验证） | `Compose/rust/pet/pet_types.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Pet/PetView.swift` | 122 | 已写入（未验证） | `Compose/rust/pet/pet_view.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Pet/PetWorld.swift` | 333 | 已写入（未验证） | `Compose/rust/pet/pet_world.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Columns/Picker.swift` | 39 | 进行中（有缺项） | `Compose/rust/primitives/columns/picker.rs` | 对应实现仍有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Columns/Priority.swift` | 50 | 已写入（未验证） | `Compose/rust/primitives/columns/mod.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Adaptive/ControlExtent.swift` | 15 | 已写入（未验证） | `Compose/rust/primitives/controls/adaptive/control_extent.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Adaptive/LongPress.swift` | 41 | 已写入（未验证） | `Compose/rust/primitives/controls/adaptive/long_press.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/AppBar.swift` | 115 | 未开始 | `Compose/rust/primitives/controls/app_bar.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/AppBar/Back.swift` | 22 | 未开始 | `Compose/rust/primitives/controls/app_bar/back.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Button.swift` | 551 | 进行中（有缺项） | `Compose/rust/primitives/controls/button.rs` | 对应实现仍有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Button/Chrome.swift` | 280 | 已写入（未验证） | `Compose/rust/theme/styles/buttons/builtin_button_chrome.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Calendar/Calendar.swift` | 399 | 未开始 | `Compose/rust/primitives/controls/calendar/calendar.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Calendar/Navigation.swift` | 109 | 未开始 | `Compose/rust/primitives/controls/calendar/navigation.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Calendar/Period.swift` | 98 | 未开始 | `Compose/rust/primitives/controls/calendar/period.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/CompletionPopover.swift` | 252 | 未开始 | `Compose/rust/primitives/controls/completion_popover.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/DateValues.swift` | 69 | 已写入（未验证） | `Compose/rust/primitives/controls/date_values.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/FieldLabel.swift` | 47 | 已写入（未验证） | `Compose/rust/primitives/controls/field_label.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/AssetRefField.swift` | 188 | 未开始 | `Compose/rust/primitives/controls/fields/asset_ref_field.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/ColorField.swift` | 296 | 未开始 | `Compose/rust/primitives/controls/fields/color_field.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/Combobox/Combobox.swift` | 197 | 未开始 | `Compose/rust/primitives/controls/fields/combobox/combobox.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/Combobox/RemoteSearch.swift` | 129 | 未开始 | `Compose/rust/primitives/controls/fields/combobox/remote_search.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/Combobox/Results.swift` | 50 | 未开始 | `Compose/rust/primitives/controls/fields/combobox/results.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/DatePicker.swift` | 55 | 未开始 | `Compose/rust/primitives/controls/fields/date_picker.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/DateRange/RangePicker.swift` | 311 | 未开始 | `Compose/rust/primitives/controls/fields/date_range/range_picker.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/DateRange/Rules.swift` | 84 | 未开始 | `Compose/rust/primitives/controls/fields/date_range/rules.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/JSON/ExpandedEditor.swift` | 104 | 未开始 | `Compose/rust/primitives/controls/fields/json/expanded_editor.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/JsonField.swift` | 235 | 未开始 | `Compose/rust/primitives/controls/fields/json_field.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/Numeric/AxisLayout.swift` | 46 | 未开始 | `Compose/rust/primitives/controls/fields/numeric/axis_layout.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/Numeric/NumberField.swift` | 291 | 未开始 | `Compose/rust/primitives/controls/fields/numeric/number_field.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/Numeric/PrefixStyle.swift` | 37 | 未开始 | `Compose/rust/primitives/controls/fields/numeric/prefix_style.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/Numeric/Scrub.swift` | 110 | 未开始 | `Compose/rust/primitives/controls/fields/numeric/scrub.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/Numeric/TextFormat.swift` | 50 | 未开始 | `Compose/rust/primitives/controls/fields/numeric/text_format.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/Numeric/Vec3Field.swift` | 115 | 未开始 | `Compose/rust/primitives/controls/fields/numeric/vec3_field.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/RadioGroup.swift` | 104 | 未开始 | `Compose/rust/primitives/controls/fields/radio_group.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/RangeSlider.swift` | 498 | 未开始 | `Compose/rust/primitives/controls/fields/range_slider.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/SearchField.swift` | 154 | 未开始 | `Compose/rust/primitives/controls/fields/search_field.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/Select.swift` | 255 | 进行中（有缺项） | `Compose/rust/primitives/controls/fields/select.rs` | 对应实现仍有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/Slider/Geometry.swift` | 37 | 已写入（未验证） | `Compose/rust/primitives/controls/fields/slider/geometry.rs`<br>`Compose/rust/primitives/controls/fields/slider/mod.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/Slider/Host.swift` | 206 | 未开始 | `Compose/rust/primitives/controls/fields/slider/host.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/Slider/Paint.swift` | 78 | 未开始 | `Compose/rust/primitives/controls/fields/slider/paint.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/Slider/Slider.swift` | 76 | 未开始 | `Compose/rust/primitives/controls/fields/slider/slider.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/TimeField.swift` | 60 | 未开始 | `Compose/rust/primitives/controls/fields/time_field.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/TimeInput.swift` | 320 | 未开始 | `Compose/rust/primitives/controls/fields/time_input.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Form.swift` | 144 | 已写入（未验证） | `Compose/rust/primitives/controls/form.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Form/LabelPlacement.swift` | 15 | 已写入（未验证） | `Compose/rust/primitives/controls/form/label_placement.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Form/LabelScope.swift` | 19 | 已写入（未验证） | `Compose/rust/primitives/controls/form/label_scope.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/FormValidation.swift` | 75 | 已写入（未验证） | `Compose/rust/primitives/controls/form_validation.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/InlineLabelLayout.swift` | 46 | 已写入（未验证） | `Compose/rust/primitives/controls/inline_label_layout.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/KeyCap.swift` | 78 | 已写入（未验证） | `Compose/rust/primitives/controls/key_cap.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Link.swift` | 113 | 进行中（有缺项） | `Compose/rust/primitives/controls/link.rs` | 对应实现仍有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/MenuButton.swift` | 117 | 未开始 | `Compose/rust/primitives/controls/menu_button.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Rating/Rating.swift` | 74 | 未开始 | `Compose/rust/primitives/controls/rating/rating.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Rating/RatingGeometry.swift` | 75 | 未开始 | `Compose/rust/primitives/controls/rating/rating_geometry.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Rating/RatingHost.swift` | 114 | 未开始 | `Compose/rust/primitives/controls/rating/rating_host.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Rating/RatingPaint.swift` | 61 | 未开始 | `Compose/rust/primitives/controls/rating/rating_paint.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/SplitButton.swift` | 102 | 未开始 | `Compose/rust/primitives/controls/split_button.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Tag.swift` | 153 | 进行中（有缺项） | `Compose/rust/primitives/controls/tag.rs` | 对应实现仍有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/TimePicker.swift` | 132 | 未开始 | `Compose/rust/primitives/controls/time_picker.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Toggle.swift` | 500 | 未开始 | `Compose/rust/primitives/controls/toggle.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Toolbar.swift` | 163 | 未开始 | `Compose/rust/primitives/controls/toolbar.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Toolbar/Commands.swift` | 143 | 未开始 | `Compose/rust/primitives/controls/toolbar/commands.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Toolbar/OverflowLayout.swift` | 83 | 未开始 | `Compose/rust/primitives/controls/toolbar/overflow_layout.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Toolbar/RetainedCommand.swift` | 61 | 未开始 | `Compose/rust/primitives/controls/toolbar/retained_command.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/WheelPicker.swift` | 269 | 未开始 | `Compose/rust/primitives/controls/wheel_picker.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Avatar/Avatar.swift` | 165 | 已写入（未验证） | `Compose/rust/primitives/data/avatar/avatar.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Avatar/Group.swift` | 135 | 已写入（未验证） | `Compose/rust/primitives/data/avatar/group.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Avatar/MemberList.swift` | 89 | 已写入（未验证） | `Compose/rust/primitives/data/avatar/member_list.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Avatar/Status.swift` | 96 | 已写入（未验证） | `Compose/rust/primitives/data/avatar/status.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Chart/Axes.swift` | 68 | 已写入（未验证） | `Compose/rust/primitives/data/chart/axes.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Chart/Axis.swift` | 218 | 已写入（未验证） | `Compose/rust/primitives/data/chart/axis.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Chart/Cache.swift` | 47 | 已写入（未验证） | `Compose/rust/primitives/data/chart/cache.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Chart/Chart.swift` | 262 | 未开始 | `Compose/rust/primitives/data/chart/chart.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Chart/ChartDataTable.swift` | 205 | 未开始 | `Compose/rust/primitives/data/chart/chart_data_table.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Chart/ChartOverlay.swift` | 218 | 已写入（未验证） | `Compose/rust/primitives/data/chart/overlay.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Chart/Coordinates.swift` | 333 | 已写入（未验证） | `Compose/rust/primitives/data/chart/coordinates.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Chart/Drawing.swift` | 499 | 已写入（未验证） | `Compose/rust/primitives/data/chart/drawing.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Chart/Histogram.swift` | 80 | 已写入（未验证） | `Compose/rust/primitives/data/chart/histogram.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Chart/Interaction.swift` | 175 | 未开始 | `Compose/rust/primitives/data/chart/interaction.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Chart/Legend.swift` | 102 | 已写入（未验证） | `Compose/rust/primitives/data/chart/legend.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Chart/Model.swift` | 474 | 已写入（未验证） | `Compose/rust/primitives/data/chart/model.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Chart/Morph.swift` | 77 | 已写入（未验证） | `Compose/rust/primitives/data/chart/morph.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Chart/Plot.swift` | 708 | 已写入（未验证） | `Compose/rust/primitives/data/chart/plot.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Chart/Scale.swift` | 203 | 已写入（未验证） | `Compose/rust/primitives/data/chart/scale.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Chart/Stacking.swift` | 112 | 已写入（未验证） | `Compose/rust/primitives/data/chart/stacking.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Chart/Viewport.swift` | 92 | 已写入（未验证） | `Compose/rust/primitives/data/chart/viewport.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Columns/Focus.swift` | 44 | 未开始 | `Compose/rust/primitives/data/columns/focus.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Columns/TablePresentation.swift` | 34 | 未开始 | `Compose/rust/primitives/data/columns/table_presentation.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Columns/Visibility.swift` | 25 | 未开始 | `Compose/rust/primitives/data/columns/visibility.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Data/DataTable/ColumnOrder.swift` | 141 | 未开始 | `Compose/rust/primitives/data/data_table/column_order.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Data/DataTable/DataTable.swift` | 604 | 未开始 | `Compose/rust/primitives/data/data_table/data_table.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Data/DataTable/DataTableEditing.swift` | 66 | 未开始 | `Compose/rust/primitives/data/data_table/data_table_editing.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Data/DataTable/DataTableGeometry.swift` | 32 | 已写入（未验证） | `Compose/rust/primitives/data/data_table/data_table_geometry.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/DataTable/DataTableHosts.swift` | 119 | 未开始 | `Compose/rust/primitives/data/data_table/data_table_hosts.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Data/DataTable/DataTableModel.swift` | 112 | 未开始 | `Compose/rust/primitives/data/data_table/data_table_model.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Data/DataTable/DataTableSelection.swift` | 29 | 未开始 | `Compose/rust/primitives/data/data_table/data_table_selection.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Data/DataTable/DataTableSortWorker.swift` | 45 | 未开始 | `Compose/rust/primitives/data/data_table/data_table_sort_worker.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Data/DataTable/EditorPresence.swift` | 22 | 未开始 | `Compose/rust/primitives/data/data_table/editor_presence.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Data/DataTable/VariableRows.swift` | 60 | 未开始 | `Compose/rust/primitives/data/data_table/variable_rows.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Lists/InsetGroup.swift` | 79 | 未开始 | `Compose/rust/primitives/data/lists/inset_group.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Lists/List.swift` | 285 | 未开始 | `Compose/rust/primitives/data/lists/list.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Lists/Refresh.swift` | 79 | 未开始 | `Compose/rust/primitives/data/lists/refresh.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Lists/Reorder/Handle.swift` | 151 | 未开始 | `Compose/rust/primitives/data/lists/reorder/handle.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Lists/Reorder/Preview.swift` | 62 | 未开始 | `Compose/rust/primitives/data/lists/reorder/preview.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Lists/Reorder/Reorderable.swift` | 268 | 未开始 | `Compose/rust/primitives/data/lists/reorder/reorderable.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Lists/Row.swift` | 152 | 未开始 | `Compose/rust/primitives/data/lists/row.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Lists/SectionedList.swift` | 272 | 未开始 | `Compose/rust/primitives/data/lists/sectioned_list.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Lists/Selection.swift` | 61 | 未开始 | `Compose/rust/primitives/data/lists/selection.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Lists/SwipeList.swift` | 409 | 未开始 | `Compose/rust/primitives/data/lists/swipe_list.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Lists/Table.swift` | 309 | 未开始 | `Compose/rust/primitives/data/lists/table.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Lists/Tree.swift` | 1372 | 未开始 | `Compose/rust/primitives/data/lists/tree.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Lists/Tree/Guides.swift` | 135 | 未开始 | `Compose/rust/primitives/data/lists/tree/guides.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Lists/TreeOptions.swift` | 86 | 未开始 | `Compose/rust/primitives/data/lists/tree_options.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Data/PropertyGrid.swift` | 454 | 未开始 | `Compose/rust/primitives/data/property_grid.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Timeline.swift` | 266 | 未开始 | `Compose/rust/primitives/data/timeline.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Timeline/HeadingLayout.swift` | 52 | 未开始 | `Compose/rust/primitives/data/timeline/heading_layout.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Data/TreeTable/TreeTable.swift` | 201 | 未开始 | `Compose/rust/primitives/data/tree_table/tree_table.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Feedback/Alert.swift` | 157 | 未开始 | `Compose/rust/primitives/feedback/alert.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Feedback/Badge.swift` | 142 | 未开始 | `Compose/rust/primitives/feedback/badge.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Feedback/EmptyState.swift` | 117 | 未开始 | `Compose/rust/primitives/feedback/empty_state.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Feedback/Progress.swift` | 129 | 未开始 | `Compose/rust/primitives/feedback/progress.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Feedback/ProgressCircle.swift` | 170 | 未开始 | `Compose/rust/primitives/feedback/progress_circle.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Feedback/ProgressValue.swift` | 35 | 已写入（未验证） | `Compose/rust/primitives/feedback/progress_value.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Feedback/Spinner.swift` | 60 | 已写入（未验证） | `Compose/rust/primitives/feedback/spinner.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Feedback/StatusTone.swift` | 46 | 已写入（未验证） | `Compose/rust/primitives/feedback/status_tone.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Interaction/DragAndDrop.swift` | 285 | 未开始 | `Compose/rust/primitives/interaction/drag_and_drop.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Interaction/EdgeAutoScroll.swift` | 72 | 未开始 | `Compose/rust/primitives/interaction/edge_auto_scroll.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Interaction/HorizontalSwipe.swift` | 120 | 未开始 | `Compose/rust/primitives/interaction/horizontal_swipe.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Interaction/Scrollbar.swift` | 303 | 未开始 | `Compose/rust/primitives/interaction/scrollbar.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Accordion.swift` | 74 | 未开始 | `Compose/rust/primitives/layout/accordion.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/BoundedScrollView.swift` | 50 | 未开始 | `Compose/rust/primitives/layout/bounded_scroll_view.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Carousel/Autoplay.swift` | 132 | 未开始 | `Compose/rust/primitives/layout/carousel/autoplay.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Carousel/Carousel.swift` | 215 | 未开始 | `Compose/rust/primitives/layout/carousel/carousel.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Carousel/Indicator.swift` | 44 | 未开始 | `Compose/rust/primitives/layout/carousel/indicator.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Direction/Resolved.swift` | 35 | 已写入（未验证） | `Compose/rust/primitives/layout/direction/resolved.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/DisclosureGroup.swift` | 62 | 未开始 | `Compose/rust/primitives/layout/disclosure_group.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Grid/Columns.swift` | 157 | 已写入（未验证） | `Compose/rust/primitives/layout/grid/columns.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Grid/Grid.swift` | 267 | 已写入（未验证） | `Compose/rust/primitives/layout/grid/grid.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Grid/InteractionGate.swift` | 17 | 已写入（未验证） | `Compose/rust/primitives/layout/grid/grid.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Grid/LazyGrid.swift` | 211 | 已写入（未验证） | `Compose/rust/primitives/layout/grid/lazy_grid.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Grid/NamedColumns.swift` | 27 | 已写入（未验证） | `Compose/rust/primitives/layout/grid/named_columns.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Grid/RowHeights.swift` | 109 | 已写入（未验证） | `Compose/rust/primitives/layout/grid/row_heights.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Grid/Span.swift` | 82 | 已写入（未验证） | `Compose/rust/primitives/layout/grid/span.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/HorizontalScrollOrigin.swift` | 16 | 已写入（未验证） | `Compose/rust/primitives/layout/horizontal_scroll_origin.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Insets/SafeArea.swift` | 65 | 未开始 | `Compose/rust/primitives/layout/insets/safe_area.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Insets/SurfaceRoot.swift` | 53 | 未开始 | `Compose/rust/primitives/layout/insets/surface_root.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Masonry/Masonry.swift` | 167 | 已写入（未验证） | `Compose/rust/primitives/layout/masonry/masonry.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/NavigationComponents.swift` | 114 | 未开始 | `Compose/rust/primitives/layout/navigation_components.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Overlay.swift` | 97 | 已写入（未验证） | `Compose/rust/primitives/layout/overlay.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Panel.swift` | 70 | 未开始 | `Compose/rust/primitives/layout/panel.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/ResizableEditor.swift` | 36 | 未开始 | `Compose/rust/primitives/layout/resizable_editor.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/ResizableTextArea.swift` | 56 | 未开始 | `Compose/rust/primitives/layout/resizable_text_area.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Resize/HeightHandle.swift` | 131 | 未开始 | `Compose/rust/primitives/layout/resize/height_handle.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Resize/HeightLimits.swift` | 13 | 已写入（未验证） | `Compose/rust/primitives/layout/resize/height_limits.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Responsive/ActionGroup.swift` | 57 | 已写入（未验证） | `Compose/rust/primitives/layout/responsive/action_group.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Responsive/Candidates.swift` | 41 | 已写入（未验证） | `Compose/rust/primitives/layout/responsive/candidates.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Responsive/Stack.swift` | 118 | 未开始 | `Compose/rust/primitives/layout/responsive/stack.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Responsive/ViewThatFits.swift` | 118 | 未开始 | `Compose/rust/primitives/layout/responsive/view_that_fits.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/RowSlot.swift` | 22 | 已写入（未验证） | `Compose/rust/primitives/layout/row_slot.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/ScrollReveal.swift` | 19 | 已写入（未验证） | `Compose/rust/primitives/layout/scroll_reveal.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/ScrollView.swift` | 1139 | 未开始 | `Compose/rust/primitives/layout/scroll_view.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Scrolling/KeyboardFocus.swift` | 49 | 未开始 | `Compose/rust/primitives/layout/scrolling/keyboard_focus.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Sidebar/AdaptiveHosts.swift` | 103 | 未开始 | `Compose/rust/primitives/layout/sidebar/adaptive_hosts.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Sidebar/AdaptiveOptions.swift` | 69 | 未开始 | `Compose/rust/primitives/layout/sidebar/adaptive_options.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Sidebar/AdaptiveSidebar.swift` | 199 | 未开始 | `Compose/rust/primitives/layout/sidebar/adaptive_sidebar.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Sidebar/Destination.swift` | 234 | 未开始 | `Compose/rust/primitives/layout/sidebar/destination.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Sidebar/Drawer.swift` | 92 | 未开始 | `Compose/rust/primitives/layout/sidebar/drawer.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Sidebar/FocusHandoff.swift` | 110 | 未开始 | `Compose/rust/primitives/layout/sidebar/focus_handoff.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Sidebar/Hierarchy.swift` | 64 | 未开始 | `Compose/rust/primitives/layout/sidebar/hierarchy.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Sidebar/HierarchyHost.swift` | 85 | 未开始 | `Compose/rust/primitives/layout/sidebar/hierarchy_host.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Sidebar/Items.swift` | 48 | 未开始 | `Compose/rust/primitives/layout/sidebar/items.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Sidebar/Presentation.swift` | 19 | 未开始 | `Compose/rust/primitives/layout/sidebar/presentation.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Sidebar/PresentationToggle.swift` | 12 | 未开始 | `Compose/rust/primitives/layout/sidebar/presentation_toggle.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Sidebar/Rail.swift` | 51 | 未开始 | `Compose/rust/primitives/layout/sidebar/rail.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Sidebar/RailDestination.swift` | 89 | 未开始 | `Compose/rust/primitives/layout/sidebar/rail_destination.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Sidebar/ResizeHandle.swift` | 136 | 未开始 | `Compose/rust/primitives/layout/sidebar/resize_handle.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Sidebar/Resizing.swift` | 20 | 未开始 | `Compose/rust/primitives/layout/sidebar/resizing.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Sidebar/Sidebar.swift` | 126 | 未开始 | `Compose/rust/primitives/layout/sidebar/sidebar.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Split/Compact.swift` | 67 | 未开始 | `Compose/rust/primitives/layout/split/compact.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Split/Constraints.swift` | 44 | 未开始 | `Compose/rust/primitives/layout/split/constraints.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Split/Retention.swift` | 109 | 未开始 | `Compose/rust/primitives/layout/split/retention.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/SplitView.swift` | 360 | 未开始 | `Compose/rust/primitives/layout/split_view.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/StatusBar.swift` | 184 | 未开始 | `Compose/rust/primitives/layout/status_bar.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/TabView.swift` | 299 | 未开始 | `Compose/rust/primitives/layout/tab_view.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Tabs/MenuStrip.swift` | 61 | 未开始 | `Compose/rust/primitives/layout/tabs/menu_strip.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Tabs/MobileBar.swift` | 94 | 未开始 | `Compose/rust/primitives/layout/tabs/mobile_bar.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Tabs/StripLayout.swift` | 64 | 未开始 | `Compose/rust/primitives/layout/tabs/strip_layout.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Virtualization/EstimatedStack.swift` | 169 | 未开始 | `Compose/rust/primitives/layout/virtualization/estimated_stack.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Virtualization/FixedStack.swift` | 137 | 未开始 | `Compose/rust/primitives/layout/virtualization/fixed_stack.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Virtualization/RowIndex.swift` | 147 | 未开始 | `Compose/rust/primitives/layout/virtualization/row_index.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Virtualization/RowMetrics.swift` | 47 | 未开始 | `Compose/rust/primitives/layout/virtualization/row_metrics.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Virtualization/VariableStack.swift` | 108 | 未开始 | `Compose/rust/primitives/layout/virtualization/variable_stack.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Virtualization/VirtualStack.swift` | 77 | 未开始 | `Compose/rust/primitives/layout/virtualization/virtual_stack.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Virtualization/VirtualWindow.swift` | 41 | 已写入（未验证） | `Compose/rust/primitives/layout/virtualization/virtual_window.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/ZStack.swift` | 89 | 已写入（未验证） | `Compose/rust/primitives/layout/zstack.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Media/AsyncImage.swift` | 335 | 未开始 | `Compose/rust/primitives/media/async_image.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Media/Icon.swift` | 87 | 未开始 | `Compose/rust/primitives/media/icon.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Media/Image+File.swift` | 144 | 已写入（未验证） | `Compose/rust/primitives/media/image_file.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Media/Image.swift` | 173 | 已写入（未验证） | `Compose/rust/primitives/media/image.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Media/ImageDataDownload.swift` | 130 | 未开始 | `Compose/rust/primitives/media/image_data_download.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Media/ImageGeometry.swift` | 33 | 已写入（未验证） | `Compose/rust/primitives/media/image_geometry.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Media/ImageLoadServices.swift` | 146 | 未开始 | `Compose/rust/primitives/media/image_load_services.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Media/LoadingAnimationClock.swift` | 33 | 已写入（未验证） | `Compose/rust/primitives/media/loading_animation_clock.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Media/Skeleton.swift` | 88 | 未开始 | `Compose/rust/primitives/media/skeleton.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Media/UIIconCatalog.swift` | 735 | 已写入（未验证） | `Compose/rust/primitives/media/ui_icon_catalog.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Media/VectorImageRaster.swift` | 44 | 已写入（未验证） | `Compose/rust/primitives/media/vector_image_raster.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Navigation/NavigationStack.swift` | 311 | 未开始 | `Compose/rust/primitives/navigation/navigation_stack.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Navigation/RouteOutlet.swift` | 19 | 已写入（未验证） | `Compose/rust/primitives/navigation/route_outlet.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Navigation/Stepper/Stepper.swift` | 155 | 未开始 | `Compose/rust/primitives/navigation/stepper/stepper.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Navigation/Stepper/Values.swift` | 50 | 未开始 | `Compose/rust/primitives/navigation/stepper/values.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/ActionSheet.swift` | 125 | 未开始 | `Compose/rust/primitives/overlays/action_sheet.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/ActionSheet/Content.swift` | 106 | 未开始 | `Compose/rust/primitives/overlays/action_sheet/content.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Anchored/Anchoring.swift` | 126 | 未开始 | `Compose/rust/primitives/overlays/anchored/anchoring.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Anchored/Placement.swift` | 94 | 未开始 | `Compose/rust/primitives/overlays/anchored/placement.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Anchored/Surface.swift` | 60 | 未开始 | `Compose/rust/primitives/overlays/anchored/surface.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/AssetDropTarget.swift` | 212 | 未开始 | `Compose/rust/primitives/overlays/asset_drop_target.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Command/Search.swift` | 89 | 未开始 | `Compose/rust/primitives/overlays/command/search.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/CommandPalette.swift` | 436 | 未开始 | `Compose/rust/primitives/overlays/command_palette.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/FocusScope.swift` | 22 | 已写入（未验证） | `Compose/rust/primitives/overlays/focus_scope.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/HoverCard.swift` | 183 | 未开始 | `Compose/rust/primitives/overlays/hover_card.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Menu/ContextMenu.swift` | 155 | 未开始 | `Compose/rust/primitives/overlays/menu/context_menu.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Menu/Menu.swift` | 300 | 未开始 | `Compose/rust/primitives/overlays/menu/menu.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Menu/MenuDescriptor.swift` | 19 | 已写入（未验证） | `Compose/rust/primitives/overlays/menu/menu_descriptor.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Menu/MenuItemRow.swift` | 264 | 未开始 | `Compose/rust/primitives/overlays/menu/menu_item_row.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Menu/MenuNavigation.swift` | 188 | 未开始 | `Compose/rust/primitives/overlays/menu/menu_navigation.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Menu/NativeMenuBridge.swift` | 128 | 未开始 | `Compose/rust/primitives/overlays/menu/native_menu_bridge.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Menu/RowContent.swift` | 60 | 未开始 | `Compose/rust/primitives/overlays/menu/row_content.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Menu/TouchMenu.swift` | 106 | 未开始 | `Compose/rust/primitives/overlays/menu/touch_menu.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Modal/BottomSheet.swift` | 104 | 未开始 | `Compose/rust/primitives/overlays/modal/bottom_sheet.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Modal/Dialog.swift` | 146 | 未开始 | `Compose/rust/primitives/overlays/modal/dialog.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Modal/DragHandle.swift` | 104 | 未开始 | `Compose/rust/primitives/overlays/modal/drag_handle.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Modal/Modal.swift` | 115 | 未开始 | `Compose/rust/primitives/overlays/modal/modal.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Modal/ModalOptions.swift` | 48 | 未开始 | `Compose/rust/primitives/overlays/modal/modal_options.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Modal/TouchPresentation.swift` | 76 | 未开始 | `Compose/rust/primitives/overlays/modal/touch_presentation.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/OverlayHost.swift` | 10 | 未开始 | `Compose/rust/primitives/overlays/overlay_host.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Popconfirm/Popconfirm.swift` | 71 | 未开始 | `Compose/rust/primitives/overlays/popconfirm/popconfirm.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Popover.swift` | 108 | 未开始 | `Compose/rust/primitives/overlays/popover.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Portal/FitConstraints.swift` | 19 | 已写入（未验证） | `Compose/rust/primitives/overlays/portal/fit_constraints.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Portal/LayerRoot.swift` | 95 | 未开始 | `Compose/rust/primitives/overlays/portal/layer_root.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Portal/OverlayPlacement.swift` | 24 | 已写入（未验证） | `Compose/rust/primitives/overlays/portal/overlay_placement.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Portal/PortalHost.swift` | 218 | 未开始 | `Compose/rust/primitives/overlays/portal/portal_host.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Portal/PortalPlacement.swift` | 41 | 已写入（未验证） | `Compose/rust/primitives/overlays/portal/portal_placement.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Portal/PortalRegistry.swift` | 386 | 已写入（未验证） | `Compose/rust/primitives/overlays/portal/portal_registry.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/RovingFocus.swift` | 128 | 未开始 | `Compose/rust/primitives/overlays/roving_focus.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/SelectionKeyHost.swift` | 44 | 已写入（未验证） | `Compose/rust/primitives/overlays/selection_key_host.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/ShortcutHost.swift` | 58 | 未开始 | `Compose/rust/primitives/overlays/shortcut_host.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Toast/Controller.swift` | 90 | 未开始 | `Compose/rust/primitives/overlays/toast/controller.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Toast/ExitRetention.swift` | 45 | 未开始 | `Compose/rust/primitives/overlays/toast/exit_retention.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Toast/NotificationHost.swift` | 172 | 未开始 | `Compose/rust/primitives/overlays/toast/notification_host.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Tooltip.swift` | 412 | 未开始 | `Compose/rust/primitives/overlays/tooltip.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Code/FoldingOptions.swift` | 121 | 未开始 | `Compose/rust/primitives/text/code/folding_options.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Code/GutterControls.swift` | 181 | 未开始 | `Compose/rust/primitives/text/code/gutter_controls.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Code/GutterGeometry.swift` | 46 | 未开始 | `Compose/rust/primitives/text/code/gutter_geometry.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Code/GutterLayout.swift` | 74 | 未开始 | `Compose/rust/primitives/text/code/gutter_layout.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Code/GutterOptions.swift` | 69 | 未开始 | `Compose/rust/primitives/text/code/gutter_options.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Code/GutterSession.swift` | 121 | 未开始 | `Compose/rust/primitives/text/code/gutter_session.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Code/Minimap.swift` | 112 | 未开始 | `Compose/rust/primitives/text/code/minimap.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Code/Occurrences.swift` | 27 | 未开始 | `Compose/rust/primitives/text/code/occurrences.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Markdown/CodeBlock.swift` | 100 | 未开始 | `Compose/rust/primitives/text/markdown/code_block.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Markdown/Document.swift` | 30 | 未开始 | `Compose/rust/primitives/text/markdown/document.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Markdown/ListBlock.swift` | 98 | 未开始 | `Compose/rust/primitives/text/markdown/list_block.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Markdown/MarkdownView.swift` | 135 | 未开始 | `Compose/rust/primitives/text/markdown/markdown_view.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Markdown/Parse.swift` | 139 | 未开始 | `Compose/rust/primitives/text/markdown/parse.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Markdown/TableBlock.swift` | 107 | 未开始 | `Compose/rust/primitives/text/markdown/table_block.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Rich/Flow.swift` | 413 | 已写入（未验证） | `Compose/rust/primitives/text/rich/flow.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Rich/LinkAnchor.swift` | 56 | 未开始 | `Compose/rust/primitives/text/rich/link_anchor.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Rich/Paragraph.swift` | 357 | 未开始 | `Compose/rust/primitives/text/rich/paragraph.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Rich/RichText.swift` | 199 | 未开始 | `Compose/rust/primitives/text/rich/rich_text.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Rich/Run.swift` | 27 | 已写入（未验证） | `Compose/rust/primitives/text/rich/run.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Rich/TextSelection.swift` | 620 | 已写入（未验证） | `Compose/rust/primitives/text/rich/text_selection.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Rope/RopeDifference.swift` | 45 | 已写入（未验证） | `Compose/rust/primitives/text/rope/rope_difference.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Rope/RopeEditing.swift` | 70 | 已写入（未验证） | `Compose/rust/primitives/text/rope/rope_editing.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Rope/RopeLookup.swift` | 126 | 已写入（未验证） | `Compose/rust/primitives/text/rope/rope_lookup.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Rope/RopeStorage.swift` | 93 | 已写入（未验证） | `Compose/rust/primitives/text/rope/rope_storage.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/SingleLine.swift` | 19 | 已写入（未验证） | `Compose/rust/primitives/text/single_line.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Text.swift` | 605 | 进行中（有缺项） | `Compose/rust/primitives/text/text.rs` | 对应实现仍有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Text/TextBuffer.swift` | 188 | 已写入（未验证） | `Compose/rust/primitives/text/text_buffer.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/TextCompletionRebase.swift` | 47 | 已写入（未验证） | `Compose/rust/primitives/text/text_completion_rebase.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/TextDiagnostic.swift` | 118 | 已写入（未验证） | `Compose/rust/primitives/text/text_diagnostic.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/TextDocumentLayout.swift` | 263 | 已写入（未验证） | `Compose/rust/primitives/text/text_document_layout.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/TextEditHistory.swift` | 97 | 已写入（未验证） | `Compose/rust/primitives/text/text_edit_history.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/TextEditingCommands.swift` | 18 | 已写入（未验证） | `Compose/rust/primitives/text/text_editing_commands.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/TextField/BracketMatching.swift` | 129 | 未开始 | `Compose/rust/primitives/text/text_field/bracket_matching.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Text/TextField/IndentGuides.swift` | 84 | 未开始 | `Compose/rust/primitives/text/text_field/indent_guides.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Text/TextField/PlatformInput.swift` | 75 | 未开始 | `Compose/rust/primitives/text/text_field/platform_input.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Text/TextField/Snippet.swift` | 177 | 未开始 | `Compose/rust/primitives/text/text_field/snippet.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Text/TextField/TextField.swift` | 1483 | 未开始 | `Compose/rust/primitives/text/text_field/text_field.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Text/TextField/TextFieldCodeEditing.swift` | 54 | 未开始 | `Compose/rust/primitives/text/text_field/text_field_code_editing.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Text/TextField/TextFieldEditing.swift` | 218 | 未开始 | `Compose/rust/primitives/text/text_field/text_field_editing.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Text/TextField/TextFieldHorizontalScroll.swift` | 35 | 未开始 | `Compose/rust/primitives/text/text_field/text_field_horizontal_scroll.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Text/TextField/TextFieldHost.swift` | 162 | 未开始 | `Compose/rust/primitives/text/text_field/text_field_host.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Text/TextField/TextFieldInputController.swift` | 155 | 未开始 | `Compose/rust/primitives/text/text_field/text_field_input_controller.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Text/TextField/TextFieldLayoutEngine.swift` | 262 | 未开始 | `Compose/rust/primitives/text/text_field/text_field_layout_engine.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Text/TextField/TextFieldNavigation.swift` | 60 | 未开始 | `Compose/rust/primitives/text/text_field/text_field_navigation.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Text/TextField/TextFieldObservation.swift` | 62 | 未开始 | `Compose/rust/primitives/text/text_field/text_field_observation.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Text/TextField/TextFieldOptions.swift` | 122 | 未开始 | `Compose/rust/primitives/text/text_field/text_field_options.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Primitives/Text/TextField/TextVisualLineIndex.swift` | 106 | 已写入（未验证） | `Compose/rust/primitives/text/text_field/text_visual_line_index.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Rendering/OffscreenView.swift` | 154 | 已写入（未验证） | `Compose/rust/rendering/offscreen_view.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Scrolling/ScrollPhysicsEnvironment.swift` | 30 | 已写入（未验证） | `Compose/rust/scrolling/scroll_physics_environment.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Styles/AppearanceModifier.swift` | 25 | 已写入（未验证） | `Compose/rust/theme/styles/appearance_modifier.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Styles/Buttons/ButtonStyle.swift` | 186 | 已写入（未验证） | `Compose/rust/theme/styles/buttons/button_style.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Styles/Buttons/DestructiveButtonStyle.swift` | 18 | 已写入（未验证） | `Compose/rust/theme/styles/buttons/destructive_button_style.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Styles/Buttons/GhostButtonStyle.swift` | 17 | 已写入（未验证） | `Compose/rust/theme/styles/buttons/ghost_button_style.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Styles/Buttons/PlainButtonStyle.swift` | 18 | 已写入（未验证） | `Compose/rust/theme/styles/buttons/plain_button_style.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Styles/Buttons/PrimaryButtonStyle.swift` | 26 | 已写入（未验证） | `Compose/rust/theme/styles/buttons/primary_button_style.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Styles/Buttons/SecondaryButtonStyle.swift` | 17 | 已写入（未验证） | `Compose/rust/theme/styles/buttons/secondary_button_style.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Styles/Buttons/SegmentButtonStyle.swift` | 24 | 已写入（未验证） | `Compose/rust/theme/styles/buttons/segment_button_style.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Styles/Buttons/StatusButtonStyle.swift` | 34 | 已写入（未验证） | `Compose/rust/theme/styles/buttons/status_button_style.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Styles/Buttons/TabButtonStyle.swift` | 50 | 已写入（未验证） | `Compose/rust/theme/styles/buttons/tab_button_style.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Styles/Buttons/ToggleButtonStyle.swift` | 33 | 已写入（未验证） | `Compose/rust/theme/styles/buttons/toggle_button_style.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Styles/Buttons/ToolbarToggleButtonStyle.swift` | 32 | 已写入（未验证） | `Compose/rust/theme/styles/buttons/toolbar_toggle_button_style.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Styles/CircularAppearance.swift` | 89 | 已写入（未验证） | `Compose/rust/theme/styles/circular_appearance.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Styles/Components/DefaultDividerStyle.swift` | 19 | 已写入（未验证） | `Compose/rust/theme/styles/components/default_divider_style.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Styles/Components/DefaultListRowStyle.swift` | 78 | 未开始 | `Compose/rust/theme/styles/components/default_list_row_style.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Theme/Styles/Components/DefaultPanelStyle.swift` | 48 | 未开始 | `Compose/rust/theme/styles/components/default_panel_style.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Theme/Styles/Components/DefaultTextFieldStyle.swift` | 48 | 未开始 | `Compose/rust/theme/styles/components/default_text_field_style.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Theme/Styles/Components/DefaultTreeRowStyle.swift` | 62 | 未开始 | `Compose/rust/theme/styles/components/default_tree_row_style.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Theme/Styles/Components/DividerStyle.swift` | 42 | 已写入（未验证） | `Compose/rust/theme/styles/components/divider_style.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Styles/Components/EditorTextFieldStyle.swift` | 18 | 未开始 | `Compose/rust/theme/styles/components/editor_text_field_style.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Theme/Styles/Components/InputAppearance.swift` | 72 | 已写入（未验证） | `Compose/rust/theme/styles/components/input_appearance.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Styles/Components/ListRowStyle.swift` | 64 | 未开始 | `Compose/rust/theme/styles/components/list_row_style.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Theme/Styles/Components/PanelStyle.swift` | 40 | 未开始 | `Compose/rust/theme/styles/components/panel_style.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Theme/Styles/Components/SelectionAppearance.swift` | 83 | 已写入（未验证） | `Compose/rust/theme/styles/components/selection_appearance.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Styles/Components/SliderStyle.swift` | 161 | 未开始 | `Compose/rust/theme/styles/components/slider_style.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Theme/Styles/Components/TextFieldStyle.swift` | 71 | 未开始 | `Compose/rust/theme/styles/components/text_field_style.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Theme/Styles/Components/TreeRowStyle.swift` | 78 | 未开始 | `Compose/rust/theme/styles/components/tree_row_style.rs`<br>（计划） | 依赖 Composition/Core；控件 API 与交互层尚有缺项 |
| `Compose/Sources/CoveUICompose/Theme/Styles/DefaultDarkTheme.swift` | 95 | 已写入（未验证） | `Compose/rust/theme/styles/default_dark_theme.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Styles/DefaultLightTheme.swift` | 85 | 已写入（未验证） | `Compose/rust/theme/styles/default_light_theme.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Theme.swift` | 58 | 已写入（未验证） | `Compose/rust/theme/theme.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/ThemeEnvironment.swift` | 63 | 已写入（未验证） | `Compose/rust/theme/theme_environment.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/ThemeReader.swift` | 20 | 已写入（未验证） | `Compose/rust/theme/theme_reader.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Tokens/ColorScheme.swift` | 151 | 已写入（未验证） | `Compose/rust/theme/tokens/color_scheme.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Tokens/ControlSize.swift` | 61 | 已写入（未验证） | `Compose/rust/theme/tokens/control_size.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Tokens/ElevationScale.swift` | 26 | 已写入（未验证） | `Compose/rust/theme/tokens/elevation_scale.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Tokens/Extensions.swift` | 21 | 已写入（未验证） | `Compose/rust/theme/tokens/extensions.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Tokens/FloatingSurfaceTokens.swift` | 17 | 已写入（未验证） | `Compose/rust/theme/tokens/floating_surface_tokens.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Tokens/Icons.swift` | 78 | 已写入（未验证） | `Compose/rust/theme/tokens/icons.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Tokens/Metrics.swift` | 26 | 已写入（未验证） | `Compose/rust/theme/tokens/metrics.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Tokens/MotionScale.swift` | 59 | 已写入（未验证） | `Compose/rust/theme/tokens/motion_scale.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Tokens/RadiusScale.swift` | 19 | 已写入（未验证） | `Compose/rust/theme/tokens/radius_scale.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Tokens/ReducedMotionEnvironment.swift` | 13 | 已写入（未验证） | `Compose/rust/theme/tokens/reduced_motion_environment.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Tokens/SemanticColor.swift` | 86 | 已写入（未验证） | `Compose/rust/theme/tokens/semantic_color.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Tokens/SemanticModifiers.swift` | 92 | 已写入（未验证） | `Compose/rust/theme/tokens/semantic_modifiers.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Tokens/SemanticMotion.swift` | 108 | 已写入（未验证） | `Compose/rust/theme/tokens/semantic_motion.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Tokens/SpacingScale.swift` | 20 | 已写入（未验证） | `Compose/rust/theme/tokens/spacing_scale.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Tokens/TextEmphasis.swift` | 12 | 已写入（未验证） | `Compose/rust/theme/tokens/text_emphasis.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Tokens/TextScaler.swift` | 76 | 已写入（未验证） | `Compose/rust/theme/tokens/text_scaler.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Tokens/Typography.swift` | 74 | 已写入（未验证） | `Compose/rust/theme/tokens/typography.rs` | 未编译、未行为对照 |

### Terminal

共 29 个文件：已写入 0，进行中 0，未开始 29。

| 源文件 | 行 | 状态 | Rust/C 目标 | 依赖或阻塞 |
| --- | ---: | --- | --- | --- |
| `Terminal/Sources/CoveUITerminal/Engine/Engine.swift` | 202 | 未开始 | — | 依赖 System/Graphics/Compose；Ghostty 引擎与 PTY/ConPTY |
| `Terminal/Sources/CoveUITerminal/Engine/Frame.swift` | 100 | 未开始 | — | 依赖 System/Graphics/Compose；Ghostty 引擎与 PTY/ConPTY |
| `Terminal/Sources/CoveUITerminal/Engine/FrameReader.swift` | 140 | 未开始 | — | 依赖 System/Graphics/Compose；Ghostty 引擎与 PTY/ConPTY |
| `Terminal/Sources/CoveUITerminal/Engine/Hyperlink.swift` | 18 | 未开始 | — | 依赖 System/Graphics/Compose；Ghostty 引擎与 PTY/ConPTY |
| `Terminal/Sources/CoveUITerminal/Engine/Images.swift` | 136 | 未开始 | — | 依赖 System/Graphics/Compose；Ghostty 引擎与 PTY/ConPTY |
| `Terminal/Sources/CoveUITerminal/Engine/KeyEncoder.swift` | 123 | 未开始 | — | 依赖 System/Graphics/Compose；Ghostty 引擎与 PTY/ConPTY |
| `Terminal/Sources/CoveUITerminal/Engine/Mouse.swift` | 94 | 未开始 | — | 依赖 System/Graphics/Compose；Ghostty 引擎与 PTY/ConPTY |
| `Terminal/Sources/CoveUITerminal/Engine/PlaceholderDiacritics.swift` | 30 | 未开始 | — | 依赖 System/Graphics/Compose；Ghostty 引擎与 PTY/ConPTY |
| `Terminal/Sources/CoveUITerminal/Engine/Placeholders.swift` | 182 | 未开始 | — | 依赖 System/Graphics/Compose；Ghostty 引擎与 PTY/ConPTY |
| `Terminal/Sources/CoveUITerminal/Engine/Selection.swift` | 144 | 未开始 | — | 依赖 System/Graphics/Compose；Ghostty 引擎与 PTY/ConPTY |
| `Terminal/Sources/CoveUITerminal/Engine/Sized.swift` | 7 | 未开始 | — | 依赖 System/Graphics/Compose；Ghostty 引擎与 PTY/ConPTY |
| `Terminal/Sources/CoveUITerminal/Process/Command.swift` | 77 | 未开始 | — | 依赖 System/Graphics/Compose；Ghostty 引擎与 PTY/ConPTY |
| `Terminal/Sources/CoveUITerminal/Process/ConPTY.swift` | 179 | 未开始 | — | 依赖 System/Graphics/Compose；Ghostty 引擎与 PTY/ConPTY |
| `Terminal/Sources/CoveUITerminal/Process/POSIX.swift` | 176 | 未开始 | — | 依赖 System/Graphics/Compose；Ghostty 引擎与 PTY/ConPTY |
| `Terminal/Sources/CoveUITerminal/Process/PseudoTerminal.swift` | 83 | 未开始 | — | 依赖 System/Graphics/Compose；Ghostty 引擎与 PTY/ConPTY |
| `Terminal/Sources/CoveUITerminal/Process/WindowsCommandLine.swift` | 30 | 未开始 | — | 依赖 System/Graphics/Compose；Ghostty 引擎与 PTY/ConPTY |
| `Terminal/Sources/CoveUITerminal/Session.swift` | 259 | 未开始 | — | 依赖 System/Graphics/Compose；Ghostty 引擎与 PTY/ConPTY |
| `Terminal/Sources/CoveUITerminal/View/BlinkTimer.swift` | 54 | 未开始 | — | 依赖 System/Graphics/Compose；Ghostty 引擎与 PTY/ConPTY |
| `Terminal/Sources/CoveUITerminal/View/BoxDrawing.swift` | 246 | 未开始 | — | 依赖 System/Graphics/Compose；Ghostty 引擎与 PTY/ConPTY |
| `Terminal/Sources/CoveUITerminal/View/CellMetrics.swift` | 24 | 未开始 | — | 依赖 System/Graphics/Compose；Ghostty 引擎与 PTY/ConPTY |
| `Terminal/Sources/CoveUITerminal/View/ImageTextures.swift` | 27 | 未开始 | — | 依赖 System/Graphics/Compose；Ghostty 引擎与 PTY/ConPTY |
| `Terminal/Sources/CoveUITerminal/View/KeyRouting.swift` | 71 | 未开始 | — | 依赖 System/Graphics/Compose；Ghostty 引擎与 PTY/ConPTY |
| `Terminal/Sources/CoveUITerminal/View/LinkOpener.swift` | 26 | 未开始 | — | 依赖 System/Graphics/Compose；Ghostty 引擎与 PTY/ConPTY |
| `Terminal/Sources/CoveUITerminal/View/Painter.swift` | 173 | 未开始 | — | 依赖 System/Graphics/Compose；Ghostty 引擎与 PTY/ConPTY |
| `Terminal/Sources/CoveUITerminal/View/PointerRouter.swift` | 136 | 未开始 | — | 依赖 System/Graphics/Compose；Ghostty 引擎与 PTY/ConPTY |
| `Terminal/Sources/CoveUITerminal/View/SurfaceState.swift` | 87 | 未开始 | — | 依赖 System/Graphics/Compose；Ghostty 引擎与 PTY/ConPTY |
| `Terminal/Sources/CoveUITerminal/View/TerminalView.swift` | 174 | 未开始 | — | 依赖 System/Graphics/Compose；Ghostty 引擎与 PTY/ConPTY |
| `Terminal/Sources/CoveUITerminal/View/URLDetector.swift` | 59 | 未开始 | — | 依赖 System/Graphics/Compose；Ghostty 引擎与 PTY/ConPTY |
| `Terminal/Sources/TerminalDemo/main.swift` | 31 | 未开始 | — | 依赖 System/Graphics/Compose；Ghostty 引擎与 PTY/ConPTY |

### Hosts

共 51 个文件：已写入 10，进行中 0，未开始 41。

| 源文件 | 行 | 状态 | Rust/C 目标 | 依赖或阻塞 |
| --- | ---: | --- | --- | --- |
| `Hosts/Android/Contract/Sources/CoveAndroidContract/FrameClock.swift` | 21 | 已写入（未验证） | `Hosts/rust/android/contract/frame_clock.rs` | 未编译、未行为对照 |
| `Hosts/Android/Contract/Sources/CoveAndroidContract/Launch.swift` | 14 | 已写入（未验证） | `Hosts/rust/android/contract/launch.rs` | 未编译、未行为对照 |
| `Hosts/Android/Integration/Diagnostics.swift` | 54 | 已写入（未验证） | `Hosts/rust/android/integration/diagnostics.rs` | 未编译、未行为对照 |
| `Hosts/Android/Integration/Environment.swift` | 45 | 已写入（未验证） | `Hosts/rust/android/integration/environment.rs` | 未编译、未行为对照 |
| `Hosts/Android/Integration/Inspection.swift` | 21 | 已写入（未验证） | `Hosts/rust/android/integration/inspection.rs` | 未编译、未行为对照 |
| `Hosts/Android/Integration/Scene.swift` | 196 | 已写入（未验证） | `Hosts/rust/android/integration/scene.rs` | 未编译、未行为对照 |
| `Hosts/Android/Integration/Surface/State.swift` | 56 | 已写入（未验证） | `Hosts/rust/android/integration/surface/state.rs` | 未编译、未行为对照 |
| `Hosts/Android/Integration/TouchInput.swift` | 52 | 已写入（未验证） | `Hosts/rust/android/integration/touch_input.rs` | 未编译、未行为对照 |
| `Hosts/Android/Sources/CoveAndroidApp/Application.swift` | 39 | 未开始 | — | 依赖 Runtime/Compose/Graphics；宿主平台生命周期 |
| `Hosts/Android/Sources/CoveAndroidApp/Bridge.swift` | 185 | 未开始 | — | 依赖 Runtime/Compose/Graphics；宿主平台生命周期 |
| `Hosts/Android/Sources/CoveAndroidApp/Content/InputSnapshot.swift` | 17 | 未开始 | — | 依赖 Runtime/Compose/Graphics；宿主平台生命周期 |
| `Hosts/Android/Sources/CoveAndroidApp/Content/Inspection.swift` | 29 | 未开始 | — | 依赖 Runtime/Compose/Graphics；宿主平台生命周期 |
| `Hosts/Android/Sources/CoveAndroidApp/Content/Scene.swift` | 157 | 未开始 | — | 依赖 Runtime/Compose/Graphics；宿主平台生命周期 |
| `Hosts/Android/Sources/CoveAndroidApp/Frame/GPUTimings.swift` | 85 | 未开始 | — | 依赖 Runtime/Compose/Graphics；宿主平台生命周期 |
| `Hosts/Android/Sources/CoveAndroidApp/Frame/RuntimeOptions.swift` | 29 | 已写入（未验证） | `Hosts/rust/android/sources/cove_android_app/frame/runtime_options.rs` | 未编译、未行为对照 |
| `Hosts/Android/Sources/CoveAndroidApp/Frame/Timings.swift` | 46 | 已写入（未验证） | `Hosts/rust/android/sources/cove_android_app/frame/timings.rs` | 未编译、未行为对照 |
| `Hosts/Android/Sources/CoveAndroidApp/SurfaceSession.swift` | 92 | 未开始 | — | 依赖 Runtime/Compose/Graphics；宿主平台生命周期 |
| `Hosts/Android/Sources/CoveAndroidApp/Validation/Counter/Typography.swift` | 43 | 未开始 | — | 依赖 Runtime/Compose/Graphics；宿主平台生命周期 |
| `Hosts/Android/Sources/CoveAndroidApp/Validation/Showcase.swift` | 37 | 未开始 | — | 依赖 Runtime/Compose/Graphics；宿主平台生命周期 |
| `Hosts/Android/Sources/CoveAndroidProbe/Layout.swift` | 20 | 未开始 | — | 依赖 Runtime/Compose/Graphics；宿主平台生命周期 |
| `Hosts/Android/Sources/CoveAndroidProbe/Raster.swift` | 58 | 未开始 | — | 依赖 Runtime/Compose/Graphics；宿主平台生命周期 |
| `Hosts/Android/Sources/CoveAndroidProbe/Runner.swift` | 55 | 未开始 | — | 依赖 Runtime/Compose/Graphics；宿主平台生命周期 |
| `Hosts/Android/Sources/CoveAndroidProbe/Transfer.swift` | 43 | 未开始 | — | 依赖 Runtime/Compose/Graphics；宿主平台生命周期 |
| `Hosts/Android/Sources/CoveAndroidProbe/UI.swift` | 50 | 未开始 | — | 依赖 Runtime/Compose/Graphics；宿主平台生命周期 |
| `Hosts/Android/Sources/CoveAndroidProbe/main.swift` | 45 | 未开始 | — | 依赖 Runtime/Compose/Graphics；宿主平台生命周期 |
| `Hosts/Desktop/Sources/CoveUI/Application/AdaptiveContent.swift` | 15 | 未开始 | — | 依赖 Runtime/Compose/Graphics；宿主平台生命周期 |
| `Hosts/Desktop/Sources/CoveUI/Application/Callbacks.swift` | 10 | 未开始 | — | 依赖 Runtime/Compose/Graphics；宿主平台生命周期 |
| `Hosts/Desktop/Sources/CoveUI/Application/Composition.swift` | 22 | 未开始 | — | 依赖 Runtime/Compose/Graphics；宿主平台生命周期 |
| `Hosts/Desktop/Sources/CoveUI/Application/Configuration.swift` | 100 | 未开始 | — | 依赖 Runtime/Compose/Graphics；宿主平台生命周期 |
| `Hosts/Desktop/Sources/CoveUI/Application/Memory.swift` | 94 | 未开始 | — | 依赖 Runtime/Compose/Graphics；宿主平台生命周期 |
| `Hosts/Desktop/Sources/CoveUI/Application/MemoryPressureProbe.swift` | 69 | 未开始 | — | 依赖 Runtime/Compose/Graphics；宿主平台生命周期 |
| `Hosts/Desktop/Sources/CoveUI/Application/Runtime.swift` | 911 | 未开始 | — | 依赖 Runtime/Compose/Graphics；宿主平台生命周期 |
| `Hosts/Desktop/Sources/CoveUI/Application/Services.swift` | 31 | 未开始 | — | 依赖 Runtime/Compose/Graphics；宿主平台生命周期 |
| `Hosts/Desktop/Sources/CoveUI/Application/Typography.swift` | 7 | 未开始 | — | 依赖 Runtime/Compose/Graphics；宿主平台生命周期 |
| `Hosts/Desktop/Sources/CoveUI/Diagnostics/Checkpoint.swift` | 13 | 未开始 | — | 依赖 Runtime/Compose/Graphics；宿主平台生命周期 |
| `Hosts/Desktop/Sources/CoveUI/Diagnostics/FrameMetrics.swift` | 44 | 未开始 | — | 依赖 Runtime/Compose/Graphics；宿主平台生命周期 |
| `Hosts/Desktop/Sources/CoveUI/Diagnostics/PerformanceHUD.swift` | 30 | 未开始 | — | 依赖 Runtime/Compose/Graphics；宿主平台生命周期 |
| `Hosts/Desktop/Sources/CoveUI/Diagnostics/Recording.swift` | 40 | 未开始 | — | 依赖 Runtime/Compose/Graphics；宿主平台生命周期 |
| `Hosts/Desktop/Sources/CoveUI/Diagnostics/Session.swift` | 7 | 未开始 | — | 依赖 Runtime/Compose/Graphics；宿主平台生命周期 |
| `Hosts/Desktop/Sources/CoveUI/Display/Controls.swift` | 30 | 未开始 | — | 依赖 Runtime/Compose/Graphics；宿主平台生命周期 |
| `Hosts/Desktop/Sources/CoveUI/Display/Handle.swift` | 303 | 未开始 | — | 依赖 Runtime/Compose/Graphics；宿主平台生命周期 |
| `Hosts/Desktop/Sources/CoveUI/Exports.swift` | 7 | 未开始 | — | 依赖 Runtime/Compose/Graphics；宿主平台生命周期 |
| `Hosts/Desktop/Sources/CoveUI/Rendering/Context.swift` | 45 | 未开始 | — | 依赖 Runtime/Compose/Graphics；宿主平台生命周期 |
| `Hosts/Desktop/Sources/CoveUI/ResourcesSupport/Bundle.swift` | 9 | 未开始 | — | 依赖 Runtime/Compose/Graphics；宿主平台生命周期 |
| `Hosts/Desktop/Sources/CoveUI/ResourcesSupport/Lookup.swift` | 6 | 未开始 | — | 依赖 Runtime/Compose/Graphics；宿主平台生命周期 |
| `Hosts/Desktop/Sources/CoveUI/Window/Auxiliary.swift` | 202 | 未开始 | — | 依赖 Runtime/Compose/Graphics；宿主平台生命周期 |
| `Hosts/Desktop/Sources/CoveUI/Window/NativeMenu.swift` | 228 | 未开始 | — | 依赖 Runtime/Compose/Graphics；宿主平台生命周期 |
| `Hosts/Desktop/Sources/CoveUI/Window/NativeRenderer.swift` | 115 | 未开始 | — | 依赖 Runtime/Compose/Graphics；宿主平台生命周期 |
| `Hosts/Desktop/Sources/CoveUI/Window/Renderer.swift` | 15 | 未开始 | — | 依赖 Runtime/Compose/Graphics；宿主平台生命周期 |
| `Hosts/Desktop/Sources/CoveUI/Window/State.swift` | 15 | 未开始 | — | 依赖 Runtime/Compose/Graphics；宿主平台生命周期 |
| `Hosts/Desktop/Sources/CoveUI/Window/WindowChrome.swift` | 397 | 未开始 | — | 依赖 Runtime/Compose/Graphics；宿主平台生命周期 |

### DevTools

共 51 个文件：已写入 8，进行中 0，未开始 43。

| 源文件 | 行 | 状态 | Rust/C 目标 | 依赖或阻塞 |
| --- | ---: | --- | --- | --- |
| `DevTools/Inspection/Sources/CoveUIAgentCore/Contracts/Schema.swift` | 479 | 未开始 | — | 依赖 Protocol/Compose/Runtime；客户端/服务端/Agent |
| `DevTools/Inspection/Sources/CoveUIAgentCore/Contracts/Tool.swift` | 51 | 未开始 | — | 依赖 Protocol/Compose/Runtime；客户端/服务端/Agent |
| `DevTools/Inspection/Sources/CoveUIAgentCore/MCP/Server.swift` | 105 | 未开始 | — | 依赖 Protocol/Compose/Runtime；客户端/服务端/Agent |
| `DevTools/Inspection/Sources/CoveUIAgentCore/Session/Conversation.swift` | 86 | 未开始 | — | 依赖 Protocol/Compose/Runtime；客户端/服务端/Agent |
| `DevTools/Inspection/Sources/CoveUIAgentCore/Skills/Directory.swift` | 49 | 未开始 | — | 依赖 Protocol/Compose/Runtime；客户端/服务端/Agent |
| `DevTools/Inspection/Sources/CoveUIAgentCore/Skills/Skill.swift` | 42 | 未开始 | — | 依赖 Protocol/Compose/Runtime；客户端/服务端/Agent |
| `DevTools/Inspection/Sources/CoveUIAgentCore/Tools/Diagnostics.swift` | 92 | 未开始 | — | 依赖 Protocol/Compose/Runtime；客户端/服务端/Agent |
| `DevTools/Inspection/Sources/CoveUIAgentCore/Tools/Toolbox.swift` | 44 | 未开始 | — | 依赖 Protocol/Compose/Runtime；客户端/服务端/Agent |
| `DevTools/Inspection/Sources/CoveUIDevToolsScene/Diagnostics/Actions.swift` | 41 | 未开始 | — | 依赖 Protocol/Compose/Runtime；客户端/服务端/Agent |
| `DevTools/Inspection/Sources/CoveUIDevToolsScene/Diagnostics/Geometry.swift` | 72 | 未开始 | — | 依赖 Protocol/Compose/Runtime；客户端/服务端/Agent |
| `DevTools/Inspection/Sources/CoveUIDevToolsScene/Diagnostics/Memory.swift` | 11 | 未开始 | — | 依赖 Protocol/Compose/Runtime；客户端/服务端/Agent |
| `DevTools/Inspection/Sources/CoveUIDevToolsScene/Diagnostics/Presentation.swift` | 62 | 未开始 | — | 依赖 Protocol/Compose/Runtime；客户端/服务端/Agent |
| `DevTools/Inspection/Sources/CoveUIDevToolsScene/Diagnostics/Queries.swift` | 200 | 未开始 | — | 依赖 Protocol/Compose/Runtime；客户端/服务端/Agent |
| `DevTools/Inspection/Sources/CoveUIDevToolsScene/Diagnostics/Requests.swift` | 50 | 未开始 | — | 依赖 Protocol/Compose/Runtime；客户端/服务端/Agent |
| `DevTools/Inspection/Sources/CoveUIDevToolsScene/Diagnostics/Tools.swift` | 222 | 未开始 | — | 依赖 Protocol/Compose/Runtime；客户端/服务端/Agent |
| `DevTools/Inspection/Sources/CoveUIDevToolsScene/Diagnostics/Trace.swift` | 103 | 未开始 | — | 依赖 Protocol/Compose/Runtime；客户端/服务端/Agent |
| `DevTools/Inspection/Sources/CoveUIDevToolsScene/Observation.swift` | 19 | 未开始 | — | 依赖 Protocol/Compose/Runtime；客户端/服务端/Agent |
| `DevTools/Inspection/Sources/CoveUIDevToolsScene/SceneEditor.swift` | 209 | 未开始 | — | 依赖 Protocol/Compose/Runtime；客户端/服务端/Agent |
| `DevTools/Inspection/Sources/CoveUIDevToolsScene/SceneInspector.swift` | 177 | 未开始 | — | 依赖 Protocol/Compose/Runtime；客户端/服务端/Agent |
| `DevTools/Protocol/Sources/CoveUIDevToolsProtocol/ComponentAnalysis.swift` | 25 | 已写入（未验证） | `DevTools/Protocol/rust/lib.rs` | 未编译、未行为对照 |
| `DevTools/Protocol/Sources/CoveUIDevToolsProtocol/DevToolsSession.swift` | 104 | 已写入（未验证） | `DevTools/Protocol/rust/session.rs` | 未编译、未行为对照 |
| `DevTools/Protocol/Sources/CoveUIDevToolsProtocol/Discovery.swift` | 22 | 已写入（未验证） | `DevTools/Protocol/rust/lib.rs` | 未编译、未行为对照 |
| `DevTools/Protocol/Sources/CoveUIDevToolsProtocol/Exports.swift` | 1 | 已写入（未验证） | `DevTools/Protocol/rust/lib.rs` | 未编译、未行为对照 |
| `DevTools/Protocol/Sources/CoveUIDevToolsProtocol/InputRecording.swift` | 75 | 已写入（未验证） | `DevTools/Protocol/rust/recording.rs` | 未编译、未行为对照 |
| `DevTools/Protocol/Sources/CoveUIDevToolsProtocol/Inspection.swift` | 86 | 已写入（未验证） | `DevTools/Protocol/rust/lib.rs` | 未编译、未行为对照 |
| `DevTools/Protocol/Sources/CoveUIDevToolsProtocol/Observation.swift` | 57 | 已写入（未验证） | `DevTools/Protocol/rust/lib.rs` | 未编译、未行为对照 |
| `DevTools/Protocol/Sources/CoveUIDevToolsProtocol/Protocol.swift` | 486 | 已写入（未验证） | `DevTools/Protocol/rust/lib.rs` | 未编译、未行为对照 |
| `DevTools/Sources/CoveUIAgent/Exports.swift` | 3 | 未开始 | — | 依赖 Protocol/Compose/Runtime；客户端/服务端/Agent |
| `DevTools/Sources/CoveUIAgentTransport/LocalServer.swift` | 110 | 未开始 | — | 依赖 Protocol/Compose/Runtime；客户端/服务端/Agent |
| `DevTools/Sources/CoveUIDevTools/Integration/Exports.swift` | 4 | 未开始 | — | 依赖 Protocol/Compose/Runtime；客户端/服务端/Agent |
| `DevTools/Sources/CoveUIDevTools/Mirror/NativeAppFrameCapture.swift` | 52 | 未开始 | — | 依赖 Protocol/Compose/Runtime；客户端/服务端/Agent |
| `DevTools/Sources/CoveUIDevTools/Session/DevTools.swift` | 230 | 未开始 | — | 依赖 Protocol/Compose/Runtime；客户端/服务端/Agent |
| `DevTools/Sources/CoveUIDevTools/Session/Discovery.swift` | 19 | 未开始 | — | 依赖 Protocol/Compose/Runtime；客户端/服务端/Agent |
| `DevTools/Sources/CoveUIDevTools/Taps/FrameTap.swift` | 276 | 未开始 | — | 依赖 Protocol/Compose/Runtime；客户端/服务端/Agent |
| `DevTools/Sources/CoveUIDevTools/Taps/InputBridge.swift` | 151 | 未开始 | — | 依赖 Protocol/Compose/Runtime；客户端/服务端/Agent |
| `DevTools/Sources/CoveUIDevTools/Taps/TimingPublisher.swift` | 45 | 未开始 | — | 依赖 Protocol/Compose/Runtime；客户端/服务端/Agent |
| `DevTools/Sources/CoveUIDevToolsProbe/main.swift` | 72 | 未开始 | — | 依赖 Protocol/Compose/Runtime；客户端/服务端/Agent |
| `DevTools/Sources/CoveUIDevToolsServer/Requests/Checkpoint.swift` | 47 | 未开始 | — | 依赖 Protocol/Compose/Runtime；客户端/服务端/Agent |
| `DevTools/Sources/CoveUIDevToolsServer/Requests/Diagnostics.swift` | 17 | 未开始 | — | 依赖 Protocol/Compose/Runtime；客户端/服务端/Agent |
| `DevTools/Sources/CoveUIDevToolsServer/Requests/Mirror.swift` | 79 | 未开始 | — | 依赖 Protocol/Compose/Runtime；客户端/服务端/Agent |
| `DevTools/Sources/CoveUIDevToolsServer/Requests/Observation.swift` | 45 | 未开始 | — | 依赖 Protocol/Compose/Runtime；客户端/服务端/Agent |
| `DevTools/Sources/CoveUIDevToolsServer/Requests/Recording.swift` | 47 | 未开始 | — | 依赖 Protocol/Compose/Runtime；客户端/服务端/Agent |
| `DevTools/Sources/CoveUIDevToolsServer/Requests/Scene.swift` | 90 | 未开始 | — | 依赖 Protocol/Compose/Runtime；客户端/服务端/Agent |
| `DevTools/Sources/CoveUIDevToolsServer/Requests/Window.swift` | 21 | 未开始 | — | 依赖 Protocol/Compose/Runtime；客户端/服务端/Agent |
| `DevTools/Sources/CoveUIDevToolsServer/Session/Configuration.swift` | 69 | 未开始 | — | 依赖 Protocol/Compose/Runtime；客户端/服务端/Agent |
| `DevTools/Sources/CoveUIDevToolsServer/Session/DevServer.swift` | 454 | 未开始 | — | 依赖 Protocol/Compose/Runtime；客户端/服务端/Agent |
| `DevTools/Sources/CoveUIDevToolsServer/Session/Host.swift` | 53 | 未开始 | — | 依赖 Protocol/Compose/Runtime；客户端/服务端/Agent |
| `DevTools/Sources/CoveUIDevToolsServer/Streams/Logs.swift` | 55 | 未开始 | — | 依赖 Protocol/Compose/Runtime；客户端/服务端/Agent |
| `DevTools/Sources/CoveUIDevToolsServer/Transport/DevToolsTransport.swift` | 24 | 未开始 | — | 依赖 Protocol/Compose/Runtime；客户端/服务端/Agent |
| `DevTools/Sources/CoveUIDevToolsServer/Transport/NIODevToolsTransport.swift` | 190 | 未开始 | — | 依赖 Protocol/Compose/Runtime；客户端/服务端/Agent |
| `DevTools/Sources/CoveUIDevToolsServer/Tree/TreeEncodingWorker.swift` | 60 | 未开始 | — | 依赖 Protocol/Compose/Runtime；客户端/服务端/Agent |

### Sources

共 19 个文件：已写入 0，进行中 0，未开始 19。

| 源文件 | 行 | 状态 | Rust/C 目标 | 依赖或阻塞 |
| --- | ---: | --- | --- | --- |
| `Sources/CoveUIWebView/Apple/WebKitHost.swift` | 22 | 未开始 | — | 依赖 Compose/Runtime；Workspace 与 WebView 宿主 |
| `Sources/CoveUIWebView/Apple/WebKitSurface.swift` | 232 | 未开始 | — | 依赖 Compose/Runtime；Workspace 与 WebView 宿主 |
| `Sources/CoveUIWebView/WebView.swift` | 170 | 未开始 | — | 依赖 Compose/Runtime；Workspace 与 WebView 宿主 |
| `Sources/CoveUIWebView/WebViewHandle.swift` | 51 | 未开始 | — | 依赖 Compose/Runtime；Workspace 与 WebView 宿主 |
| `Sources/CoveUIWebView/WebViewState.swift` | 46 | 未开始 | — | 依赖 Compose/Runtime；Workspace 与 WebView 宿主 |
| `Sources/CoveUIWebView/WebViewSurface.swift` | 77 | 未开始 | — | 依赖 Compose/Runtime；Workspace 与 WebView 宿主 |
| `Sources/CoveUIWorkspace/Icons/CoveUIWorkspaceResourceBundle.swift` | 6 | 未开始 | — | 依赖 Compose/Runtime；Workspace 与 WebView 宿主 |
| `Sources/CoveUIWorkspace/Icons/WorkspacePanelIconCatalog.swift` | 17 | 未开始 | — | 依赖 Compose/Runtime；Workspace 与 WebView 宿主 |
| `Sources/CoveUIWorkspace/Layout/Split.swift` | 32 | 未开始 | — | 依赖 Compose/Runtime；Workspace 与 WebView 宿主 |
| `Sources/CoveUIWorkspace/Layout/Tree.swift` | 49 | 未开始 | — | 依赖 Compose/Runtime；Workspace 与 WebView 宿主 |
| `Sources/CoveUIWorkspace/Observation.swift` | 14 | 未开始 | — | 依赖 Compose/Runtime；Workspace 与 WebView 宿主 |
| `Sources/CoveUIWorkspace/Sizing/Panel.swift` | 47 | 未开始 | — | 依赖 Compose/Runtime；Workspace 与 WebView 宿主 |
| `Sources/CoveUIWorkspace/Sizing/Resolution.swift` | 172 | 未开始 | — | 依赖 Compose/Runtime；Workspace 与 WebView 宿主 |
| `Sources/CoveUIWorkspace/Theme/TabStyle.swift` | 22 | 未开始 | — | 依赖 Compose/Runtime；Workspace 与 WebView 宿主 |
| `Sources/CoveUIWorkspace/Theme/WorkspaceRegionTransition.swift` | 179 | 未开始 | — | 依赖 Compose/Runtime；Workspace 与 WebView 宿主 |
| `Sources/CoveUIWorkspace/Theme/WorkspaceTheme.swift` | 51 | 未开始 | — | 依赖 Compose/Runtime；Workspace 与 WebView 宿主 |
| `Sources/CoveUIWorkspace/WorkspaceController.swift` | 915 | 未开始 | — | 依赖 Compose/Runtime；Workspace 与 WebView 宿主 |
| `Sources/CoveUIWorkspace/WorkspaceModel.swift` | 590 | 未开始 | — | 依赖 Compose/Runtime；Workspace 与 WebView 宿主 |
| `Sources/CoveUIWorkspace/WorkspaceView.swift` | 2169 | 未开始 | — | 依赖 Compose/Runtime；Workspace 与 WebView 宿主 |

### Browser

共 1 个文件：已写入 0，进行中 0，未开始 1。

| 源文件 | 行 | 状态 | Rust/C 目标 | 依赖或阻塞 |
| --- | ---: | --- | --- | --- |
| `Browser/Sources/CoveUIBrowserPrototype/main.swift` | 400 | 未开始 | — | 依赖 Compose/Graphics/DevTools；Wasm 构建链 |

### Tools

共 69 个文件：已写入 12，进行中 41，未开始 16。

| 源文件 | 行 | 状态 | Rust/C 目标 | 依赖或阻塞 |
| --- | ---: | --- | --- | --- |
| `Tools/Cove/Sources/CoveCLI/CLI/Arguments.swift` | 286 | 进行中（有缺项） | `Tools/Cove/rust/cli.rs` | 对应实现仍有缺项 |
| `Tools/Cove/Sources/CoveCLI/CLI/Diagnostics.swift` | 21 | 进行中（有缺项） | `Tools/Cove/rust/support.rs` | 对应实现仍有缺项 |
| `Tools/Cove/Sources/CoveCLI/CLI/Platform.swift` | 51 | 进行中（有缺项） | `Tools/Cove/rust/cli.rs` | 对应实现仍有缺项 |
| `Tools/Cove/Sources/CoveCLI/Development/AndroidRun.swift` | 41 | 进行中（有缺项） | `Tools/Cove/rust/main.rs` | 对应实现仍有缺项 |
| `Tools/Cove/Sources/CoveCLI/Development/Cancellation.swift` | 40 | 进行中（有缺项） | `Tools/Cove/rust/main.rs` | 对应实现仍有缺项 |
| `Tools/Cove/Sources/CoveCLI/Development/DesktopRun.swift` | 36 | 进行中（有缺项） | `Tools/Cove/rust/main.rs` | 对应实现仍有缺项 |
| `Tools/Cove/Sources/CoveCLI/Development/Inspector.swift` | 92 | 进行中（有缺项） | `Tools/Cove/rust/main.rs` | 对应实现仍有缺项 |
| `Tools/Cove/Sources/CoveCLI/Development/Session.swift` | 68 | 进行中（有缺项） | `Tools/Cove/rust/main.rs` | 对应实现仍有缺项 |
| `Tools/Cove/Sources/CoveCLI/Entry.swift` | 159 | 进行中（有缺项） | `Tools/Cove/rust/main.rs` | 对应实现仍有缺项 |
| `Tools/Cove/Sources/CoveCLI/Installation/Archive.swift` | 52 | 进行中（有缺项） | `Tools/Cove/rust/installation.rs` | 对应实现仍有缺项 |
| `Tools/Cove/Sources/CoveCLI/Installation/Directories.swift` | 25 | 进行中（有缺项） | `Tools/Cove/rust/installation.rs` | 对应实现仍有缺项 |
| `Tools/Cove/Sources/CoveCLI/Installation/Installer.swift` | 76 | 进行中（有缺项） | `Tools/Cove/rust/installation.rs` | 对应实现仍有缺项 |
| `Tools/Cove/Sources/CoveCLI/Installation/NativeArchive.swift` | 47 | 进行中（有缺项） | `Tools/Cove/rust/installation.rs` | 对应实现仍有缺项 |
| `Tools/Cove/Sources/CoveCLI/Installation/Receipt.swift` | 19 | 进行中（有缺项） | `Tools/Cove/rust/installation.rs` | 对应实现仍有缺项 |
| `Tools/Cove/Sources/CoveCLI/Installation/RuntimeLibraries.swift` | 46 | 进行中（有缺项） | `Tools/Cove/rust/installation.rs` | 对应实现仍有缺项 |
| `Tools/Cove/Sources/CoveCLI/Installation/SDKPayload.swift` | 49 | 进行中（有缺项） | `Tools/Cove/rust/installation.rs` | 对应实现仍有缺项 |
| `Tools/Cove/Sources/CoveCLI/Installation/TarArchive.swift` | 23 | 进行中（有缺项） | `Tools/Cove/rust/installation.rs` | 对应实现仍有缺项 |
| `Tools/Cove/Sources/CoveCLI/Installation/Update.swift` | 57 | 进行中（有缺项） | `Tools/Cove/rust/installation.rs` | 对应实现仍有缺项 |
| `Tools/Cove/Sources/CoveCLI/Platforms/Android/APK/Assets.swift` | 38 | 未开始 | — | CLI 平台命令、工具链与模板 |
| `Tools/Cove/Sources/CoveCLI/Platforms/Android/APK/Device.swift` | 39 | 未开始 | — | CLI 平台命令、工具链与模板 |
| `Tools/Cove/Sources/CoveCLI/Platforms/Android/APK/Package.swift` | 66 | 未开始 | `Tools/Cove/Sources/CoveCLI/Platforms/Android/APK/Cargo.toml`<br>（计划） | CLI 平台命令、工具链与模板 |
| `Tools/Cove/Sources/CoveCLI/Platforms/Android/APK/Signing.swift` | 38 | 未开始 | — | CLI 平台命令、工具链与模板 |
| `Tools/Cove/Sources/CoveCLI/Platforms/Android/Compilation/Application.swift` | 41 | 未开始 | — | CLI 平台命令、工具链与模板 |
| `Tools/Cove/Sources/CoveCLI/Platforms/Android/Compilation/Artifact.swift` | 17 | 未开始 | — | CLI 平台命令、工具链与模板 |
| `Tools/Cove/Sources/CoveCLI/Platforms/Android/Compilation/NDK.swift` | 28 | 未开始 | — | CLI 平台命令、工具链与模板 |
| `Tools/Cove/Sources/CoveCLI/Platforms/Android/Compilation/NativeDependencies.swift` | 37 | 未开始 | — | CLI 平台命令、工具链与模板 |
| `Tools/Cove/Sources/CoveCLI/Platforms/Android/Compilation/Options.swift` | 32 | 未开始 | — | CLI 平台命令、工具链与模板 |
| `Tools/Cove/Sources/CoveCLI/Platforms/Android/Compilation/SDKOverlay.swift` | 71 | 未开始 | — | CLI 平台命令、工具链与模板 |
| `Tools/Cove/Sources/CoveCLI/Platforms/Android/Compilation/Toolchain.swift` | 102 | 未开始 | — | CLI 平台命令、工具链与模板 |
| `Tools/Cove/Sources/CoveCLI/Platforms/Desktop/Build.swift` | 57 | 未开始 | — | CLI 平台命令、工具链与模板 |
| `Tools/Cove/Sources/CoveCLI/Platforms/Desktop/Bundle.swift` | 46 | 未开始 | — | CLI 平台命令、工具链与模板 |
| `Tools/Cove/Sources/CoveCLI/Platforms/Desktop/Launcher.swift` | 27 | 未开始 | — | CLI 平台命令、工具链与模板 |
| `Tools/Cove/Sources/CoveCLI/Platforms/Desktop/NativeResources.swift` | 78 | 未开始 | — | CLI 平台命令、工具链与模板 |
| `Tools/Cove/Sources/CoveCLI/Platforms/iOS/ApplicationBuild.swift` | 66 | 未开始 | — | CLI 平台命令、工具链与模板 |
| `Tools/Cove/Sources/CoveCLI/Project/Configuration.swift` | 131 | 进行中（有缺项） | `Tools/Cove/rust/project.rs` | 对应实现仍有缺项 |
| `Tools/Cove/Sources/CoveCLI/Project/Create.swift` | 39 | 进行中（有缺项） | `Tools/Cove/rust/project.rs` | 对应实现仍有缺项 |
| `Tools/Cove/Sources/CoveCLI/Project/Identity.swift` | 16 | 进行中（有缺项） | `Tools/Cove/rust/project.rs` | 对应实现仍有缺项 |
| `Tools/Cove/Sources/CoveCLI/Project/SDK/Artifacts.swift` | 82 | 进行中（有缺项） | `Tools/Cove/rust/project_sdk.rs` | 对应实现仍有缺项 |
| `Tools/Cove/Sources/CoveCLI/Project/SDK/Command.swift` | 68 | 进行中（有缺项） | `Tools/Cove/rust/project_sdk.rs` | 对应实现仍有缺项 |
| `Tools/Cove/Sources/CoveCLI/Project/SDK/Download.swift` | 21 | 进行中（有缺项） | `Tools/Cove/rust/project_sdk.rs` | 对应实现仍有缺项 |
| `Tools/Cove/Sources/CoveCLI/Project/SDK/Location.swift` | 74 | 进行中（有缺项） | `Tools/Cove/rust/project_sdk.rs` | 对应实现仍有缺项 |
| `Tools/Cove/Sources/CoveCLI/Project/SDK/Release.swift` | 43 | 进行中（有缺项） | `Tools/Cove/rust/project_sdk.rs` | 对应实现仍有缺项 |
| `Tools/Cove/Sources/CoveCLI/Project/SDK/Store.swift` | 70 | 进行中（有缺项） | `Tools/Cove/rust/project_sdk.rs` | 对应实现仍有缺项 |
| `Tools/Cove/Sources/CoveCLI/Project/Tasks/Dependencies.swift` | 51 | 进行中（有缺项） | `Tools/Cove/rust/project_tasks.rs` | 对应实现仍有缺项 |
| `Tools/Cove/Sources/CoveCLI/Project/Tasks/Runner.swift` | 93 | 进行中（有缺项） | `Tools/Cove/rust/project_tasks.rs` | 对应实现仍有缺项 |
| `Tools/Cove/Sources/CoveCLI/Project/Tasks/Workspace.swift` | 112 | 进行中（有缺项） | `Tools/Cove/rust/project_tasks.rs` | 对应实现仍有缺项 |
| `Tools/Cove/Sources/CoveCLI/Project/Templates.swift` | 14 | 进行中（有缺项） | `Tools/Cove/rust/project.rs` | 对应实现仍有缺项 |
| `Tools/Cove/Sources/CoveCLI/Project/Testing.swift` | 59 | 进行中（有缺项） | `Tools/Cove/rust/project_tasks.rs` | 对应实现仍有缺项 |
| `Tools/Cove/Sources/CoveCLI/Project/Version.swift` | 20 | 进行中（有缺项） | `Tools/Cove/rust/project.rs` | 对应实现仍有缺项 |
| `Tools/Cove/Sources/CoveCLI/Support/DeveloperTool.swift` | 30 | 进行中（有缺项） | `Tools/Cove/rust/support.rs` | 对应实现仍有缺项 |
| `Tools/Cove/Sources/CoveCLI/Support/Files.swift` | 70 | 进行中（有缺项） | `Tools/Cove/rust/support.rs` | 对应实现仍有缺项 |
| `Tools/Cove/Sources/CoveCLI/Support/Process.swift` | 76 | 进行中（有缺项） | `Tools/Cove/rust/support.rs` | 对应实现仍有缺项 |
| `Tools/Cove/Sources/CoveCLI/Support/ProcessStream.swift` | 42 | 进行中（有缺项） | `Tools/Cove/rust/support.rs` | 对应实现仍有缺项 |
| `Tools/Cove/Sources/CoveCLI/Toolchains/Android.swift` | 29 | 进行中（有缺项） | `Tools/Cove/rust/installation.rs` | 对应实现仍有缺项 |
| `Tools/Cove/Sources/CoveCLI/Toolchains/Native.swift` | 45 | 进行中（有缺项） | `Tools/Cove/rust/installation.rs` | 对应实现仍有缺项 |
| `Tools/Cove/Sources/CoveCLI/Toolchains/Requirements.swift` | 50 | 进行中（有缺项） | `Tools/Cove/rust/installation.rs` | 对应实现仍有缺项 |
| `Tools/Cove/Sources/CoveCLI/Toolchains/Setup.swift` | 30 | 进行中（有缺项） | `Tools/Cove/rust/installation.rs` | 对应实现仍有缺项 |
| `Tools/Support/Sources/ToolSupport/Archive.swift` | 24 | 已写入（未验证） | `Tools/Support/rust/archive.rs` | 未编译、未行为对照 |
| `Tools/Support/Sources/ToolSupport/Concurrency.swift` | 20 | 已写入（未验证） | `Tools/Support/rust/concurrency.rs` | 未编译、未行为对照 |
| `Tools/Support/Sources/ToolSupport/Download.swift` | 110 | 已写入（未验证） | `Tools/Support/rust/download.rs` | 未编译、未行为对照 |
| `Tools/Support/Sources/ToolSupport/Files.swift` | 94 | 已写入（未验证） | `Tools/Support/rust/files.rs` | 未编译、未行为对照 |
| `Tools/Support/Sources/ToolSupport/NSNumber+Boolean.swift` | 19 | 已写入（未验证） | `Tools/Support/rust/nsnumber_boolean.rs` | 未编译、未行为对照 |
| `Tools/Support/Sources/ToolSupport/ProcessRunner.swift` | 45 | 已写入（未验证） | `Tools/Support/rust/process_runner.rs` | 未编译、未行为对照 |
| `Tools/Support/Sources/ToolSupport/ProcessSignal.swift` | 20 | 已写入（未验证） | `Tools/Support/rust/process_signal.rs` | 未编译、未行为对照 |
| `Tools/Support/Sources/ToolSupport/Repository.swift` | 31 | 已写入（未验证） | `Tools/Support/rust/repository.rs` | 未编译、未行为对照 |
| `Tools/Support/Sources/ToolSupport/SHA256.swift` | 126 | 已写入（未验证） | `Tools/Support/rust/sha256.rs` | 未编译、未行为对照 |
| `Tools/Support/Sources/ToolSupport/Shell.swift` | 70 | 已写入（未验证） | `Tools/Support/rust/shell.rs` | 未编译、未行为对照 |
| `Tools/Support/Sources/ToolSupport/Terminal.swift` | 39 | 已写入（未验证） | `Tools/Support/rust/terminal.rs` | 未编译、未行为对照 |
| `Tools/Support/Sources/ToolSupport/WindowsBuildEnvironment.swift` | 67 | 已写入（未验证） | `Tools/Support/rust/windows_build_environment.rs` | 未编译、未行为对照 |

### Examples

共 61 个文件：已写入 0，进行中 0，未开始 61。

| 源文件 | 行 | 状态 | Rust/C 目标 | 依赖或阻塞 |
| --- | ---: | --- | --- | --- |
| `Examples/Counter/Sources/CounterExample/CounterView.swift` | 200 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Mobile/Platforms/iOS/main.swift` | 4 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Mobile/Sources/AppContent/App.swift` | 33 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Mobile/Sources/AppContent/Model.swift` | 30 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Mobile/Sources/AppContent/Projects.swift` | 57 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Mobile/Sources/AppContent/Settings.swift` | 50 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Mobile/Sources/AppContent/Today.swift` | 49 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Platforms/iOS/main.swift` | 10 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/CoveUIDemo/Launch.swift` | 79 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/CoveUIDemo/main.swift` | 7 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/CounterContent.swift` | 18 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/GalleryCatalog.swift` | 150 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/GalleryView.swift` | 218 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/NavigationIcons.swift` | 105 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Controls/ControlStories.swift` | 380 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Controls/StepperStory.swift` | 36 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Controls/TextEditorStory.swift` | 327 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Controls/TimePickerStory.swift` | 95 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Controls/ToolbarStory.swift` | 71 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Controls/WheelPickerStory.swift` | 104 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Data/AdaptiveTables.swift` | 19 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Data/ChartExploration.swift` | 37 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Data/DataStories.swift` | 714 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Data/DataTableStory.swift` | 131 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Data/DateTableStories.swift` | 114 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Data/MarkdownStory.swift` | 77 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Data/RichTextStory.swift` | 58 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Data/WebStory.swift` | 193 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Feedback/AvatarStory.swift` | 86 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Feedback/FeedbackStories.swift` | 250 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Feedback/RatingSample.swift` | 48 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Feedback/RatingStory.swift` | 77 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Feedback/SkeletonStory.swift` | 43 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Foundation/AdaptiveExamples.swift` | 551 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Foundation/AnimationCurves.swift` | 51 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Foundation/AnimationSystem.swift` | 81 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Foundation/IntrinsicFitting.swift` | 35 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Foundation/SurfaceTraits.swift` | 70 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Foundation/ThemeStory.swift` | 89 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Layout/DrawerStory.swift` | 56 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Layout/FoundationExamples.swift` | 134 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Layout/GridColumns.swift` | 53 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Layout/ImageStory.swift` | 80 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Layout/LayoutStories.swift` | 396 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Layout/MasonryStory.swift` | 81 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Layout/NavigationStories.swift` | 291 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Layout/NestedSidebarExample.swift` | 56 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Layout/OverlayStories.swift` | 199 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Layout/ProductStories.swift` | 152 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Layout/RailStory.swift` | 84 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Layout/ResizableSidebarExample.swift` | 31 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Layout/SidebarStory.swift` | 118 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Navigation/AppBarStory.swift` | 53 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Overlays/ActionSheetStory.swift` | 75 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Overlays/CommandStory.swift` | 67 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Overlays/HoverStory.swift` | 50 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Overlays/Popconfirm/PlacementExample.swift` | 73 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Overlays/PopconfirmStory.swift` | 74 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Overview/Dashboard.swift` | 105 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/GalleryContent/Stories/Overview/MetricCard.swift` | 49 | 未开始 | — | 依赖全部下层 crate |
| `Examples/Sources/PetDemo/main.swift` | 111 | 未开始 | — | 依赖全部下层 crate |

## 测试源码逐文件映射

共 424 个文件，均为未开始。迁移顺序在这些文件依赖的生产 crate 完成之后。

### Core

| 源文件 | 行 | 状态 | Rust/C 目标 | 依赖或阻塞 |
| --- | ---: | --- | --- | --- |
| `Core/Tests/CoveUICoreTests/AdaptiveEnvironmentTests.swift` | 42 | 未开始 | — | 全部生产实现完成后迁移 |
| `Core/Tests/CoveUICoreTests/DrawListResourceTests.swift` | 42 | 未开始 | — | 全部生产实现完成后迁移 |
| `Core/Tests/CoveUICoreTests/GeometryKernelTests.swift` | 59 | 未开始 | — | 全部生产实现完成后迁移 |
| `Core/Tests/CoveUICoreTests/MipmapTests.swift` | 15 | 未开始 | — | 全部生产实现完成后迁移 |
| `Core/Tests/CoveUICoreTests/PortableCoreTests.swift` | 41 | 未开始 | — | 全部生产实现完成后迁移 |
| `Core/Tests/CoveUICoreTests/TextScaleTests.swift` | 32 | 未开始 | — | 全部生产实现完成后迁移 |

### PlatformCore

| 源文件 | 行 | 状态 | Rust/C 目标 | 依赖或阻塞 |
| --- | ---: | --- | --- | --- |
| `PlatformCore/Tests/CovePlatformCoreTests/ScrollInputTests.swift` | 31 | 未开始 | — | 全部生产实现完成后迁移 |

### System

| 源文件 | 行 | 状态 | Rust/C 目标 | 依赖或阻塞 |
| --- | ---: | --- | --- | --- |
| `System/Tests/CoveFileSystemTests/FileLockTests.swift` | 24 | 未开始 | — | 全部生产实现完成后迁移 |
| `System/Tests/CoveFileSystemTests/Files.swift` | 91 | 未开始 | — | 全部生产实现完成后迁移 |
| `System/Tests/CoveFileSystemTests/Paths.swift` | 49 | 未开始 | — | 全部生产实现完成后迁移 |
| `System/Tests/CoveProcessTests/BufferTests.swift` | 39 | 未开始 | — | 全部生产实现完成后迁移 |
| `System/Tests/CoveProcessTests/Contracts.swift` | 115 | 未开始 | — | 全部生产实现完成后迁移 |
| `System/Tests/CoveProcessTests/ProcessTests.swift` | 104 | 未开始 | — | 全部生产实现完成后迁移 |

### Logging

| 源文件 | 行 | 状态 | Rust/C 目标 | 依赖或阻塞 |
| --- | ---: | --- | --- | --- |
| `Logging/Tests/CoveLoggingTests/PipelineTests.swift` | 113 | 未开始 | — | 全部生产实现完成后迁移 |
| `Logging/Tests/CoveLoggingTests/StartupTests.swift` | 22 | 未开始 | — | 全部生产实现完成后迁移 |
| `Logging/Tests/CoveLoggingTests/StorageTests.swift` | 53 | 未开始 | — | 全部生产实现完成后迁移 |

### Text

| 源文件 | 行 | 状态 | Rust/C 目标 | 依赖或阻塞 |
| --- | ---: | --- | --- | --- |
| `Text/Tests/CoveUITextTests/ColorGlyphAtlasTests.swift` | 74 | 未开始 | — | 全部生产实现完成后迁移 |
| `Text/Tests/CoveUITextTests/FontPipelineTests.swift` | 137 | 未开始 | — | 全部生产实现完成后迁移 |

### Graphics

| 源文件 | 行 | 状态 | Rust/C 目标 | 依赖或阻塞 |
| --- | ---: | --- | --- | --- |
| `Graphics/Tests/NativeRHITests/AccessTrackerTests.swift` | 182 | 未开始 | — | 全部生产实现完成后迁移 |
| `Graphics/Tests/NativeRHITests/CacheTests.swift` | 84 | 未开始 | — | 全部生产实现完成后迁移 |
| `Graphics/Tests/NativeRHITests/ChunkUploadAllocatorTests.swift` | 97 | 未开始 | — | 全部生产实现完成后迁移 |
| `Graphics/Tests/NativeRHITests/FrameRingTests.swift` | 116 | 未开始 | — | 全部生产实现完成后迁移 |
| `Graphics/Tests/NativeRHITests/GPUTimestampProfileTests.swift` | 63 | 未开始 | — | 全部生产实现完成后迁移 |
| `Graphics/Tests/NativeRHITests/LRUMapTests.swift` | 82 | 未开始 | — | 全部生产实现完成后迁移 |
| `Graphics/Tests/NativeRHITests/MacSurfaceTests.swift` | 144 | 未开始 | — | 全部生产实现完成后迁移 |
| `Graphics/Tests/NativeRHITests/Metal/MetalBindingLifetimeTests.swift` | 107 | 未开始 | — | 全部生产实现完成后迁移 |
| `Graphics/Tests/NativeRHITests/Metal/MetalCapabilityProbeTests.swift` | 63 | 未开始 | — | 全部生产实现完成后迁移 |
| `Graphics/Tests/NativeRHITests/Metal/MetalClearReadbackTests.swift` | 63 | 未开始 | — | 全部生产实现完成后迁移 |
| `Graphics/Tests/NativeRHITests/Metal/MetalFrameLifecycleTests.swift` | 88 | 未开始 | — | 全部生产实现完成后迁移 |
| `Graphics/Tests/NativeRHITests/Metal/MetalRayQueryTests.swift` | 76 | 未开始 | — | 全部生产实现完成后迁移 |
| `Graphics/Tests/NativeRHITests/Metal/MetalRenderBindingTests.swift` | 108 | 未开始 | — | 全部生产实现完成后迁移 |
| `Graphics/Tests/NativeRHITests/Metal/MetalSlangTests.swift` | 72 | 未开始 | — | 全部生产实现完成后迁移 |
| `Graphics/Tests/NativeRHITests/Metal/MetalStorageHazardTests.swift` | 85 | 未开始 | — | 全部生产实现完成后迁移 |
| `Graphics/Tests/NativeRHITests/NativeGPUContractTests.swift` | 223 | 未开始 | — | 全部生产实现完成后迁移 |
| `Graphics/Tests/NativeRHITests/NativeMultisampleTests.swift` | 210 | 未开始 | — | 全部生产实现完成后迁移 |
| `Graphics/Tests/NativeRHITests/NativeRHISmokeTests.swift` | 38 | 未开始 | — | 全部生产实现完成后迁移 |
| `Graphics/Tests/NativeRHITests/NativeRecordedTextureUploadTests.swift` | 83 | 未开始 | — | 全部生产实现完成后迁移 |
| `Graphics/Tests/NativeRHITests/NativeTestDevice.swift` | 14 | 未开始 | — | 全部生产实现完成后迁移 |
| `Graphics/Tests/NativeRHITests/NativeTextureCopyTests.swift` | 46 | 未开始 | — | 全部生产实现完成后迁移 |
| `Graphics/Tests/NativeRHITests/NativeTextureRegionTests.swift` | 82 | 未开始 | — | 全部生产实现完成后迁移 |
| `Graphics/Tests/NativeRHITests/PipelineCacheTests.swift` | 264 | 未开始 | — | 全部生产实现完成后迁移 |
| `Graphics/Tests/NativeRHITests/ReflectedBindingLayoutTests.swift` | 38 | 未开始 | — | 全部生产实现完成后迁移 |
| `Graphics/Tests/NativeRHITests/RenderAttachmentValidationTests.swift` | 77 | 未开始 | — | 全部生产实现完成后迁移 |
| `Graphics/Tests/NativeRHITests/ResourcePreparationTests.swift` | 153 | 未开始 | — | 全部生产实现完成后迁移 |
| `Graphics/Tests/NativeRHITests/ShaderArtifactTests.swift` | 29 | 未开始 | — | 全部生产实现完成后迁移 |
| `Graphics/Tests/NativeRHITests/ShaderFixtureCompiler.swift` | 90 | 未开始 | — | 全部生产实现完成后迁移 |
| `Graphics/Tests/NativeRHITests/ShaderSpecializationTests.swift` | 60 | 未开始 | — | 全部生产实现完成后迁移 |
| `Graphics/Tests/NativeRHITests/StateTrackerTests.swift` | 76 | 未开始 | — | 全部生产实现完成后迁移 |
| `Graphics/Tests/NativeRHITests/SubmissionPlannerTests.swift` | 414 | 未开始 | — | 全部生产实现完成后迁移 |
| `Graphics/Tests/NativeRHITests/SwapchainLifetimeTests.swift` | 223 | 未开始 | — | 全部生产实现完成后迁移 |
| `Graphics/Tests/NativeRHITests/Vulkan/PresentationPolicyTests.swift` | 36 | 未开始 | — | 全部生产实现完成后迁移 |
| `Graphics/Tests/NativeRHITests/Vulkan/VulkanCapabilityProbeTests.swift` | 51 | 未开始 | — | 全部生产实现完成后迁移 |
| `Graphics/Tests/NativeRHITests/Vulkan/VulkanSlangTests.swift` | 80 | 未开始 | — | 全部生产实现完成后迁移 |
| `Graphics/Tests/NativeRHITests/Vulkan/VulkanUploadReadbackTests.swift` | 86 | 未开始 | — | 全部生产实现完成后迁移 |

### Terminal

| 源文件 | 行 | 状态 | Rust/C 目标 | 依赖或阻塞 |
| --- | ---: | --- | --- | --- |
| `Terminal/Tests/CoveUITerminalTests/EngineTests.swift` | 151 | 未开始 | — | 全部生产实现完成后迁移 |
| `Terminal/Tests/CoveUITerminalTests/InteractionTests.swift` | 104 | 未开始 | — | 全部生产实现完成后迁移 |
| `Terminal/Tests/CoveUITerminalTests/KeyRoutingTests.swift` | 44 | 未开始 | — | 全部生产实现完成后迁移 |
| `Terminal/Tests/CoveUITerminalTests/RenderingFeatureTests.swift` | 118 | 未开始 | — | 全部生产实现完成后迁移 |
| `Terminal/Tests/CoveUITerminalTests/SessionTests.swift` | 106 | 未开始 | — | 全部生产实现完成后迁移 |
| `Terminal/Tests/CoveUITerminalTests/ViewTests.swift` | 287 | 未开始 | — | 全部生产实现完成后迁移 |
| `Terminal/Tests/CoveUITerminalTests/WindowsCommandLineTests.swift` | 25 | 未开始 | — | 全部生产实现完成后迁移 |
| `Terminal/Tests/CoveUITerminalTests/WindowsSessionTests.swift` | 52 | 未开始 | — | 全部生产实现完成后迁移 |

### Hosts

| 源文件 | 行 | 状态 | Rust/C 目标 | 依赖或阻塞 |
| --- | ---: | --- | --- | --- |
| `Hosts/Android/Contract/Tests/CoveAndroidContractTests/FrameClockTests.swift` | 40 | 未开始 | — | 全部生产实现完成后迁移 |
| `Hosts/Android/Contract/Tests/CoveAndroidContractTests/LaunchTests.swift` | 13 | 未开始 | — | 全部生产实现完成后迁移 |

### DevTools

| 源文件 | 行 | 状态 | Rust/C 目标 | 依赖或阻塞 |
| --- | ---: | --- | --- | --- |
| `DevTools/Inspection/Tests/CoveUIAgentCoreTests/AgentTests.swift` | 93 | 未开始 | — | 全部生产实现完成后迁移 |
| `DevTools/Tests/CoveUIAgentTransportTests/TransportTests.swift` | 26 | 未开始 | — | 全部生产实现完成后迁移 |
| `DevTools/Tests/CoveUIDevToolsServerTests/DevServerTests.swift` | 553 | 未开始 | — | 全部生产实现完成后迁移 |

### Tools

| 源文件 | 行 | 状态 | Rust/C 目标 | 依赖或阻塞 |
| --- | ---: | --- | --- | --- |
| `Tools/Cove/Tests/CoveCLITests/ArgumentsTests.swift` | 96 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tools/Cove/Tests/CoveCLITests/CLIUpdateTests.swift` | 81 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tools/Cove/Tests/CoveCLITests/DesktopTests.swift` | 38 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tools/Cove/Tests/CoveCLITests/DeveloperToolTests.swift` | 50 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tools/Cove/Tests/CoveCLITests/DevelopmentTests.swift` | 49 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tools/Cove/Tests/CoveCLITests/Helpers.swift` | 17 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tools/Cove/Tests/CoveCLITests/IOSTests.swift` | 27 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tools/Cove/Tests/CoveCLITests/InstallationTests.swift` | 147 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tools/Cove/Tests/CoveCLITests/PackageTests.swift` | 76 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tools/Cove/Tests/CoveCLITests/ProcessTests.swift` | 43 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tools/Cove/Tests/CoveCLITests/ProjectTaskTests.swift` | 53 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tools/Cove/Tests/CoveCLITests/ProjectTests.swift` | 72 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tools/Cove/Tests/CoveCLITests/SDKPayloadTests.swift` | 66 | 未开始 | — | 全部生产实现完成后迁移 |

### Examples

| 源文件 | 行 | 状态 | Rust/C 目标 | 依赖或阻塞 |
| --- | ---: | --- | --- | --- |
| `Examples/Counter/Tests/CounterExampleTests/ComponentAnalysisTests.swift` | 236 | 未开始 | — | 全部生产实现完成后迁移 |
| `Examples/Counter/Tests/CounterExampleTests/InspectionTests.swift` | 184 | 未开始 | — | 全部生产实现完成后迁移 |
| `Examples/Counter/Tests/CounterExampleTests/SharedSceneTests.swift` | 117 | 未开始 | — | 全部生产实现完成后迁移 |
| `Examples/Mobile/Tests/AppContentTests/Model.swift` | 6 | 未开始 | — | 全部生产实现完成后迁移 |
| `Examples/Tests/GalleryContentTests/AdaptiveShellTests.swift` | 190 | 未开始 | — | 全部生产实现完成后迁移 |
| `Examples/Tests/GalleryContentTests/AnimationSystemTests.swift` | 165 | 未开始 | — | 全部生产实现完成后迁移 |
| `Examples/Tests/GalleryContentTests/AnimationTests.swift` | 105 | 未开始 | — | 全部生产实现完成后迁移 |
| `Examples/Tests/GalleryContentTests/AppBarTests.swift` | 48 | 未开始 | — | 全部生产实现完成后迁移 |
| `Examples/Tests/GalleryContentTests/AppearanceTests.swift` | 52 | 未开始 | — | 全部生产实现完成后迁移 |
| `Examples/Tests/GalleryContentTests/DrawerTests.swift` | 105 | 未开始 | — | 全部生产实现完成后迁移 |
| `Examples/Tests/GalleryContentTests/GalleryTests.swift` | 206 | 未开始 | — | 全部生产实现完成后迁移 |
| `Examples/Tests/GalleryContentTests/ImageLayoutTests.swift` | 62 | 未开始 | — | 全部生产实现完成后迁移 |
| `Examples/Tests/GalleryContentTests/ListRowLayoutTests.swift` | 67 | 未开始 | — | 全部生产实现完成后迁移 |
| `Examples/Tests/GalleryContentTests/NavigationStateTests.swift` | 143 | 未开始 | — | 全部生产实现完成后迁移 |
| `Examples/Tests/GalleryContentTests/RailTests.swift` | 80 | 未开始 | — | 全部生产实现完成后迁移 |
| `Examples/Tests/GalleryContentTests/RatingLayoutTests.swift` | 58 | 未开始 | — | 全部生产实现完成后迁移 |
| `Examples/Tests/GalleryContentTests/SelectionShellTests.swift` | 421 | 未开始 | — | 全部生产实现完成后迁移 |
| `Examples/Tests/GalleryContentTests/SplitTests.swift` | 80 | 未开始 | — | 全部生产实现完成后迁移 |
| `Examples/Tests/GalleryContentTests/SurfaceTraitsTests.swift` | 31 | 未开始 | — | 全部生产实现完成后迁移 |
| `Examples/Tests/GalleryContentTests/TextEditorLayoutTests.swift` | 66 | 未开始 | — | 全部生产实现完成后迁移 |

### Tests

| 源文件 | 行 | 状态 | Rust/C 目标 | 依赖或阻塞 |
| --- | ---: | --- | --- | --- |
| `Tests/CoveUIAndroidTests/SessionTests.swift` | 51 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIAndroidTests/SurfaceMetricsTests.swift` | 98 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/AccessibilitySemanticsTests.swift` | 130 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ActionSheetTests.swift` | 265 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/Adaptive/AdaptiveSidebarTests.swift` | 574 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/Adaptive/AlertTests.swift` | 117 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/Adaptive/AppBarTests.swift` | 270 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/Adaptive/AssetFieldTests.swift` | 88 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/Adaptive/CommandPaletteAdaptiveTests.swift` | 92 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/Adaptive/CompactLabelTests.swift` | 75 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/Adaptive/DialogTests.swift` | 123 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/Adaptive/DisclosureTests.swift` | 60 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/Adaptive/EditorHeightTests.swift` | 196 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/Adaptive/FieldLayoutTests.swift` | 222 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/Adaptive/GridColumnsTests.swift` | 270 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/Adaptive/JsonEditorTests.swift` | 132 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/Adaptive/NarrowReflowTests.swift` | 139 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/Adaptive/NavigationDrawerTests.swift` | 394 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/Adaptive/NavigationRailTests.swift` | 360 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/Adaptive/RowDensityTests.swift` | 233 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/Adaptive/SelectionFieldTests.swift` | 199 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/Adaptive/SidebarHierarchyTests.swift` | 300 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/Adaptive/SidebarResizeTests.swift` | 251 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/Adaptive/SplitPresentationTests.swift` | 347 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/Adaptive/SurfaceRootTests.swift` | 59 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/Adaptive/TabOverflowTests.swift` | 232 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/Adaptive/TableColumnsTests.swift` | 285 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/Adaptive/TextScaleRows.swift` | 201 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/Adaptive/TimelineTests.swift` | 145 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/Adaptive/ToolbarOverflowTests.swift` | 195 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/AdaptiveEnvironmentTests.swift` | 41 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/AdaptiveNavigationTests.swift` | 52 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/AdvancedInteractionTests.swift` | 263 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/Agent/ControlDiscoveryTests.swift` | 78 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/AnimatedVisibilityTests.swift` | 157 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/Animation/AppearanceRevealTests.swift` | 97 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/Animation/InteractionTests.swift` | 117 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/Animation/SequenceLifecycleTests.swift` | 58 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/AnimationIntegrationTests.swift` | 719 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/AnimationValueModifierTests.swift` | 272 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/AppStorageTests.swift` | 212 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/AppearanceTests.swift` | 60 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/AssetRefFieldTests.swift` | 90 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/AsyncImageTests.swift` | 247 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/AvatarPresenceTests.swift` | 76 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/AvatarTests.swift` | 93 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/BidiTests.swift` | 100 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/BoolControlLabelTests.swift` | 44 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/BottomSheetTests.swift` | 139 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/BracketMatchingTests.swift` | 44 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ButtonIconTests.swift` | 302 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ButtonPressRecomposeTests.swift` | 219 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ButtonScrollViewTests.swift` | 1054 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ButtonStyleAnimationTests.swift` | 113 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ButtonStyleTests.swift` | 358 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/Calendar/AdaptiveTests.swift` | 57 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/Calendar/KeyboardNavigationTests.swift` | 225 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/Calendar/ModesTests.swift` | 159 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/CarouselTests.swift` | 252 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/CatalogInteractionTests.swift` | 197 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ChartAccessibilityTests.swift` | 242 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ChartAxisTests.swift` | 242 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ChartInteractionTests.swift` | 237 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ChartOrientationTests.swift` | 256 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ChartPerformanceTests.swift` | 148 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ChartTests.swift` | 463 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ChartViewportTests.swift` | 175 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ChromeComponentTests.swift` | 58 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ChromeRegressionTests.swift` | 72 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/CodeEditing/GutterTests.swift` | 325 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/CodeEditingUITests.swift` | 231 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ColorGlyphResampleTests.swift` | 24 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ComboboxSearchTests.swift` | 129 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/CommandPaletteFooterTests.swift` | 83 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/CompositionLocalTests.swift` | 102 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ContainerQueryTests.swift` | 174 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ControlDensityTests.swift` | 74 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ControlMotionTests.swift` | 72 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/CursorPipelineTests.swift` | 76 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/CustomLayoutTests.swift` | 170 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/CustomLayoutVisibilityTests.swift` | 76 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/DataTableInteractionTests.swift` | 132 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/DataTableLayoutTests.swift` | 161 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/DataTableModelTests.swift` | 89 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/DataTableScrollTests.swift` | 94 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/DateRangePickerTests.swift` | 96 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/DateRangeRulesTests.swift` | 91 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/DemoCrossFadeTests.swift` | 66 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/DesignSystemTests.swift` | 414 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/DisclosureContainerTests.swift` | 127 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/DisclosureGroupTests.swift` | 105 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/DragAndDropTests.swift` | 149 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/EdgeInsetsInterpolatableTests.swift` | 26 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/EditorInteractionTests.swift` | 432 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/FeedbackComponentTests.swift` | 121 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/FloatingSurfaceTests.swift` | 161 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/FocusRingTests.swift` | 122 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/FourTreeConsistencyTests.swift` | 118 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/GridLayoutTests.swift` | 111 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/GridSpanTests.swift` | 74 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ImageFileContentModeTests.swift` | 165 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ImagePipelineTests.swift` | 138 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/Images/HTTPServer.swift` | 88 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/InteractiveScene.swift` | 100 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/JsonFieldTests.swift` | 141 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/KeyboardShortcutTests.swift` | 17 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/KeyedReconcilerTests.swift` | 246 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/LabelSelectionTests.swift` | 348 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/LazyGridTests.swift` | 129 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ListMultipleSelectionTests.swift` | 112 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ListRowTests.swift` | 241 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ListTreeTests.swift` | 896 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/MarkdownTests.swift` | 266 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/MasonryTests.swift` | 84 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/MenuButtonTests.swift` | 113 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/MenuContentTests.swift` | 89 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/MenuHierarchyTests.swift` | 337 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/MenuPointerRoutingTests.swift` | 137 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/Mobile/Fixtures.swift` | 119 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/Mobile/PresentationTests.swift` | 273 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ModifierReplayTests.swift` | 115 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/NativeMenuTests.swift` | 158 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/NavigationStackTests.swift` | 163 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/NumberFieldTests.swift` | 114 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/NumericLocaleTests.swift` | 85 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ObservableStateTrackingTests.swift` | 304 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/OffscreenReuseTests.swift` | 30 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/OverlayScrollInputTests.swift` | 76 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/OverlayTests.swift` | 93 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/PackageResourceBundleTests.swift` | 15 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/PopconfirmTests.swift` | 247 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/PopoverTests.swift` | 686 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/PortalLayoutTests.swift` | 99 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/PortalScopeTests.swift` | 88 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/PrimitiveLayoutTests.swift` | 286 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ProductComponentTests.swift` | 390 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ProductInteractionRegressionTests.swift` | 397 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ProgressAnimationTests.swift` | 47 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/PropertyGridSizingTests.swift` | 99 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/RangeSliderTests.swift` | 189 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/RatingTests.swift` | 299 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/RecompositionPaintTests.swift` | 49 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/RefreshTests.swift` | 154 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/RenderTreeMirrorTests.swift` | 258 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ReorderableListTests.swift` | 263 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ResponsiveLayoutTests.swift` | 187 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/RetainedVisibilityTests.swift` | 67 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/RichTextTests.swift` | 282 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/RouteAndPreloadTests.swift` | 167 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/RowSlotTests.swift` | 58 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ScratchGeometryTests.swift` | 48 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ScrollOffsetBindingTests.swift` | 151 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ScrollViewInputTests.swift` | 167 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ScrollbarTests.swift` | 281 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ScrolledControlClickTests.swift` | 507 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/Scrolling/IndicatorTests.swift` | 88 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/Scrolling/KeyboardFocusTests.swift` | 63 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/Scrolling/NestedScrollRoutingTests.swift` | 146 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/Scrolling/ScrollPhysicsTests.swift` | 454 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/SearchFieldTests.swift` | 95 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/SectionedListTests.swift` | 211 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/SelectEnumerationTests.swift` | 18 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/SemanticMotionTests.swift` | 96 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/SheetTests.swift` | 82 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ShortcutHostTests.swift` | 59 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/SidebarContentTests.swift` | 212 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/SidebarSlotTests.swift` | 62 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/SidebarTests.swift` | 87 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/SingleLineTextTests.swift` | 92 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/SliderAxisTests.swift` | 111 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/SliderTests.swift` | 300 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/SnippetTests.swift` | 109 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/SplitButtonTests.swift` | 146 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/SplitViewPanelTests.swift` | 137 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/SpringInterruptionTests.swift` | 145 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/StatePropagationRegressionTests.swift` | 148 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/StatusBarPanelTruncationTests.swift` | 101 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/StyleIntegrationTests.swift` | 210 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/StyleSkeletonTests.swift` | 102 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/SwiftObservationTests.swift` | 92 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/SwipeListTests.swift` | 236 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/TabViewTests.swift` | 122 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/TestSupport.swift` | 63 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/TextBufferTests.swift` | 194 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/TextCacheTests.swift` | 53 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/TextDividerTests.swift` | 204 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/TextDocumentLayoutTests.swift` | 77 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/TextEditorAppearanceTests.swift` | 77 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/TextFieldLargeDocumentTests.swift` | 122 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/TextFieldNavigationTests.swift` | 187 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/TextFieldTests.swift` | 1014 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/TextIndentGuidesTests.swift` | 50 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/TextMeasureSlotTests.swift` | 52 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/TextSelectionGeometryTests.swift` | 74 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/TextSelectionScopeTests.swift` | 184 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/TextShapeCacheTests.swift` | 136 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/TextVisualLineIndexTests.swift` | 51 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ThemeCrossFadeComponentTests.swift` | 213 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ThemeEnvironmentTests.swift` | 108 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ThemeInteractionRegressionTests.swift` | 254 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ThemePaintInvalidationTests.swift` | 54 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ThemeTests.swift` | 72 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/TimeInputTests.swift` | 125 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/TimePickerTests.swift` | 85 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/TimelineGroupTests.swift` | 86 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ToastAnimationTests.swift` | 96 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ToggleTests.swift` | 209 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/TooltipDeduplicationTests.swift` | 109 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/TouchDensityTests.swift` | 171 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/Transitions/ScaleTests.swift` | 245 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/Transitions/SurfaceRevealTests.swift` | 120 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/Tree/BehaviorTests.swift` | 148 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/TruncationTests.swift` | 79 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/TypographyRasterTests.swift` | 98 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ValueSubtreeTests.swift` | 47 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/Vec3FieldTests.swift` | 129 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/VerticalToolbarTests.swift` | 58 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ViewBuilderTests.swift` | 74 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ViewGraphTests.swift` | 351 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ViewThatFitsTests.swift` | 137 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ViewportTraitsTests.swift` | 102 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/VirtualStackVariableRowsTests.swift` | 217 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/VirtualizationEstimateTests.swift` | 59 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/WheelPickerTests.swift` | 169 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/WithAnimationCompletionTests.swift` | 169 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/WithAnimationOpacityTests.swift` | 150 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/WithAnimationPropertiesTests.swift` | 465 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIComposeTests/ZStackTests.swift` | 132 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIDevToolsTests/DevServerInputAndStateTests.swift` | 286 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIDevToolsTests/DevServerSmokeTests.swift` | 209 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIDevToolsTests/MirrorBackdropTests.swift` | 47 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIDevToolsTests/TimingPublisherTests.swift` | 15 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/AccessibilityGeometryTests.swift` | 36 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/AnimationControllerTests.swift` | 220 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/AnimationValueTests.swift` | 184 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/AppKitScrollInputTests.swift` | 125 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/ColorGlyphTests.swift` | 90 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/ContainerOpacityTests.swift` | 38 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/DisplayRefreshRateTests.swift` | 23 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/DrawBatchCoalescingTests.swift` | 68 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/DrawListTests.swift` | 287 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/EventDispatcherCursorTests.swift` | 98 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/EventDispatcherTests.swift` | 496 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/FallbackStyleTests.swift` | 49 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/FocusChainTests.swift` | 149 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/FrameDeadlineTests.swift` | 85 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/GeometryFunnelReproTests.swift` | 79 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/HitTesterTests.swift` | 112 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/ImageAssetRegistryTests.swift` | 57 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/ImageDecoderTests.swift` | 141 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/InterpolatableTests.swift` | 93 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/LayerAwareNodeRendererTests.swift` | 544 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/LayoutTests.swift` | 334 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/LinearGradientTests.swift` | 49 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/MultiWindowSmokeTests.swift` | 1048 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/NativeAccessibilityTests.swift` | 162 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/NativeAnalyticClipTests.swift` | 33 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/NativeDisplayRefreshTests.swift` | 71 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/NativeDrawListBenchmarkTests.swift` | 306 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/NativeDrawListLifecycleTests.swift` | 269 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/NativeDrawListParityTests.swift` | 62 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/NativeImageAssetTests.swift` | 106 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/NativeRoundedFillTests.swift` | 36 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/NativeShaderLibraryTests.swift` | 60 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/NativeUIBackdropTests.swift` | 337 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/NativeUIBindingReuseTests.swift` | 62 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/NativeUIDrawTestSupport.swift` | 165 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/NodeLifecycleCleanupTests.swift` | 107 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/NodeRendererTests.swift` | 227 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/NodeResourceTests.swift` | 90 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/NodeTransformRenderTests.swift` | 247 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/NodeTransformTests.swift` | 173 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/NodeTreeTests.swift` | 159 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/ProgressiveImageTests.swift` | 76 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/RecomposerTests.swift` | 182 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/Rendering/BackdropOpacityTests.swift` | 61 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/Rendering/CircularRevealPixels.swift` | 72 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/Rendering/CompositionPaintTests.swift` | 39 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/Rendering/EffectReplayTests.swift` | 68 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/Rendering/MSAAEvaluationTests.swift` | 184 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/Rendering/RevealRequestTests.swift` | 88 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/Rendering/TransientMSAATests.swift` | 90 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/RoundedClipTests.swift` | 212 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/ShadowQuantizationTests.swift` | 214 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/SurfacePaintingTests.swift` | 111 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/SurfaceRenderingTests.swift` | 56 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/SystemFontInstanceTests.swift` | 77 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/TextLayoutRegressionTests.swift` | 66 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/TextTests.swift` | 403 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/Transforms/HitTestingTests.swift` | 200 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/Transforms/PaintTests.swift` | 47 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/Transforms/ShadowTests.swift` | 112 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/WheelInputTests.swift` | 125 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIRuntimeTests/ZIndexTests.swift` | 34 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUITests/AppDisplayHandleTests.swift` | 38 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUITests/ApplicationTests.swift` | 101 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUITests/NativeWindowLifecycleTests.swift` | 72 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUITests/PackageResourceBundleTests.swift` | 13 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUITests/WindowChromeInteractionTests.swift` | 159 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIWebViewTests/WebViewGeometryTests.swift` | 115 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIWebViewTests/WebViewLifecycleTests.swift` | 128 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIWorkspaceTests/PackageResourceBundleTests.swift` | 13 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIWorkspaceTests/PanelRegistryTests.swift` | 69 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIWorkspaceTests/PanelSizingTests.swift` | 85 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIWorkspaceTests/PanelWorkspaceIntegrationTests.swift` | 46 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIWorkspaceTests/WorkspaceControllerTests.swift` | 473 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/CoveUIWorkspaceTests/WorkspaceViewTests.swift` | 1177 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/Headless/Tests/CoveUISceneTests/Animation/CurveTests.swift` | 141 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/Headless/Tests/CoveUISceneTests/Animation/PlaybackTests.swift` | 155 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/Headless/Tests/CoveUISceneTests/DiagnosticPresentationTests.swift` | 184 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/Headless/Tests/CoveUISceneTests/DiagnosticsTests.swift` | 337 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/Headless/Tests/CoveUISceneTests/FlatRenderPlanTests.swift` | 157 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/Headless/Tests/CoveUISceneTests/Input/CaptureCancellationTests.swift` | 50 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/Headless/Tests/CoveUISceneTests/Input/FocusAccessoryTests.swift` | 58 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/Headless/Tests/CoveUISceneTests/Layout/MeasurementLifecycleTests.swift` | 70 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/Headless/Tests/CoveUISceneTests/Layout/MinimumSizeTests.swift` | 41 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/Headless/Tests/CoveUISceneTests/ObservationTests.swift` | 103 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/Headless/Tests/CoveUISceneTests/RenderPipelinePerformanceTests.swift` | 43 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/Headless/Tests/CoveUISceneTests/Scrolling/RubberBandTests.swift` | 67 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/Headless/Tests/CoveUISceneTests/Scrolling/ScrollAnimatorTests.swift` | 603 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/Headless/Tests/CoveUISceneTests/Scrolling/ScrollSpringTests.swift` | 92 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/Headless/Tests/CoveUISceneTests/Scrolling/ScrollVelocityTrackerTests.swift` | 107 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/Headless/Tests/CoveUISceneTests/StateFieldReadTests.swift` | 595 | 未开始 | — | 全部生产实现完成后迁移 |
| `Tests/Headless/Tests/CoveUISceneTests/StructuralModifierTests.swift` | 138 | 未开始 | — | 全部生产实现完成后迁移 |

## SwiftPM 清单与模板

### SwiftPM 清单

| 源文件 | 行 | 状态 | Rust/C 目标 | 说明 |
| --- | ---: | --- | --- | --- |
| `Browser/Package.swift` | 19 | Cargo 清单已建立 | `Browser/Cargo.toml` | 对应 Cargo 清单未建立 |
| `Compose/Composition/Package.swift` | 18 | Cargo 清单已建立 | `Compose/Composition/Cargo.toml` | 对应 Cargo 清单未建立 |
| `Compose/Package.swift` | 18 | Cargo 清单已建立 | `Compose/Cargo.toml` | 对应 Cargo 清单未建立 |
| `Core/Package.swift` | 18 | Cargo 清单已建立 | `Core/Cargo.toml` | 对应 Cargo 清单未建立 |
| `DevTools/Inspection/Package.swift` | 14 | 未开始 | `DevTools/Inspection/Cargo.toml`<br>（计划） | 对应 Cargo 清单未建立 |
| `DevTools/Package.swift` | 31 | Cargo 清单已建立 | `DevTools/Cargo.toml` | 对应 Cargo 清单未建立 |
| `DevTools/Protocol/Package.swift` | 10 | Cargo 清单已建立 | `DevTools/Protocol/Cargo.toml` | 对应 Cargo 清单未建立 |
| `Examples/Counter/Package.swift` | 20 | 未开始 | `Examples/Counter/Cargo.toml`<br>（计划） | 对应 Cargo 清单未建立 |
| `Examples/Mobile/Package.swift` | 73 | 未开始 | `Examples/Mobile/Cargo.toml`<br>（计划） | 对应 Cargo 清单未建立 |
| `Examples/Package.swift` | 73 | Cargo 清单已建立 | `Examples/Cargo.toml` | 对应 Cargo 清单未建立 |
| `Graphics/Package.swift` | 28 | Cargo 清单已建立 | `Graphics/Cargo.toml` | 对应 Cargo 清单未建立 |
| `Hosts/Android/Contract/Package.swift` | 12 | 未开始 | `Hosts/Android/Contract/Cargo.toml`<br>（计划） | 对应 Cargo 清单未建立 |
| `Hosts/Android/Package.swift` | 30 | 未开始 | `Hosts/Android/Cargo.toml`<br>（计划） | 对应 Cargo 清单未建立 |
| `Imaging/Package.swift` | 22 | Cargo 清单已建立 | `Imaging/Cargo.toml` | 对应 Cargo 清单未建立 |
| `Logging/Package.swift` | 13 | Cargo 清单已建立 | `Logging/Cargo.toml` | 对应 Cargo 清单未建立 |
| `Package.swift` | 92 | Cargo 清单已建立 | `Cargo.toml` | 对应 Cargo 清单未建立 |
| `PlatformCore/Package.swift` | 10 | Cargo 清单已建立 | `PlatformCore/Cargo.toml` | 对应 Cargo 清单未建立 |
| `Runtime/Package.swift` | 16 | Cargo 清单已建立 | `Runtime/Cargo.toml` | 对应 Cargo 清单未建立 |
| `Shell/Package.swift` | 55 | Cargo 清单已建立 | `Shell/Cargo.toml` | 对应 Cargo 清单未建立 |
| `System/Package.swift` | 26 | Cargo 清单已建立 | `System/Cargo.toml` | 对应 Cargo 清单未建立 |
| `Terminal/Package.swift` | 55 | Cargo 清单已建立 | `Terminal/Cargo.toml` | 对应 Cargo 清单未建立 |
| `Tests/Headless/Package.swift` | 6 | 未开始 | `Tests/Headless/Cargo.toml`<br>（计划） | 对应 Cargo 清单未建立 |
| `Text/Package.swift` | 16 | Cargo 清单已建立 | `Text/Cargo.toml` | 对应 Cargo 清单未建立 |
| `Tools/Cove/Package.swift` | 24 | Cargo 清单已建立 | `Tools/Cove/Cargo.toml` | 对应 Cargo 清单未建立 |
| `Tools/Support/Package.swift` | 10 | Cargo 清单已建立 | `Tools/Support/Cargo.toml` | 对应 Cargo 清单未建立 |

### CLI 项目模板

| 源文件 | 行 | 状态 | Rust/C 目标 | 说明 |
| --- | ---: | --- | --- | --- |
| `Tools/Cove/Sources/CoveCLI/Templates/UI/Package.swift.in` | 73 | 未开始 | — | CLI 生成模板，随 SDK 模板迁移 |
| `Tools/Cove/Sources/CoveCLI/Templates/UI/Platforms/iOS/main.swift` | 4 | 未开始 | — | CLI 生成模板，随 SDK 模板迁移 |
| `Tools/Cove/Sources/CoveCLI/Templates/UI/Sources/AppContent/App.swift` | 26 | 未开始 | — | CLI 生成模板，随 SDK 模板迁移 |
| `Tools/Cove/Sources/CoveCLI/Templates/UI/Sources/AppContent/AppResources.swift` | 14 | 未开始 | — | CLI 生成模板，随 SDK 模板迁移 |
| `Tools/Cove/Sources/CoveCLI/Templates/UI/Tests/AppContentTests/Resources.swift` | 6 | 未开始 | — | CLI 生成模板，随 SDK 模板迁移 |

