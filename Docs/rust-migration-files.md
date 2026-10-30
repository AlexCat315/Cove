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
| 生产源码 | 984 | 894 | 0 | 90 |
| 测试源码 | 424 | 54 | 0 | 370 |
| SwiftPM 清单 | 25 | 19 | 0 | 6 |
| 项目模板 | 5 | 0 | 0 | 5 |
| 合计 | 1438 | — | — | — |

## 生产源码逐文件映射

### Compose

共 414 个文件：已写入 375，进行中 0，未开始 39。

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
| `Compose/Sources/CoveUICompose/Animation/Geometry/Matched.swift` | 146 | 已写入（未验证） | `Compose/rust/animation/geometry/matched.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Animation/Geometry/Transforms.swift` | 40 | 已写入（未验证） | `Compose/rust/animation/geometry/transforms.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Animation/Interaction/SpringDrag.swift` | 126 | 已写入（未验证） | `Compose/rust/animation/interaction/spring_drag.rs` | 未编译、未行为对照 |
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
| `Compose/Sources/CoveUICompose/Primitives/Columns/Picker.swift` | 39 | 已写入（未验证） | `Compose/rust/primitives/columns/picker.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Columns/Priority.swift` | 50 | 已写入（未验证） | `Compose/rust/primitives/columns/mod.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Adaptive/ControlExtent.swift` | 15 | 已写入（未验证） | `Compose/rust/primitives/controls/adaptive/control_extent.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Adaptive/LongPress.swift` | 41 | 已写入（未验证） | `Compose/rust/primitives/controls/adaptive/long_press.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/AppBar.swift` | 115 | 已写入（未验证） | `Compose/rust/primitives/controls/app_bar.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/AppBar/Back.swift` | 22 | 已写入（未验证） | `Core/rust/scene/node_renderer.rs`<br>`Graphics/rust/dx12/backend.rs`<br>`Graphics/rust/dx12/device.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Button.swift` | 551 | 已写入（未验证） | `Compose/rust/primitives/controls/button.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Button/Chrome.swift` | 280 | 已写入（未验证） | `Compose/rust/theme/styles/buttons/builtin_button_chrome.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Calendar/Calendar.swift` | 399 | 已写入（未验证） | `Compose/rust/foundation/calendar.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Calendar/Navigation.swift` | 109 | 已写入（未验证） | `Compose/rust/primitives/layout/sidebar/mod.rs`<br>`Compose/rust/primitives/layout/sidebar/sidebar.rs`<br>`Compose/rust/primitives/navigation/mod.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Calendar/Period.swift` | 98 | 已写入（未验证） | `Compose/rust/primitives/controls/calendar/period.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/CompletionPopover.swift` | 252 | 已写入（未验证） | `Compose/rust/primitives/overlays/popover.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/DateValues.swift` | 69 | 已写入（未验证） | `Compose/rust/primitives/controls/date_values.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/FieldLabel.swift` | 47 | 已写入（未验证） | `Compose/rust/primitives/controls/field_label.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/AssetRefField.swift` | 188 | 未开始 | — | 等待实现 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/ColorField.swift` | 296 | 未开始 | — | 等待实现 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/Combobox/Combobox.swift` | 197 | 未开始 | — | 等待实现 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/Combobox/RemoteSearch.swift` | 129 | 未开始 | — | 等待实现 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/Combobox/Results.swift` | 50 | 未开始 | — | 等待实现 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/DatePicker.swift` | 55 | 已写入（未验证） | `Compose/rust/primitives/columns/picker.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/DateRange/RangePicker.swift` | 311 | 已写入（未验证） | `Compose/rust/primitives/columns/picker.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/DateRange/Rules.swift` | 84 | 未开始 | — | 等待实现 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/JSON/ExpandedEditor.swift` | 104 | 已写入（未验证） | `DevTools/rust/inspection/editor.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/JsonField.swift` | 235 | 未开始 | — | 等待实现 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/Numeric/AxisLayout.swift` | 46 | 已写入（未验证） | `Hosts/rust/android/probe/layout.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/Numeric/NumberField.swift` | 291 | 已写入（未验证） | `Compose/rust/primitives/controls/fields/numeric/number_field.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/Numeric/PrefixStyle.swift` | 37 | 已写入（未验证） | `Compose/rust/primitives/controls/fields/numeric/prefix_style.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/Numeric/Scrub.swift` | 110 | 已写入（未验证） | `Compose/rust/primitives/controls/fields/numeric/scrub.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/Numeric/TextFormat.swift` | 50 | 已写入（未验证） | `Compose/rust/primitives/controls/fields/numeric/text_format.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/Numeric/Vec3Field.swift` | 115 | 未开始 | — | 等待实现 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/RadioGroup.swift` | 104 | 已写入（未验证） | `Compose/rust/primitives/controls/fields/radio_group.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/RangeSlider.swift` | 498 | 已写入（未验证） | `Compose/rust/primitives/controls/fields/slider/slider.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/SearchField.swift` | 154 | 未开始 | — | 等待实现 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/Select.swift` | 255 | 已写入（未验证） | `Compose/rust/primitives/controls/fields/select.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/Slider/Geometry.swift` | 37 | 已写入（未验证） | `Compose/rust/primitives/controls/fields/slider/geometry.rs`<br>`Compose/rust/primitives/controls/fields/slider/mod.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/Slider/Host.swift` | 206 | 已写入（未验证） | `Compose/rust/primitives/controls/fields/slider/host.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/Slider/Paint.swift` | 78 | 已写入（未验证） | `Compose/rust/primitives/controls/fields/slider/paint.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/Slider/Slider.swift` | 76 | 已写入（未验证） | `Compose/rust/primitives/controls/fields/slider/slider.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/TimeField.swift` | 60 | 未开始 | — | 等待实现 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Fields/TimeInput.swift` | 320 | 未开始 | — | 等待实现 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Form.swift` | 144 | 已写入（未验证） | `Compose/rust/primitives/controls/form/form.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Form/LabelPlacement.swift` | 15 | 已写入（未验证） | `Compose/rust/primitives/controls/form/label_placement.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Form/LabelScope.swift` | 19 | 已写入（未验证） | `Compose/rust/primitives/controls/form/label_scope.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/FormValidation.swift` | 75 | 已写入（未验证） | `Compose/rust/primitives/controls/form_validation.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/InlineLabelLayout.swift` | 46 | 未开始 | — | 等待实现 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/KeyCap.swift` | 78 | 已写入（未验证） | `Compose/rust/primitives/controls/key_cap.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Link.swift` | 113 | 已写入（未验证） | `Compose/rust/primitives/controls/link.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/MenuButton.swift` | 117 | 已写入（未验证） | `Compose/rust/primitives/controls/button.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Rating/Rating.swift` | 74 | 已写入（未验证） | `Compose/rust/primitives/controls/rating/rating.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Rating/RatingGeometry.swift` | 75 | 已写入（未验证） | `Compose/rust/primitives/controls/fields/slider/geometry.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Rating/RatingHost.swift` | 114 | 已写入（未验证） | `Compose/rust/primitives/controls/fields/slider/host.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Rating/RatingPaint.swift` | 61 | 已写入（未验证） | `Compose/rust/primitives/controls/fields/slider/paint.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/SplitButton.swift` | 102 | 已写入（未验证） | `Compose/rust/primitives/controls/button.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Tag.swift` | 153 | 已写入（未验证） | `Compose/rust/primitives/controls/tag.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/TimePicker.swift` | 132 | 已写入（未验证） | `Compose/rust/primitives/columns/picker.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Toggle.swift` | 500 | 已写入（未验证） | `Compose/rust/primitives/controls/toggle.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Toolbar.swift` | 163 | 已写入（未验证） | `Compose/rust/theme/styles/buttons/toolbar_toggle_button_style.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Toolbar/Commands.swift` | 143 | 已写入（未验证） | `Graphics/rust/rhi/commands.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Toolbar/OverflowLayout.swift` | 83 | 已写入（未验证） | `Hosts/rust/android/probe/layout.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/Toolbar/RetainedCommand.swift` | 61 | 已写入（未验证） | `System/rust/command.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Controls/WheelPicker.swift` | 269 | 已写入（未验证） | `Compose/rust/primitives/columns/picker.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Avatar/Avatar.swift` | 165 | 已写入（未验证） | `Compose/rust/primitives/data/avatar/avatar.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Avatar/Group.swift` | 135 | 已写入（未验证） | `Compose/rust/primitives/data/avatar/group.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Avatar/MemberList.swift` | 89 | 已写入（未验证） | `Compose/rust/primitives/data/avatar/member_list.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Avatar/Status.swift` | 96 | 已写入（未验证） | `Compose/rust/primitives/data/avatar/status.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Chart/Axes.swift` | 68 | 已写入（未验证） | `Compose/rust/primitives/data/chart/axes.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Chart/Axis.swift` | 218 | 已写入（未验证） | `Compose/rust/primitives/data/chart/axis.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Chart/Cache.swift` | 47 | 已写入（未验证） | `Compose/rust/primitives/data/chart/cache.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Chart/Chart.swift` | 262 | 已写入（未验证） | `Compose/rust/primitives/data/chart/axes.rs`<br>`Compose/rust/primitives/data/chart/axis.rs`<br>`Compose/rust/primitives/data/chart/cache.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Chart/ChartDataTable.swift` | 205 | 已写入（未验证） | `Compose/rust/primitives/data/lists/table.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Chart/ChartOverlay.swift` | 218 | 已写入（未验证） | `Compose/rust/primitives/data/chart/overlay.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Chart/Coordinates.swift` | 333 | 已写入（未验证） | `Compose/rust/primitives/data/chart/coordinates.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Chart/Drawing.swift` | 499 | 已写入（未验证） | `Compose/rust/primitives/data/chart/drawing.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Chart/Histogram.swift` | 80 | 已写入（未验证） | `Compose/rust/primitives/data/chart/histogram.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Chart/Interaction.swift` | 175 | 已写入（未验证） | `Compose/rust/primitives/data/chart/interaction.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Chart/Legend.swift` | 102 | 已写入（未验证） | `Compose/rust/primitives/data/chart/legend.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Chart/Model.swift` | 474 | 已写入（未验证） | `Compose/rust/primitives/data/chart/model.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Chart/Morph.swift` | 77 | 已写入（未验证） | `Compose/rust/primitives/data/chart/morph.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Chart/Plot.swift` | 708 | 已写入（未验证） | `Compose/rust/primitives/data/chart/plot.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Chart/Scale.swift` | 203 | 已写入（未验证） | `Compose/rust/primitives/data/chart/scale.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Chart/Stacking.swift` | 112 | 已写入（未验证） | `Compose/rust/primitives/data/chart/stacking.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Chart/Viewport.swift` | 92 | 已写入（未验证） | `Compose/rust/primitives/data/chart/viewport.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Columns/Focus.swift` | 44 | 已写入（未验证） | `Compose/rust/primitives/interaction/scrollbar.rs`<br>`Compose/rust/primitives/layout/sidebar/focus_handoff.rs`<br>`Compose/rust/primitives/overlays/roving_focus.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Columns/TablePresentation.swift` | 34 | 已写入（未验证） | `Graphics/rust/rhi/presentation.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Columns/Visibility.swift` | 25 | 未开始 | — | 等待实现 |
| `Compose/Sources/CoveUICompose/Primitives/Data/DataTable/ColumnOrder.swift` | 141 | 未开始 | — | 等待实现 |
| `Compose/Sources/CoveUICompose/Primitives/Data/DataTable/DataTable.swift` | 604 | 已写入（未验证） | `Compose/rust/primitives/data/lists/table.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/DataTable/DataTableEditing.swift` | 66 | 未开始 | — | 等待实现 |
| `Compose/Sources/CoveUICompose/Primitives/Data/DataTable/DataTableGeometry.swift` | 32 | 已写入（未验证） | `Compose/rust/primitives/data/data_table/data_table_geometry.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/DataTable/DataTableHosts.swift` | 119 | 未开始 | — | 等待实现 |
| `Compose/Sources/CoveUICompose/Primitives/Data/DataTable/DataTableModel.swift` | 112 | 已写入（未验证） | `Compose/rust/primitives/data/chart/model.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/DataTable/DataTableSelection.swift` | 29 | 已写入（未验证） | `Compose/rust/primitives/data/lists/selection.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/DataTable/DataTableSortWorker.swift` | 45 | 未开始 | — | 等待实现 |
| `Compose/Sources/CoveUICompose/Primitives/Data/DataTable/EditorPresence.swift` | 22 | 未开始 | — | 等待实现 |
| `Compose/Sources/CoveUICompose/Primitives/Data/DataTable/VariableRows.swift` | 60 | 未开始 | — | 等待实现 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Lists/InsetGroup.swift` | 79 | 已写入（未验证） | `Compose/rust/primitives/data/lists/inset_group.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Lists/List.swift` | 285 | 已写入（未验证） | `Compose/rust/primitives/data/lists/list.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Lists/Refresh.swift` | 79 | 已写入（未验证） | `Compose/rust/primitives/data/lists/refresh.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Lists/Reorder/Handle.swift` | 151 | 已写入（未验证） | `Hosts/rust/desktop/display/handle.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Lists/Reorder/Preview.swift` | 62 | 未开始 | — | 等待实现 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Lists/Reorder/Reorderable.swift` | 268 | 未开始 | — | 等待实现 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Lists/Row.swift` | 152 | 已写入（未验证） | `Compose/Composition/rust/box_view.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Lists/SectionedList.swift` | 272 | 已写入（未验证） | `Compose/rust/primitives/data/lists/list.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Lists/Selection.swift` | 61 | 已写入（未验证） | `Terminal/rust/engine/selection.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Lists/SwipeList.swift` | 409 | 已写入（未验证） | `Compose/rust/primitives/data/lists/list.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Lists/Table.swift` | 309 | 已写入（未验证） | `Compose/rust/primitives/text/markdown/table_block.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Lists/Tree.swift` | 1372 | 已写入（未验证） | `Compose/Composition/rust/layout_tree.rs`<br>`Compose/Composition/rust/view_graph.rs`<br>`Compose/rust/tests/row_slot.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Lists/Tree/Guides.swift` | 135 | 已写入（未验证） | `Compose/rust/primitives/text/text_field/indent_guides.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Lists/TreeOptions.swift` | 86 | 未开始 | — | 等待实现 |
| `Compose/Sources/CoveUICompose/Primitives/Data/PropertyGrid.swift` | 454 | 已写入（未验证） | `Compose/rust/primitives/layout/grid/grid.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Timeline.swift` | 266 | 已写入（未验证） | `Core/rust/scene/animation/timeline.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/Timeline/HeadingLayout.swift` | 52 | 已写入（未验证） | `Hosts/rust/android/probe/layout.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Data/TreeTable/TreeTable.swift` | 201 | 已写入（未验证） | `Compose/rust/primitives/data/lists/table.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Feedback/Alert.swift` | 157 | 已写入（未验证） | `Compose/rust/primitives/feedback/alert.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Feedback/Badge.swift` | 142 | 已写入（未验证） | `Compose/rust/primitives/feedback/badge.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Feedback/EmptyState.swift` | 117 | 已写入（未验证） | `Compose/rust/primitives/feedback/empty_state.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Feedback/Progress.swift` | 129 | 已写入（未验证） | `Compose/rust/primitives/feedback/progress.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Feedback/ProgressCircle.swift` | 170 | 已写入（未验证） | `Compose/rust/primitives/feedback/progress_circle.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Feedback/ProgressValue.swift` | 35 | 已写入（未验证） | `Compose/rust/primitives/feedback/progress_value.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Feedback/Spinner.swift` | 60 | 已写入（未验证） | `Compose/rust/primitives/feedback/spinner.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Feedback/StatusTone.swift` | 46 | 已写入（未验证） | `Compose/rust/primitives/feedback/status_tone.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Interaction/DragAndDrop.swift` | 285 | 已写入（未验证） | `Compose/rust/primitives/interaction/drag_and_drop.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Interaction/EdgeAutoScroll.swift` | 72 | 已写入（未验证） | `Compose/rust/primitives/interaction/edge_auto_scroll.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Interaction/HorizontalSwipe.swift` | 120 | 已写入（未验证） | `Compose/rust/primitives/interaction/horizontal_swipe.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Interaction/Scrollbar.swift` | 303 | 已写入（未验证） | `Compose/rust/primitives/interaction/scrollbar.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Accordion.swift` | 74 | 已写入（未验证） | `Compose/rust/primitives/layout/accordion.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/BoundedScrollView.swift` | 50 | 已写入（未验证） | `Compose/rust/primitives/layout/bounded_scroll_view.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Carousel/Autoplay.swift` | 132 | 未开始 | — | 等待实现 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Carousel/Carousel.swift` | 215 | 未开始 | — | 等待实现 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Carousel/Indicator.swift` | 44 | 未开始 | — | 等待实现 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Direction/Resolved.swift` | 35 | 已写入（未验证） | `Compose/rust/primitives/layout/direction/resolved.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/DisclosureGroup.swift` | 62 | 已写入（未验证） | `Compose/rust/primitives/data/avatar/group.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Grid/Columns.swift` | 157 | 已写入（未验证） | `Compose/rust/primitives/layout/grid/columns.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Grid/Grid.swift` | 267 | 已写入（未验证） | `Compose/rust/primitives/layout/grid/grid.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Grid/InteractionGate.swift` | 17 | 已写入（未验证） | `Compose/rust/primitives/layout/grid/grid.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Grid/LazyGrid.swift` | 211 | 已写入（未验证） | `Compose/rust/primitives/layout/grid/lazy_grid.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Grid/NamedColumns.swift` | 27 | 已写入（未验证） | `Compose/rust/primitives/layout/grid/named_columns.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Grid/RowHeights.swift` | 109 | 已写入（未验证） | `Compose/rust/primitives/layout/grid/row_heights.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Grid/Span.swift` | 82 | 已写入（未验证） | `Compose/rust/primitives/layout/grid/span.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/HorizontalScrollOrigin.swift` | 16 | 已写入（未验证） | `Compose/rust/primitives/layout/horizontal_scroll_origin.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Insets/SafeArea.swift` | 65 | 已写入（未验证） | `Compose/rust/primitives/layout/insets/safe_area.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Insets/SurfaceRoot.swift` | 53 | 已写入（未验证） | `Compose/rust/primitives/layout/insets/surface_root.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Masonry/Masonry.swift` | 167 | 已写入（未验证） | `Compose/rust/primitives/layout/masonry/masonry.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/NavigationComponents.swift` | 114 | 已写入（未验证） | `Compose/rust/primitives/controls/button_group.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Overlay.swift` | 97 | 已写入（未验证） | `Compose/rust/primitives/layout/overlay.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Panel.swift` | 70 | 已写入（未验证） | `Compose/rust/primitives/layout/panel.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/ResizableEditor.swift` | 36 | 已写入（未验证） | `DevTools/rust/inspection/editor.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/ResizableTextArea.swift` | 56 | 未开始 | — | 等待实现 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Resize/HeightHandle.swift` | 131 | 已写入（未验证） | `Hosts/rust/desktop/display/handle.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Resize/HeightLimits.swift` | 13 | 已写入（未验证） | `Compose/rust/primitives/layout/resize/height_limits.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Responsive/ActionGroup.swift` | 57 | 已写入（未验证） | `Compose/rust/primitives/layout/responsive/action_group.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Responsive/Candidates.swift` | 41 | 已写入（未验证） | `Compose/rust/primitives/layout/responsive/candidates.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Responsive/Stack.swift` | 118 | 已写入（未验证） | `Compose/rust/primitives/layout/responsive/stack.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Responsive/ViewThatFits.swift` | 118 | 未开始 | — | 等待实现 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/RowSlot.swift` | 22 | 已写入（未验证） | `Compose/rust/primitives/layout/row_slot.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/ScrollReveal.swift` | 19 | 已写入（未验证） | `Compose/rust/primitives/layout/scroll_reveal.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/ScrollView.swift` | 1139 | 已写入（未验证） | `Compose/rust/primitives/layout/scroll_view.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Scrolling/KeyboardFocus.swift` | 49 | 未开始 | — | 等待实现 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Sidebar/AdaptiveHosts.swift` | 103 | 已写入（未验证） | `Compose/rust/primitives/layout/sidebar/adaptive_hosts.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Sidebar/AdaptiveOptions.swift` | 69 | 已写入（未验证） | `Compose/rust/primitives/layout/sidebar/adaptive_options.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Sidebar/AdaptiveSidebar.swift` | 199 | 已写入（未验证） | `Compose/rust/primitives/layout/sidebar/sidebar.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Sidebar/Destination.swift` | 234 | 已写入（未验证） | `Compose/rust/primitives/layout/sidebar/destination.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Sidebar/Drawer.swift` | 92 | 已写入（未验证） | `Compose/rust/primitives/layout/sidebar/drawer.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Sidebar/FocusHandoff.swift` | 110 | 已写入（未验证） | `Compose/rust/primitives/layout/sidebar/focus_handoff.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Sidebar/Hierarchy.swift` | 64 | 已写入（未验证） | `Compose/rust/primitives/layout/sidebar/hierarchy.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Sidebar/HierarchyHost.swift` | 85 | 已写入（未验证） | `Compose/rust/primitives/controls/fields/slider/host.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Sidebar/Items.swift` | 48 | 已写入（未验证） | `Compose/rust/primitives/layout/sidebar/items.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Sidebar/Presentation.swift` | 19 | 已写入（未验证） | `Graphics/rust/rhi/presentation.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Sidebar/PresentationToggle.swift` | 12 | 已写入（未验证） | `Compose/rust/primitives/controls/toggle.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Sidebar/Rail.swift` | 51 | 已写入（未验证） | `Compose/rust/primitives/layout/sidebar/rail.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Sidebar/RailDestination.swift` | 89 | 已写入（未验证） | `Compose/rust/primitives/layout/sidebar/rail.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Sidebar/ResizeHandle.swift` | 136 | 已写入（未验证） | `Hosts/rust/desktop/display/handle.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Sidebar/Resizing.swift` | 20 | 已写入（未验证） | `Compose/rust/primitives/layout/sidebar/resizing.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Sidebar/Sidebar.swift` | 126 | 已写入（未验证） | `Compose/rust/primitives/layout/sidebar/sidebar.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Split/Compact.swift` | 67 | 未开始 | — | 等待实现 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Split/Constraints.swift` | 44 | 已写入（未验证） | `Compose/rust/primitives/overlays/portal/fit_constraints.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Split/Retention.swift` | 109 | 已写入（未验证） | `Compose/rust/primitives/overlays/toast/exit_retention.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/SplitView.swift` | 360 | 已写入（未验证） | `Compose/Composition/rust/view.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/StatusBar.swift` | 184 | 未开始 | — | 等待实现 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/TabView.swift` | 299 | 已写入（未验证） | `Compose/Composition/rust/view.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Tabs/MenuStrip.swift` | 61 | 未开始 | — | 等待实现 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Tabs/MobileBar.swift` | 94 | 未开始 | — | 等待实现 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Tabs/StripLayout.swift` | 64 | 已写入（未验证） | `Hosts/rust/android/probe/layout.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Virtualization/EstimatedStack.swift` | 169 | 已写入（未验证） | `Compose/rust/primitives/layout/responsive/stack.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Virtualization/FixedStack.swift` | 137 | 已写入（未验证） | `Compose/rust/primitives/layout/responsive/stack.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Virtualization/RowIndex.swift` | 147 | 已写入（未验证） | `Compose/rust/primitives/layout/virtualization/row_index.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Virtualization/RowMetrics.swift` | 47 | 已写入（未验证） | `Compose/rust/primitives/layout/virtualization/row_metrics.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Virtualization/VariableStack.swift` | 108 | 已写入（未验证） | `Compose/rust/primitives/layout/responsive/stack.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Virtualization/VirtualStack.swift` | 77 | 已写入（未验证） | `Compose/rust/primitives/layout/responsive/stack.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/Virtualization/VirtualWindow.swift` | 41 | 已写入（未验证） | `Compose/rust/primitives/layout/virtualization/virtual_window.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Layout/ZStack.swift` | 89 | 已写入（未验证） | `Compose/rust/primitives/layout/zstack.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Media/AsyncImage.swift` | 335 | 已写入（未验证） | `Compose/rust/primitives/media/async_image.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Media/Icon.swift` | 87 | 已写入（未验证） | `Compose/rust/primitives/media/icon.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Media/Image+File.swift` | 144 | 已写入（未验证） | `Compose/rust/primitives/media/image_file.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Media/Image.swift` | 173 | 已写入（未验证） | `Compose/rust/primitives/media/image.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Media/ImageDataDownload.swift` | 130 | 已写入（未验证） | `Compose/rust/primitives/media/image_data_download.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Media/ImageGeometry.swift` | 33 | 已写入（未验证） | `Compose/rust/primitives/media/image_geometry.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Media/ImageLoadServices.swift` | 146 | 已写入（未验证） | `Compose/rust/primitives/media/image_load_services.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Media/LoadingAnimationClock.swift` | 33 | 已写入（未验证） | `Compose/rust/primitives/media/loading_animation_clock.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Media/Skeleton.swift` | 88 | 已写入（未验证） | `Compose/rust/primitives/media/skeleton.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Media/UIIconCatalog.swift` | 735 | 已写入（未验证） | `Compose/rust/primitives/media/ui_icon_catalog.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Media/VectorImageRaster.swift` | 44 | 已写入（未验证） | `Compose/rust/primitives/media/vector_image_raster.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Navigation/NavigationStack.swift` | 311 | 已写入（未验证） | `Compose/rust/primitives/navigation/navigation_stack.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Navigation/RouteOutlet.swift` | 19 | 已写入（未验证） | `Compose/rust/primitives/navigation/route_outlet.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Navigation/Stepper/Stepper.swift` | 155 | 已写入（未验证） | `Compose/rust/primitives/navigation/stepper/stepper.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Navigation/Stepper/Values.swift` | 50 | 已写入（未验证） | `Compose/rust/primitives/navigation/stepper/values.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/ActionSheet.swift` | 125 | 已写入（未验证） | `Compose/rust/primitives/overlays/modal/action_sheet.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/ActionSheet/Content.swift` | 106 | 已写入（未验证） | `Browser/rust/prototype/mod.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Anchored/Anchoring.swift` | 126 | 已写入（未验证） | `Compose/rust/primitives/overlays/anchored/anchoring.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Anchored/Placement.swift` | 94 | 已写入（未验证） | `Compose/rust/primitives/overlays/anchored/placement.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Anchored/Surface.swift` | 60 | 已写入（未验证） | `Compose/Composition/rust/adaptive/surface.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/AssetDropTarget.swift` | 212 | 未开始 | — | 等待实现 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Command/Search.swift` | 89 | 未开始 | — | 等待实现 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/CommandPalette.swift` | 436 | 未开始 | — | 等待实现 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/FocusScope.swift` | 22 | 已写入（未验证） | `Compose/rust/primitives/overlays/focus_scope.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/HoverCard.swift` | 183 | 未开始 | — | 等待实现 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Menu/ContextMenu.swift` | 155 | 已写入（未验证） | `Compose/rust/primitives/overlays/menu/menu.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Menu/Menu.swift` | 300 | 已写入（未验证） | `Compose/rust/primitives/overlays/menu/menu.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Menu/MenuDescriptor.swift` | 19 | 已写入（未验证） | `Compose/rust/primitives/overlays/menu/menu_descriptor.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Menu/MenuItemRow.swift` | 264 | 已写入（未验证） | `Compose/rust/primitives/overlays/menu/menu_item_row.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Menu/MenuNavigation.swift` | 188 | 已写入（未验证） | `Compose/rust/primitives/overlays/menu/menu_navigation.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Menu/NativeMenuBridge.swift` | 128 | 已写入（未验证） | `Hosts/rust/android/sources/cove_android_app/bridge.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Menu/RowContent.swift` | 60 | 已写入（未验证） | `Compose/rust/primitives/overlays/menu/row_content.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Menu/TouchMenu.swift` | 106 | 已写入（未验证） | `Compose/rust/primitives/overlays/menu/menu.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Modal/BottomSheet.swift` | 104 | 已写入（未验证） | `Compose/rust/primitives/overlays/modal/bottom_sheet.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Modal/Dialog.swift` | 146 | 已写入（未验证） | `Compose/rust/primitives/overlays/modal/dialog.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Modal/DragHandle.swift` | 104 | 已写入（未验证） | `Compose/rust/primitives/overlays/modal/drag_handle.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Modal/Modal.swift` | 115 | 已写入（未验证） | `Compose/rust/primitives/overlays/modal/modal.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Modal/ModalOptions.swift` | 48 | 已写入（未验证） | `Compose/rust/primitives/overlays/modal/modal_options.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Modal/TouchPresentation.swift` | 76 | 已写入（未验证） | `Compose/rust/primitives/overlays/modal/touch_presentation.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/OverlayHost.swift` | 10 | 已写入（未验证） | `Compose/rust/primitives/controls/fields/slider/host.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Popconfirm/Popconfirm.swift` | 71 | 未开始 | — | 等待实现 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Popover.swift` | 108 | 已写入（未验证） | `Compose/rust/primitives/overlays/popover.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Portal/FitConstraints.swift` | 19 | 已写入（未验证） | `Compose/rust/primitives/overlays/portal/fit_constraints.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Portal/LayerRoot.swift` | 95 | 未开始 | — | 等待实现 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Portal/OverlayPlacement.swift` | 24 | 已写入（未验证） | `Compose/rust/primitives/overlays/portal/overlay_placement.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Portal/PortalHost.swift` | 218 | 已写入（未验证） | `Compose/rust/primitives/controls/fields/slider/host.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Portal/PortalPlacement.swift` | 41 | 已写入（未验证） | `Compose/rust/primitives/overlays/portal/portal_placement.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Portal/PortalRegistry.swift` | 386 | 已写入（未验证） | `Compose/rust/primitives/overlays/portal/portal_registry.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/RovingFocus.swift` | 128 | 已写入（未验证） | `Compose/rust/primitives/overlays/roving_focus.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/SelectionKeyHost.swift` | 44 | 已写入（未验证） | `Compose/rust/primitives/overlays/selection_key_host.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/ShortcutHost.swift` | 58 | 已写入（未验证） | `Compose/rust/primitives/controls/fields/slider/host.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Toast/Controller.swift` | 90 | 已写入（未验证） | `Core/rust/scene/animation/controller.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Toast/ExitRetention.swift` | 45 | 已写入（未验证） | `Compose/rust/primitives/overlays/toast/exit_retention.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Toast/NotificationHost.swift` | 172 | 已写入（未验证） | `Compose/rust/primitives/overlays/toast/notification_host.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Overlays/Tooltip.swift` | 412 | 未开始 | — | 等待实现 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Code/FoldingOptions.swift` | 121 | 已写入（未验证） | `Compose/rust/primitives/text/code/folding_options.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Code/GutterControls.swift` | 181 | 已写入（未验证） | `Hosts/rust/desktop/display/controls.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Code/GutterGeometry.swift` | 46 | 已写入（未验证） | `Compose/rust/primitives/controls/fields/slider/geometry.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Code/GutterLayout.swift` | 74 | 已写入（未验证） | `Hosts/rust/android/probe/layout.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Code/GutterOptions.swift` | 69 | 已写入（未验证） | `Compose/rust/primitives/text/code/gutter_options.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Code/GutterSession.swift` | 121 | 已写入（未验证） | `DevTools/Protocol/rust/session.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Code/Minimap.swift` | 112 | 已写入（未验证） | `Compose/rust/primitives/text/code/minimap.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Code/Occurrences.swift` | 27 | 已写入（未验证） | `Compose/rust/primitives/text/code/occurrences.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Markdown/CodeBlock.swift` | 100 | 已写入（未验证） | `Compose/rust/primitives/text/markdown/code_block.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Markdown/Document.swift` | 30 | 已写入（未验证） | `Compose/rust/primitives/text/markdown/document.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Markdown/ListBlock.swift` | 98 | 已写入（未验证） | `Compose/rust/primitives/text/markdown/list_block.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Markdown/MarkdownView.swift` | 135 | 已写入（未验证） | `Compose/Composition/rust/view.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Markdown/Parse.swift` | 139 | 已写入（未验证） | `Compose/rust/primitives/text/markdown/parse.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Markdown/TableBlock.swift` | 107 | 已写入（未验证） | `Compose/rust/primitives/text/markdown/table_block.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Rich/Flow.swift` | 413 | 已写入（未验证） | `Compose/rust/primitives/text/rich/flow.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Rich/LinkAnchor.swift` | 56 | 已写入（未验证） | `Compose/rust/primitives/text/rich/link_anchor.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Rich/Paragraph.swift` | 357 | 已写入（未验证） | `Compose/rust/primitives/text/rich/paragraph.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Rich/RichText.swift` | 199 | 已写入（未验证） | `Compose/rust/primitives/text/text.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Rich/Run.swift` | 27 | 已写入（未验证） | `Compose/rust/primitives/text/rich/run.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Rich/TextSelection.swift` | 620 | 已写入（未验证） | `Compose/rust/primitives/text/rich/text_selection.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Rope/RopeDifference.swift` | 45 | 已写入（未验证） | `Compose/rust/primitives/text/rope/rope_difference.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Rope/RopeEditing.swift` | 70 | 已写入（未验证） | `Compose/rust/primitives/text/rope/rope_editing.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Rope/RopeLookup.swift` | 126 | 已写入（未验证） | `Compose/rust/primitives/text/rope/rope_lookup.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Rope/RopeStorage.swift` | 93 | 已写入（未验证） | `Compose/rust/primitives/text/rope/rope_storage.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/SingleLine.swift` | 19 | 已写入（未验证） | `Compose/rust/primitives/text/single_line.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/Text.swift` | 605 | 已写入（未验证） | `Compose/rust/primitives/text/text.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/TextBuffer.swift` | 188 | 已写入（未验证） | `Compose/rust/primitives/text/text_buffer.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/TextCompletionRebase.swift` | 47 | 已写入（未验证） | `Compose/rust/primitives/text/text_completion_rebase.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/TextDiagnostic.swift` | 118 | 已写入（未验证） | `Compose/rust/primitives/text/text_diagnostic.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/TextDocumentLayout.swift` | 263 | 已写入（未验证） | `Compose/rust/primitives/text/text_document_layout.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/TextEditHistory.swift` | 97 | 已写入（未验证） | `Compose/rust/primitives/text/text_edit_history.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/TextEditingCommands.swift` | 18 | 已写入（未验证） | `Compose/rust/primitives/text/text_editing_commands.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/TextField/BracketMatching.swift` | 129 | 已写入（未验证） | `Compose/rust/primitives/text/text_field/bracket_matching.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/TextField/IndentGuides.swift` | 84 | 已写入（未验证） | `Compose/rust/primitives/text/text_field/indent_guides.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/TextField/PlatformInput.swift` | 75 | 已写入（未验证） | `Core/rust/scene/input/platform_input_context.rs`<br>`Core/rust/scene/input/pointer_capture.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/TextField/Snippet.swift` | 177 | 已写入（未验证） | `Compose/rust/primitives/text/text_field/snippet.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/TextField/TextField.swift` | 1483 | 已写入（未验证） | `Compose/rust/primitives/text/text_field/text_field.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/TextField/TextFieldCodeEditing.swift` | 54 | 已写入（未验证） | `Compose/rust/primitives/text/text_field/text_field_code_editing.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/TextField/TextFieldEditing.swift` | 218 | 已写入（未验证） | `Compose/rust/primitives/text/text_field/text_field_editing.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/TextField/TextFieldHorizontalScroll.swift` | 35 | 已写入（未验证） | `Compose/rust/primitives/text/text_field/text_field_horizontal_scroll.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/TextField/TextFieldHost.swift` | 162 | 已写入（未验证） | `Compose/rust/primitives/controls/fields/slider/host.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/TextField/TextFieldInputController.swift` | 155 | 已写入（未验证） | `Compose/rust/primitives/overlays/toast/controller.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/TextField/TextFieldLayoutEngine.swift` | 262 | 已写入（未验证） | `Compose/rust/primitives/text/text_field/text_field_layout_engine.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/TextField/TextFieldNavigation.swift` | 60 | 已写入（未验证） | `Compose/rust/primitives/text/text_field/text_field_navigation.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/TextField/TextFieldObservation.swift` | 62 | 已写入（未验证） | `Compose/rust/primitives/text/text_field/text_field_observation.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Primitives/Text/TextField/TextFieldOptions.swift` | 122 | 已写入（未验证） | `Compose/rust/primitives/text/text_field/text_field_options.rs` | 未编译、未行为对照 |
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
| `Compose/Sources/CoveUICompose/Theme/Styles/Components/DefaultListRowStyle.swift` | 78 | 已写入（未验证） | `Compose/rust/theme/styles/components/default_list_row_style.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Styles/Components/DefaultPanelStyle.swift` | 48 | 已写入（未验证） | `Compose/rust/theme/styles/components/default_panel_style.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Styles/Components/DefaultTextFieldStyle.swift` | 48 | 已写入（未验证） | `Compose/rust/theme/styles/components/default_text_field_style.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Styles/Components/DefaultTreeRowStyle.swift` | 62 | 已写入（未验证） | `Compose/rust/theme/styles/components/default_tree_row_style.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Styles/Components/DividerStyle.swift` | 42 | 已写入（未验证） | `Compose/rust/theme/styles/components/divider_style.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Styles/Components/EditorTextFieldStyle.swift` | 18 | 已写入（未验证） | `Compose/rust/theme/styles/components/editor_text_field_style.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Styles/Components/InputAppearance.swift` | 72 | 已写入（未验证） | `Compose/rust/theme/styles/components/input_appearance.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Styles/Components/ListRowStyle.swift` | 64 | 已写入（未验证） | `Compose/rust/theme/styles/components/list_row_style.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Styles/Components/PanelStyle.swift` | 40 | 已写入（未验证） | `Compose/rust/theme/styles/components/panel_style.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Styles/Components/SelectionAppearance.swift` | 83 | 已写入（未验证） | `Compose/rust/theme/styles/components/selection_appearance.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Styles/Components/SliderStyle.swift` | 161 | 已写入（未验证） | `Compose/rust/theme/styles/components/slider_style.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Styles/Components/TextFieldStyle.swift` | 71 | 已写入（未验证） | `Compose/rust/theme/styles/components/text_field_style.rs` | 未编译、未行为对照 |
| `Compose/Sources/CoveUICompose/Theme/Styles/Components/TreeRowStyle.swift` | 78 | 已写入（未验证） | `Compose/rust/theme/styles/components/tree_row_style.rs` | 未编译、未行为对照 |
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

### Graphics

共 107 个文件：已写入 102，进行中 0，未开始 5。

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
| `Graphics/Sources/NativeRHI/DX12/DX12Backend.swift` | 15 | 已写入（未验证） | `Graphics/rust/dx12/backend.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/DX12/DX12Device.swift` | 94 | 已写入（未验证） | `Graphics/rust/dx12/device.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/DX12/DX12Formats.swift` | 32 | 已写入（未验证） | `Graphics/rust/dx12/formats.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/DX12/Resources/DX12Compilation.swift` | 24 | 已写入（未验证） | `Graphics/rust/dx12/resources.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/DX12/Resources/DX12Pipelines.swift` | 74 | 已写入（未验证） | `Graphics/rust/vulkan/pipelines.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/DX12/Resources/DX12Resources.swift` | 92 | 已写入（未验证） | `Graphics/rust/dx12/resources.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/DX12/Submission/DX12Submission.swift` | 144 | 已写入（未验证） | `Graphics/rust/rhi/submission.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/DX12/Submission/DX12Uploader.swift` | 34 | 已写入（未验证） | `Graphics/rust/dx12/resources.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Device/AsyncResources.swift` | 26 | 已写入（未验证） | `Graphics/rust/rhi/resource_preparation.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Device/Capabilities.swift` | 45 | 已写入（未验证） | `Graphics/rust/rhi/capabilities.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Device/Commands.swift` | 353 | 已写入（未验证） | `Graphics/rust/rhi/commands.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Device/Descriptors.swift` | 507 | 已写入（未验证） | `Graphics/rust/rhi/descriptors.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Device/Device.swift` | 937 | 已写入（未验证） | `Graphics/rust/rhi/device.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Device/Handles.swift` | 25 | 已写入（未验证） | `Graphics/rust/rhi/handles.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Device/PipelineCache.swift` | 295 | 已写入（未验证） | `Graphics/rust/rhi/pipeline_cache.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Device/Queue.swift` | 24 | 已写入（未验证） | `Graphics/rust/rhi/queue.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Device/ResourcePreparation.swift` | 120 | 已写入（未验证） | `Graphics/rust/rhi/resource_preparation.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Metal/Bindings/MetalBindingRegistry.swift` | 33 | 已写入（未验证） | `Graphics/rust/metal/registries.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Metal/Bindings/MetalLibraryCache.swift` | 62 | 已写入（未验证） | `Graphics/rust/metal/library_cache.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Metal/Bindings/MetalRegistries.swift` | 129 | 已写入（未验证） | `Graphics/rust/metal/registries.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Metal/Bindings/MetalRenderBindings.swift` | 46 | 已写入（未验证） | `Graphics/rust/metal/render_bindings.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Metal/Device/MetalCompilation.swift` | 12 | 未开始 | — | 等待实现 |
| `Graphics/Sources/NativeRHI/Metal/Device/MetalDeviceResources.swift` | 275 | 已写入（未验证） | `Graphics/rust/dx12/resources.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Metal/Device/MetalDeviceTransfer.swift` | 299 | 已写入（未验证） | `Graphics/rust/metal/transfer.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Metal/Device/MetalUploadRing.swift` | 55 | 已写入（未验证） | `Graphics/rust/metal/upload_ring.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Metal/MetalAccelerationStructure.swift` | 97 | 已写入（未验证） | `Graphics/rust/metal/acceleration.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Metal/MetalBackend.swift` | 16 | 已写入（未验证） | `Graphics/rust/metal/backend.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Metal/MetalDevice.swift` | 208 | 已写入（未验证） | `Graphics/rust/metal/device.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Metal/MetalFormats.swift` | 232 | 已写入（未验证） | `Graphics/rust/metal/formats.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Metal/Pipelines/MetalMeshPipeline.swift` | 86 | 已写入（未验证） | `Graphics/rust/metal/registries.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Metal/Pipelines/MetalSubmissionStatus.swift` | 20 | 已写入（未验证） | `Graphics/rust/metal/submission_status.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Metal/Pipelines/MetalSubmitEncoder.swift` | 610 | 已写入（未验证） | `Graphics/rust/metal/backend.rs`<br>`Graphics/rust/metal/submit.rs` | 未编译、未行为对照 |
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
| `Graphics/Sources/NativeRHI/Vulkan/Backend/Bindings.swift` | 147 | 已写入（未验证） | `Graphics/rust/metal/library_cache.rs`<br>`Graphics/rust/metal/mod.rs`<br>`Graphics/rust/metal/registries.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Vulkan/Backend/Encoder.swift` | 324 | 已写入（未验证） | `Graphics/rust/vulkan/submission.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Vulkan/Backend/Pipelines.swift` | 179 | 已写入（未验证） | `Graphics/rust/vulkan/pipelines.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Vulkan/Backend/Submit.swift` | 405 | 已写入（未验证） | `Graphics/rust/metal/submit.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Vulkan/Backend/Timestamps.swift` | 149 | 已写入（未验证） | `Graphics/rust/vulkan/submission.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Vulkan/Backend/VulkanBackendResources.swift` | 339 | 已写入（未验证） | `Graphics/rust/dx12/resources.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Vulkan/Backend/VulkanBackendTransfer.swift` | 258 | 已写入（未验证） | `Graphics/rust/metal/transfer.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Vulkan/Backend/VulkanCompilation.swift` | 12 | 未开始 | — | 等待实现 |
| `Graphics/Sources/NativeRHI/Vulkan/Presentation/PresentMode.swift` | 11 | 已写入（未验证） | `Graphics/rust/vulkan/presentation.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Vulkan/Presentation/SurfaceState.swift` | 7 | 已写入（未验证） | `Hosts/rust/android/integration/surface/state.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Vulkan/Presentation/VulkanPresentation.swift` | 24 | 已写入（未验证） | `Graphics/rust/rhi/presentation.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Vulkan/Presentation/VulkanSwapchain.swift` | 319 | 已写入（未验证） | `Graphics/rust/vulkan/presentation.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Vulkan/Presentation/VulkanSynchronization.swift` | 107 | 已写入（未验证） | `Graphics/rust/vulkan/presentation.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Vulkan/Resources/VulkanAcceleration.swift` | 154 | 已写入（未验证） | `Graphics/rust/metal/acceleration.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Vulkan/Resources/VulkanMemory.swift` | 206 | 已写入（未验证） | `Graphics/rust/vulkan/memory.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Vulkan/Resources/VulkanMesh.swift` | 44 | 已写入（未验证） | `Graphics/rust/vulkan/registries.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Vulkan/Resources/VulkanResources.swift` | 100 | 已写入（未验证） | `Graphics/rust/dx12/resources.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Vulkan/Setup/VulkanDeviceSetup.swift` | 373 | 已写入（未验证） | `Graphics/rust/vulkan/setup.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Vulkan/Setup/VulkanInstanceSetup.swift` | 84 | 已写入（未验证） | `Graphics/rust/vulkan/setup.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Vulkan/Submission/CommandPool.swift` | 32 | 已写入（未验证） | `Graphics/rust/vulkan/submission.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Vulkan/Submission/VulkanCleanup.swift` | 32 | 未开始 | — | 等待实现 |
| `Graphics/Sources/NativeRHI/Vulkan/Submission/VulkanScratch.swift` | 28 | 已写入（未验证） | `Graphics/rust/vulkan/pipelines.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Vulkan/Submission/VulkanSubmissionStatus.swift` | 10 | 已写入（未验证） | `Graphics/rust/vulkan/status.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Vulkan/Submission/VulkanUploadRing.swift` | 32 | 已写入（未验证） | `Graphics/rust/metal/upload_ring.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Vulkan/VulkanAdvanced.swift` | 37 | 未开始 | — | 等待实现 |
| `Graphics/Sources/NativeRHI/Vulkan/VulkanAuxiliary.swift` | 52 | 已写入（未验证） | `Hosts/rust/desktop/window/auxiliary.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Vulkan/VulkanBackend.swift` | 175 | 已写入（未验证） | `Graphics/rust/vulkan/backend.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Vulkan/VulkanFormats.swift` | 148 | 已写入（未验证） | `Graphics/rust/vulkan/formats.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Vulkan/VulkanFunctions.swift` | 687 | 未开始 | — | 等待实现 |
| `Graphics/Sources/NativeRHI/Vulkan/VulkanRendering.swift` | 149 | 已写入（未验证） | `Graphics/rust/vulkan/rendering.rs` | 未编译、未行为对照 |
| `Graphics/Sources/NativeRHI/Vulkan/VulkanRuntime.swift` | 67 | 已写入（未验证） | `Hosts/rust/desktop/application/runtime.rs` | 未编译、未行为对照 |

### Core

共 97 个文件：已写入 97，进行中 0，未开始 0。

| 源文件 | 行 | 状态 | Rust/C 目标 | 依赖或阻塞 |
| --- | ---: | --- | --- | --- |
| `Core/Sources/CoveUICore/Adaptive/Environment.swift` | 82 | 已写入（未验证） | `Core/rust/adaptive_environment.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/Adaptive/TextScale.swift` | 44 | 已写入（未验证） | `Core/rust/text_scale.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/AnalyticClipper.swift` | 62 | 已写入（未验证） | `Core/rust/analytic_clipper.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/AnalyticStroke.swift` | 80 | 已写入（未验证） | `Core/rust/analytic_stroke.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/Bidi.swift` | 267 | 已写入（未验证） | `Core/rust/bidi.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/Binding.swift` | 32 | 已写入（未验证） | `Core/rust/binding.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/Bitmap.swift` | 21 | 已写入（未验证） | `Core/rust/bitmap.rs`<br>`Core/rust/draw_list_resources.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/Color.swift` | 56 | 已写入（未验证） | `Core/rust/geometry.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/ContentScale.swift` | 7 | 已写入（未验证） | `Core/rust/content_scale.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/DrawList.swift` | 1026 | 已写入（未验证） | `Core/rust/draw.rs`<br>`Core/rust/draw_primitives.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/DrawListResources.swift` | 25 | 已写入（未验证） | `Core/rust/draw_list_resources.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/DynamicProperty.swift` | 5 | 已写入（未验证） | `Core/rust/dynamic_property.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/Font.swift` | 50 | 已写入（未验证） | `Core/rust/font.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/GeometryKernels.swift` | 62 | 已写入（未验证） | `Core/rust/geometry_kernels.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/GlyphMetrics.swift` | 66 | 已写入（未验证） | `Core/rust/glyph_metrics.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/LineBreaks.swift` | 45 | 已写入（未验证） | `Core/rust/line_breaks.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/LinearGradient.swift` | 53 | 已写入（未验证） | `Core/rust/linear_gradient.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/Mipmaps.swift` | 52 | 已写入（未验证） | `Core/rust/bitmap.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/RoundedClip.swift` | 124 | 已写入（未验证） | `Core/rust/rounded_clip.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/ShapedGlyph.swift` | 27 | 已写入（未验证） | `Core/rust/shaped_glyph.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/State.swift` | 154 | 已写入（未验证） | `Core/rust/state.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/StateReadTracking.swift` | 431 | 已写入（未验证） | `Core/rust/state_read_tracking.rs`<br>`Core/rust/state.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/SurfaceShadow.swift` | 55 | 已写入（未验证） | `Core/rust/surface_shadow.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/TextLayout.swift` | 381 | 已写入（未验证） | `Core/rust/text_layout.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/TextMeasureSlot.swift` | 67 | 已写入（未验证） | `Core/rust/text_measure_slot.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/UIReveal.swift` | 43 | 已写入（未验证） | `Core/rust/ui_reveal.rs` | 未编译、未行为对照 |
| `Core/Sources/CoveUICore/UIShader.swift` | 107 | 已写入（未验证） | `Core/rust/ui_shader.rs` | 未编译、未行为对照 |
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

### Tools

共 69 个文件：已写入 69，进行中 0，未开始 0。

| 源文件 | 行 | 状态 | Rust/C 目标 | 依赖或阻塞 |
| --- | ---: | --- | --- | --- |
| `Tools/Cove/Sources/CoveCLI/CLI/Arguments.swift` | 286 | 已写入（未验证） | `Tools/Cove/rust/cli.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/CLI/Diagnostics.swift` | 21 | 已写入（未验证） | `Tools/Cove/rust/support.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/CLI/Platform.swift` | 51 | 已写入（未验证） | `Tools/Cove/rust/cli.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Development/AndroidRun.swift` | 41 | 已写入（未验证） | `Tools/Cove/rust/main.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Development/Cancellation.swift` | 40 | 已写入（未验证） | `Tools/Cove/rust/main.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Development/DesktopRun.swift` | 36 | 已写入（未验证） | `Tools/Cove/rust/main.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Development/Inspector.swift` | 92 | 已写入（未验证） | `Tools/Cove/rust/main.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Development/Session.swift` | 68 | 已写入（未验证） | `Tools/Cove/rust/main.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Entry.swift` | 159 | 已写入（未验证） | `Tools/Cove/rust/main.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Installation/Archive.swift` | 52 | 已写入（未验证） | `Tools/Cove/rust/installation.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Installation/Directories.swift` | 25 | 已写入（未验证） | `Tools/Cove/rust/installation.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Installation/Installer.swift` | 76 | 已写入（未验证） | `Tools/Cove/rust/installation.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Installation/NativeArchive.swift` | 47 | 已写入（未验证） | `Tools/Cove/rust/installation.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Installation/Receipt.swift` | 19 | 已写入（未验证） | `Tools/Cove/rust/installation.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Installation/RuntimeLibraries.swift` | 46 | 已写入（未验证） | `Tools/Cove/rust/installation.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Installation/SDKPayload.swift` | 49 | 已写入（未验证） | `Tools/Cove/rust/installation.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Installation/TarArchive.swift` | 23 | 已写入（未验证） | `Tools/Cove/rust/installation.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Installation/Update.swift` | 57 | 已写入（未验证） | `Tools/Cove/rust/installation.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Platforms/Android/APK/Assets.swift` | 38 | 已写入（未验证） | `Tools/Cove/rust/platforms_android/apk.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Platforms/Android/APK/Device.swift` | 39 | 已写入（未验证） | `Graphics/rust/metal/device.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Platforms/Android/APK/Package.swift` | 66 | 已写入（未验证） | `Tools/Cove/rust/platforms_android/apk.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Platforms/Android/APK/Signing.swift` | 38 | 已写入（未验证） | `Tools/Cove/rust/platforms_android/apk.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Platforms/Android/Compilation/Application.swift` | 41 | 已写入（未验证） | `Hosts/rust/android/sources/cove_android_app/application.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Platforms/Android/Compilation/Artifact.swift` | 17 | 已写入（未验证） | `Graphics/rust/rhi/shader_artifact.rs`<br>`Graphics/rust/rhi/shader_artifact_loading.rs`<br>`Tools/Cove/rust/project_sdk.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Platforms/Android/Compilation/NDK.swift` | 28 | 已写入（未验证） | `Hosts/rust/android/sources/cove_android_app/bridge.rs`<br>`Tools/Cove/rust/platforms_android/mod.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Platforms/Android/Compilation/NativeDependencies.swift` | 37 | 已写入（未验证） | `Tools/Cove/rust/platforms_android/mod.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Platforms/Android/Compilation/Options.swift` | 32 | 已写入（未验证） | `Compose/rust/primitives/layout/sidebar/adaptive_options.rs`<br>`Compose/rust/primitives/layout/sidebar/resizing.rs`<br>`Compose/rust/primitives/overlays/modal/modal_options.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Platforms/Android/Compilation/SDKOverlay.swift` | 71 | 已写入（未验证） | `Compose/rust/primitives/data/chart/overlay.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Platforms/Android/Compilation/Toolchain.swift` | 102 | 已写入（未验证） | `Tools/Cove/rust/project.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Platforms/Desktop/Build.swift` | 57 | 已写入（未验证） | `Core/rust/build.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Platforms/Desktop/Bundle.swift` | 46 | 已写入（未验证） | `Compose/rust/primitives/media/ui_icon_catalog.rs`<br>`Compose/rust/resource_bundle.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Platforms/Desktop/Launcher.swift` | 27 | 已写入（未验证） | `Tools/Cove/rust/platforms_desktop/mod.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Platforms/Desktop/NativeResources.swift` | 78 | 已写入（未验证） | `Graphics/rust/dx12/resources.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Platforms/iOS/ApplicationBuild.swift` | 66 | 已写入（未验证） | `Core/rust/build.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Project/Configuration.swift` | 131 | 已写入（未验证） | `Tools/Cove/rust/project.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Project/Create.swift` | 39 | 已写入（未验证） | `Tools/Cove/rust/project.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Project/Identity.swift` | 16 | 已写入（未验证） | `Tools/Cove/rust/project.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Project/SDK/Artifacts.swift` | 82 | 已写入（未验证） | `Tools/Cove/rust/project_sdk.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Project/SDK/Command.swift` | 68 | 已写入（未验证） | `Tools/Cove/rust/project_sdk.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Project/SDK/Download.swift` | 21 | 已写入（未验证） | `Tools/Cove/rust/project_sdk.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Project/SDK/Location.swift` | 74 | 已写入（未验证） | `Tools/Cove/rust/project_sdk.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Project/SDK/Release.swift` | 43 | 已写入（未验证） | `Tools/Cove/rust/project_sdk.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Project/SDK/Store.swift` | 70 | 已写入（未验证） | `Tools/Cove/rust/project_sdk.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Project/Tasks/Dependencies.swift` | 51 | 已写入（未验证） | `Tools/Cove/rust/project_tasks.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Project/Tasks/Runner.swift` | 93 | 已写入（未验证） | `Tools/Cove/rust/project_tasks.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Project/Tasks/Workspace.swift` | 112 | 已写入（未验证） | `Tools/Cove/rust/project_tasks.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Project/Templates.swift` | 14 | 已写入（未验证） | `Tools/Cove/rust/project.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Project/Testing.swift` | 59 | 已写入（未验证） | `Tools/Cove/rust/project_tasks.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Project/Version.swift` | 20 | 已写入（未验证） | `Tools/Cove/rust/project.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Support/DeveloperTool.swift` | 30 | 已写入（未验证） | `Tools/Cove/rust/support.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Support/Files.swift` | 70 | 已写入（未验证） | `Tools/Cove/rust/support.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Support/Process.swift` | 76 | 已写入（未验证） | `Tools/Cove/rust/support.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Support/ProcessStream.swift` | 42 | 已写入（未验证） | `Tools/Cove/rust/support.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Toolchains/Android.swift` | 29 | 已写入（未验证） | `Tools/Cove/rust/installation.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Toolchains/Native.swift` | 45 | 已写入（未验证） | `Tools/Cove/rust/installation.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Toolchains/Requirements.swift` | 50 | 已写入（未验证） | `Tools/Cove/rust/installation.rs` | 未编译、未行为对照 |
| `Tools/Cove/Sources/CoveCLI/Toolchains/Setup.swift` | 30 | 已写入（未验证） | `Tools/Cove/rust/installation.rs` | 未编译、未行为对照 |
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

共 61 个文件：已写入 28，进行中 0，未开始 33。

| 源文件 | 行 | 状态 | Rust/C 目标 | 依赖或阻塞 |
| --- | ---: | --- | --- | --- |
| `Examples/Counter/Sources/CounterExample/CounterView.swift` | 200 | 已写入（未验证） | `Examples/rust/counter.rs` | 未编译、未行为对照 |
| `Examples/Mobile/Platforms/iOS/main.swift` | 4 | 已写入（未验证） | `Examples/RustCounter/src/main.rs` | 未编译、未行为对照 |
| `Examples/Mobile/Sources/AppContent/App.swift` | 33 | 已写入（未验证） | `Compose/Composition/rust/around_applying_modifier.rs`<br>`Compose/Composition/rust/view_modifier.rs` | 未编译、未行为对照 |
| `Examples/Mobile/Sources/AppContent/Model.swift` | 30 | 已写入（未验证） | `Compose/rust/primitives/data/chart/model.rs` | 未编译、未行为对照 |
| `Examples/Mobile/Sources/AppContent/Projects.swift` | 57 | 未开始 | — | 等待实现 |
| `Examples/Mobile/Sources/AppContent/Settings.swift` | 50 | 未开始 | — | 等待实现 |
| `Examples/Mobile/Sources/AppContent/Today.swift` | 49 | 已写入（未验证） | `Examples/rust/mobile/today.rs` | 未编译、未行为对照 |
| `Examples/Platforms/iOS/main.swift` | 10 | 已写入（未验证） | `Examples/RustCounter/src/main.rs` | 未编译、未行为对照 |
| `Examples/Sources/CoveUIDemo/Launch.swift` | 79 | 已写入（未验证） | `Hosts/rust/android/contract/launch.rs` | 未编译、未行为对照 |
| `Examples/Sources/CoveUIDemo/main.swift` | 7 | 已写入（未验证） | `Examples/RustCounter/src/main.rs` | 未编译、未行为对照 |
| `Examples/Sources/GalleryContent/CounterContent.swift` | 18 | 已写入（未验证） | `Examples/rust/gallery_content/counter_content.rs` | 未编译、未行为对照 |
| `Examples/Sources/GalleryContent/GalleryCatalog.swift` | 150 | 已写入（未验证） | `Examples/rust/gallery_content/catalog.rs` | 未编译、未行为对照 |
| `Examples/Sources/GalleryContent/GalleryView.swift` | 218 | 已写入（未验证） | `Compose/Composition/rust/view.rs` | 未编译、未行为对照 |
| `Examples/Sources/GalleryContent/NavigationIcons.swift` | 105 | 已写入（未验证） | `Examples/rust/gallery_content/icons.rs` | 未编译、未行为对照 |
| `Examples/Sources/GalleryContent/Stories/Controls/ControlStories.swift` | 380 | 已写入（未验证） | `Examples/rust/gallery_content/stories/controls.rs` | 未编译、未行为对照 |
| `Examples/Sources/GalleryContent/Stories/Controls/StepperStory.swift` | 36 | 已写入（未验证） | `Examples/rust/gallery_content/stories/stepper.rs` | 未编译、未行为对照 |
| `Examples/Sources/GalleryContent/Stories/Controls/TextEditorStory.swift` | 327 | 未开始 | — | 等待实现 |
| `Examples/Sources/GalleryContent/Stories/Controls/TimePickerStory.swift` | 95 | 未开始 | — | 等待实现 |
| `Examples/Sources/GalleryContent/Stories/Controls/ToolbarStory.swift` | 71 | 未开始 | — | 等待实现 |
| `Examples/Sources/GalleryContent/Stories/Controls/WheelPickerStory.swift` | 104 | 未开始 | — | 等待实现 |
| `Examples/Sources/GalleryContent/Stories/Data/AdaptiveTables.swift` | 19 | 未开始 | — | 等待实现 |
| `Examples/Sources/GalleryContent/Stories/Data/ChartExploration.swift` | 37 | 未开始 | — | 等待实现 |
| `Examples/Sources/GalleryContent/Stories/Data/DataStories.swift` | 714 | 未开始 | — | 等待实现 |
| `Examples/Sources/GalleryContent/Stories/Data/DataTableStory.swift` | 131 | 未开始 | — | 等待实现 |
| `Examples/Sources/GalleryContent/Stories/Data/DateTableStories.swift` | 114 | 已写入（未验证） | `Examples/rust/gallery_content/stories/calendar.rs` | 未编译、未行为对照 |
| `Examples/Sources/GalleryContent/Stories/Data/MarkdownStory.swift` | 77 | 未开始 | — | 等待实现 |
| `Examples/Sources/GalleryContent/Stories/Data/RichTextStory.swift` | 58 | 已写入（未验证） | `Examples/rust/gallery_content/stories/rich_text.rs` | 未编译、未行为对照 |
| `Examples/Sources/GalleryContent/Stories/Data/WebStory.swift` | 193 | 未开始 | — | 等待实现 |
| `Examples/Sources/GalleryContent/Stories/Feedback/AvatarStory.swift` | 86 | 未开始 | — | 等待实现 |
| `Examples/Sources/GalleryContent/Stories/Feedback/FeedbackStories.swift` | 250 | 已写入（未验证） | `Examples/rust/gallery_content/routing.rs` | 未编译、未行为对照 |
| `Examples/Sources/GalleryContent/Stories/Feedback/RatingSample.swift` | 48 | 未开始 | — | 等待实现 |
| `Examples/Sources/GalleryContent/Stories/Feedback/RatingStory.swift` | 77 | 未开始 | — | 等待实现 |
| `Examples/Sources/GalleryContent/Stories/Feedback/SkeletonStory.swift` | 43 | 未开始 | — | 等待实现 |
| `Examples/Sources/GalleryContent/Stories/Foundation/AdaptiveExamples.swift` | 551 | 未开始 | — | 等待实现 |
| `Examples/Sources/GalleryContent/Stories/Foundation/AnimationCurves.swift` | 51 | 已写入（未验证） | `Examples/rust/gallery_content/stories/animation_curves.rs` | 未编译、未行为对照 |
| `Examples/Sources/GalleryContent/Stories/Foundation/AnimationSystem.swift` | 81 | 已写入（未验证） | `Examples/rust/gallery_content/stories/animation_system.rs` | 未编译、未行为对照 |
| `Examples/Sources/GalleryContent/Stories/Foundation/IntrinsicFitting.swift` | 35 | 已写入（未验证） | `Examples/rust/gallery_content/stories/intrinsic_fitting.rs` | 未编译、未行为对照 |
| `Examples/Sources/GalleryContent/Stories/Foundation/SurfaceTraits.swift` | 70 | 已写入（未验证） | `Examples/rust/gallery_content/stories/viewport_traits.rs` | 未编译、未行为对照 |
| `Examples/Sources/GalleryContent/Stories/Foundation/ThemeStory.swift` | 89 | 未开始 | — | 等待实现 |
| `Examples/Sources/GalleryContent/Stories/Layout/DrawerStory.swift` | 56 | 未开始 | — | 等待实现 |
| `Examples/Sources/GalleryContent/Stories/Layout/FoundationExamples.swift` | 134 | 已写入（未验证） | `Examples/rust/gallery_content/stories/layout.rs` | 未编译、未行为对照 |
| `Examples/Sources/GalleryContent/Stories/Layout/GridColumns.swift` | 53 | 已写入（未验证） | `Compose/rust/primitives/layout/grid/columns.rs` | 未编译、未行为对照 |
| `Examples/Sources/GalleryContent/Stories/Layout/ImageStory.swift` | 80 | 未开始 | — | 等待实现 |
| `Examples/Sources/GalleryContent/Stories/Layout/LayoutStories.swift` | 396 | 已写入（未验证） | `Examples/rust/gallery_content/stories/animation.rs` | 未编译、未行为对照 |
| `Examples/Sources/GalleryContent/Stories/Layout/MasonryStory.swift` | 81 | 已写入（未验证） | `Examples/rust/gallery_content/stories/layout.rs` | 未编译、未行为对照 |
| `Examples/Sources/GalleryContent/Stories/Layout/NavigationStories.swift` | 291 | 未开始 | — | 等待实现 |
| `Examples/Sources/GalleryContent/Stories/Layout/NestedSidebarExample.swift` | 56 | 未开始 | — | 等待实现 |
| `Examples/Sources/GalleryContent/Stories/Layout/OverlayStories.swift` | 199 | 未开始 | — | 等待实现 |
| `Examples/Sources/GalleryContent/Stories/Layout/ProductStories.swift` | 152 | 未开始 | — | 等待实现 |
| `Examples/Sources/GalleryContent/Stories/Layout/RailStory.swift` | 84 | 未开始 | — | 等待实现 |
| `Examples/Sources/GalleryContent/Stories/Layout/ResizableSidebarExample.swift` | 31 | 未开始 | — | 等待实现 |
| `Examples/Sources/GalleryContent/Stories/Layout/SidebarStory.swift` | 118 | 未开始 | — | 等待实现 |
| `Examples/Sources/GalleryContent/Stories/Navigation/AppBarStory.swift` | 53 | 未开始 | — | 等待实现 |
| `Examples/Sources/GalleryContent/Stories/Overlays/ActionSheetStory.swift` | 75 | 未开始 | — | 等待实现 |
| `Examples/Sources/GalleryContent/Stories/Overlays/CommandStory.swift` | 67 | 未开始 | — | 等待实现 |
| `Examples/Sources/GalleryContent/Stories/Overlays/HoverStory.swift` | 50 | 未开始 | — | 等待实现 |
| `Examples/Sources/GalleryContent/Stories/Overlays/Popconfirm/PlacementExample.swift` | 73 | 未开始 | — | 等待实现 |
| `Examples/Sources/GalleryContent/Stories/Overlays/PopconfirmStory.swift` | 74 | 未开始 | — | 等待实现 |
| `Examples/Sources/GalleryContent/Stories/Overview/Dashboard.swift` | 105 | 已写入（未验证） | `Examples/rust/gallery_content/stories/overview.rs` | 未编译、未行为对照 |
| `Examples/Sources/GalleryContent/Stories/Overview/MetricCard.swift` | 49 | 已写入（未验证） | `Examples/rust/gallery_content/stories/overview.rs` | 未编译、未行为对照 |
| `Examples/Sources/PetDemo/main.swift` | 111 | 已写入（未验证） | `Examples/RustCounter/src/main.rs` | 未编译、未行为对照 |

### DevTools

共 51 个文件：已写入 41，进行中 0，未开始 10。

| 源文件 | 行 | 状态 | Rust/C 目标 | 依赖或阻塞 |
| --- | ---: | --- | --- | --- |
| `DevTools/Inspection/Sources/CoveUIAgentCore/Contracts/Schema.swift` | 479 | 已写入（未验证） | `DevTools/rust/lib.rs` | 未编译、未行为对照 |
| `DevTools/Inspection/Sources/CoveUIAgentCore/Contracts/Tool.swift` | 51 | 已写入（未验证） | `DevTools/rust/inspection/tools.rs` | 未编译、未行为对照 |
| `DevTools/Inspection/Sources/CoveUIAgentCore/MCP/Server.swift` | 105 | 已写入（未验证） | `DevTools/rust/devtools/mod.rs` | 未编译、未行为对照 |
| `DevTools/Inspection/Sources/CoveUIAgentCore/Session/Conversation.swift` | 86 | 未开始 | — | 等待实现 |
| `DevTools/Inspection/Sources/CoveUIAgentCore/Skills/Directory.swift` | 49 | 未开始 | — | 等待实现 |
| `DevTools/Inspection/Sources/CoveUIAgentCore/Skills/Skill.swift` | 42 | 未开始 | — | 等待实现 |
| `DevTools/Inspection/Sources/CoveUIAgentCore/Tools/Diagnostics.swift` | 92 | 已写入（未验证） | `DevTools/rust/inspection/diagnostics.rs` | 未编译、未行为对照 |
| `DevTools/Inspection/Sources/CoveUIAgentCore/Tools/Toolbox.swift` | 44 | 未开始 | — | 等待实现 |
| `DevTools/Inspection/Sources/CoveUIDevToolsScene/Diagnostics/Actions.swift` | 41 | 已写入（未验证） | `DevTools/rust/inspection/actions.rs` | 未编译、未行为对照 |
| `DevTools/Inspection/Sources/CoveUIDevToolsScene/Diagnostics/Geometry.swift` | 72 | 已写入（未验证） | `Compose/rust/primitives/controls/fields/slider/geometry.rs` | 未编译、未行为对照 |
| `DevTools/Inspection/Sources/CoveUIDevToolsScene/Diagnostics/Memory.swift` | 11 | 已写入（未验证） | `Hosts/rust/desktop/application/memory.rs` | 未编译、未行为对照 |
| `DevTools/Inspection/Sources/CoveUIDevToolsScene/Diagnostics/Presentation.swift` | 62 | 已写入（未验证） | `Graphics/rust/rhi/presentation.rs` | 未编译、未行为对照 |
| `DevTools/Inspection/Sources/CoveUIDevToolsScene/Diagnostics/Queries.swift` | 200 | 未开始 | — | 等待实现 |
| `DevTools/Inspection/Sources/CoveUIDevToolsScene/Diagnostics/Requests.swift` | 50 | 未开始 | — | 等待实现 |
| `DevTools/Inspection/Sources/CoveUIDevToolsScene/Diagnostics/Tools.swift` | 222 | 已写入（未验证） | `DevTools/rust/inspection/tools.rs` | 未编译、未行为对照 |
| `DevTools/Inspection/Sources/CoveUIDevToolsScene/Diagnostics/Trace.swift` | 103 | 已写入（未验证） | `Core/rust/scene/timing_trace.rs` | 未编译、未行为对照 |
| `DevTools/Inspection/Sources/CoveUIDevToolsScene/Observation.swift` | 19 | 已写入（未验证） | `Logging/rust/observation.rs` | 未编译、未行为对照 |
| `DevTools/Inspection/Sources/CoveUIDevToolsScene/SceneEditor.swift` | 209 | 已写入（未验证） | `DevTools/rust/inspection/editor.rs` | 未编译、未行为对照 |
| `DevTools/Inspection/Sources/CoveUIDevToolsScene/SceneInspector.swift` | 177 | 已写入（未验证） | `DevTools/rust/inspection/inspector.rs` | 未编译、未行为对照 |
| `DevTools/Protocol/Sources/CoveUIDevToolsProtocol/ComponentAnalysis.swift` | 25 | 已写入（未验证） | `DevTools/Protocol/rust/lib.rs` | 未编译、未行为对照 |
| `DevTools/Protocol/Sources/CoveUIDevToolsProtocol/DevToolsSession.swift` | 104 | 已写入（未验证） | `DevTools/Protocol/rust/session.rs` | 未编译、未行为对照 |
| `DevTools/Protocol/Sources/CoveUIDevToolsProtocol/Discovery.swift` | 22 | 已写入（未验证） | `DevTools/Protocol/rust/lib.rs` | 未编译、未行为对照 |
| `DevTools/Protocol/Sources/CoveUIDevToolsProtocol/Exports.swift` | 1 | 已写入（未验证） | `DevTools/Protocol/rust/lib.rs` | 未编译、未行为对照 |
| `DevTools/Protocol/Sources/CoveUIDevToolsProtocol/InputRecording.swift` | 75 | 已写入（未验证） | `DevTools/Protocol/rust/recording.rs` | 未编译、未行为对照 |
| `DevTools/Protocol/Sources/CoveUIDevToolsProtocol/Inspection.swift` | 86 | 已写入（未验证） | `DevTools/Protocol/rust/lib.rs` | 未编译、未行为对照 |
| `DevTools/Protocol/Sources/CoveUIDevToolsProtocol/Observation.swift` | 57 | 已写入（未验证） | `DevTools/Protocol/rust/lib.rs` | 未编译、未行为对照 |
| `DevTools/Protocol/Sources/CoveUIDevToolsProtocol/Protocol.swift` | 486 | 已写入（未验证） | `DevTools/Protocol/rust/lib.rs` | 未编译、未行为对照 |
| `DevTools/Sources/CoveUIAgent/Exports.swift` | 3 | 已写入（未验证） | `Logging/rust/exports.rs` | 未编译、未行为对照 |
| `DevTools/Sources/CoveUIAgentTransport/LocalServer.swift` | 110 | 未开始 | — | 等待实现 |
| `DevTools/Sources/CoveUIDevTools/Integration/Exports.swift` | 4 | 已写入（未验证） | `Logging/rust/exports.rs` | 未编译、未行为对照 |
| `DevTools/Sources/CoveUIDevTools/Mirror/NativeAppFrameCapture.swift` | 52 | 已写入（未验证） | `Compose/rust/modifiers/layout/frame_capture.rs` | 未编译、未行为对照 |
| `DevTools/Sources/CoveUIDevTools/Session/DevTools.swift` | 230 | 已写入（未验证） | `DevTools/rust/devtools/mod.rs` | 未编译、未行为对照 |
| `DevTools/Sources/CoveUIDevTools/Session/Discovery.swift` | 19 | 未开始 | — | 等待实现 |
| `DevTools/Sources/CoveUIDevTools/Taps/FrameTap.swift` | 276 | 已写入（未验证） | `DevTools/rust/devtools/mirror.rs` | 未编译、未行为对照 |
| `DevTools/Sources/CoveUIDevTools/Taps/InputBridge.swift` | 151 | 已写入（未验证） | `DevTools/rust/devtools/mirror.rs` | 未编译、未行为对照 |
| `DevTools/Sources/CoveUIDevTools/Taps/TimingPublisher.swift` | 45 | 已写入（未验证） | `DevTools/rust/devtools/mirror.rs` | 未编译、未行为对照 |
| `DevTools/Sources/CoveUIDevToolsProbe/main.swift` | 72 | 已写入（未验证） | `Examples/RustCounter/src/main.rs` | 未编译、未行为对照 |
| `DevTools/Sources/CoveUIDevToolsServer/Requests/Checkpoint.swift` | 47 | 已写入（未验证） | `Hosts/rust/desktop/diagnostics/checkpoint.rs` | 未编译、未行为对照 |
| `DevTools/Sources/CoveUIDevToolsServer/Requests/Diagnostics.swift` | 17 | 已写入（未验证） | `DevTools/rust/inspection/diagnostics.rs` | 未编译、未行为对照 |
| `DevTools/Sources/CoveUIDevToolsServer/Requests/Mirror.swift` | 79 | 已写入（未验证） | `DevTools/rust/devtools/mirror.rs` | 未编译、未行为对照 |
| `DevTools/Sources/CoveUIDevToolsServer/Requests/Observation.swift` | 45 | 已写入（未验证） | `Logging/rust/observation.rs` | 未编译、未行为对照 |
| `DevTools/Sources/CoveUIDevToolsServer/Requests/Recording.swift` | 47 | 已写入（未验证） | `DevTools/Protocol/rust/recording.rs` | 未编译、未行为对照 |
| `DevTools/Sources/CoveUIDevToolsServer/Requests/Scene.swift` | 90 | 已写入（未验证） | `Hosts/rust/android/integration/scene.rs` | 未编译、未行为对照 |
| `DevTools/Sources/CoveUIDevToolsServer/Requests/Window.swift` | 21 | 已写入（未验证） | `Compose/rust/primitives/overlays/toast/controller.rs`<br>`Compose/rust/theme/styles/circular_appearance.rs` | 未编译、未行为对照 |
| `DevTools/Sources/CoveUIDevToolsServer/Session/Configuration.swift` | 69 | 已写入（未验证） | `Hosts/rust/desktop/application/configuration.rs` | 未编译、未行为对照 |
| `DevTools/Sources/CoveUIDevToolsServer/Session/DevServer.swift` | 454 | 已写入（未验证） | `DevTools/rust/server/mod.rs` | 未编译、未行为对照 |
| `DevTools/Sources/CoveUIDevToolsServer/Session/Host.swift` | 53 | 已写入（未验证） | `Compose/rust/primitives/controls/fields/slider/host.rs` | 未编译、未行为对照 |
| `DevTools/Sources/CoveUIDevToolsServer/Streams/Logs.swift` | 55 | 未开始 | — | 等待实现 |
| `DevTools/Sources/CoveUIDevToolsServer/Transport/DevToolsTransport.swift` | 24 | 已写入（未验证） | `DevTools/rust/server/transport.rs` | 未编译、未行为对照 |
| `DevTools/Sources/CoveUIDevToolsServer/Transport/NIODevToolsTransport.swift` | 190 | 已写入（未验证） | `DevTools/rust/server/transport.rs` | 未编译、未行为对照 |
| `DevTools/Sources/CoveUIDevToolsServer/Tree/TreeEncodingWorker.swift` | 60 | 未开始 | — | 等待实现 |

### Hosts

共 51 个文件：已写入 50，进行中 0，未开始 1。

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
| `Hosts/Android/Sources/CoveAndroidApp/Application.swift` | 39 | 已写入（未验证） | `Hosts/rust/android/sources/cove_android_app/application.rs` | 未编译、未行为对照 |
| `Hosts/Android/Sources/CoveAndroidApp/Bridge.swift` | 185 | 已写入（未验证） | `Hosts/rust/android/sources/cove_android_app/bridge.rs` | 未编译、未行为对照 |
| `Hosts/Android/Sources/CoveAndroidApp/Content/InputSnapshot.swift` | 17 | 已写入（未验证） | `Hosts/rust/android/sources/cove_android_app/content/inspection.rs` | 未编译、未行为对照 |
| `Hosts/Android/Sources/CoveAndroidApp/Content/Inspection.swift` | 29 | 已写入（未验证） | `Hosts/rust/android/sources/cove_android_app/content/inspection.rs` | 未编译、未行为对照 |
| `Hosts/Android/Sources/CoveAndroidApp/Content/Scene.swift` | 157 | 已写入（未验证） | `Hosts/rust/android/sources/cove_android_app/content/scene.rs` | 未编译、未行为对照 |
| `Hosts/Android/Sources/CoveAndroidApp/Frame/GPUTimings.swift` | 85 | 已写入（未验证） | `Hosts/rust/android/sources/cove_android_app/frame/gpu_timings.rs` | 未编译、未行为对照 |
| `Hosts/Android/Sources/CoveAndroidApp/Frame/RuntimeOptions.swift` | 29 | 已写入（未验证） | `Hosts/rust/android/sources/cove_android_app/frame/runtime_options.rs` | 未编译、未行为对照 |
| `Hosts/Android/Sources/CoveAndroidApp/Frame/Timings.swift` | 46 | 已写入（未验证） | `Hosts/rust/android/sources/cove_android_app/frame/timings.rs` | 未编译、未行为对照 |
| `Hosts/Android/Sources/CoveAndroidApp/SurfaceSession.swift` | 92 | 已写入（未验证） | `Hosts/rust/android/sources/cove_android_app/surface_session.rs` | 未编译、未行为对照 |
| `Hosts/Android/Sources/CoveAndroidApp/Validation/Counter/Typography.swift` | 43 | 已写入（未验证） | `Hosts/rust/android/sources/cove_android_app/validation/counter/typography.rs` | 未编译、未行为对照 |
| `Hosts/Android/Sources/CoveAndroidApp/Validation/Showcase.swift` | 37 | 已写入（未验证） | `Hosts/rust/android/sources/cove_android_app/validation/showcase.rs` | 未编译、未行为对照 |
| `Hosts/Android/Sources/CoveAndroidProbe/Layout.swift` | 20 | 已写入（未验证） | `Hosts/rust/android/probe/layout.rs` | 未编译、未行为对照 |
| `Hosts/Android/Sources/CoveAndroidProbe/Raster.swift` | 58 | 已写入（未验证） | `Hosts/rust/android/probe/raster.rs` | 未编译、未行为对照 |
| `Hosts/Android/Sources/CoveAndroidProbe/Runner.swift` | 55 | 已写入（未验证） | `Hosts/rust/android/probe/runner.rs` | 未编译、未行为对照 |
| `Hosts/Android/Sources/CoveAndroidProbe/Transfer.swift` | 43 | 已写入（未验证） | `Hosts/rust/android/probe/transfer.rs` | 未编译、未行为对照 |
| `Hosts/Android/Sources/CoveAndroidProbe/UI.swift` | 50 | 已写入（未验证） | `Hosts/rust/android/probe/ui.rs` | 未编译、未行为对照 |
| `Hosts/Android/Sources/CoveAndroidProbe/main.swift` | 45 | 已写入（未验证） | `Hosts/rust/android/probe/main.rs` | 未编译、未行为对照 |
| `Hosts/Desktop/Sources/CoveUI/Application/AdaptiveContent.swift` | 15 | 已写入（未验证） | `Hosts/rust/desktop/application/typography.rs` | 未编译、未行为对照 |
| `Hosts/Desktop/Sources/CoveUI/Application/Callbacks.swift` | 10 | 未开始 | — | 等待实现 |
| `Hosts/Desktop/Sources/CoveUI/Application/Composition.swift` | 22 | 已写入（未验证） | `Hosts/rust/desktop/application/composition.rs` | 未编译、未行为对照 |
| `Hosts/Desktop/Sources/CoveUI/Application/Configuration.swift` | 100 | 已写入（未验证） | `Hosts/rust/desktop/application/configuration.rs` | 未编译、未行为对照 |
| `Hosts/Desktop/Sources/CoveUI/Application/Memory.swift` | 94 | 已写入（未验证） | `Hosts/rust/desktop/application/memory.rs` | 未编译、未行为对照 |
| `Hosts/Desktop/Sources/CoveUI/Application/MemoryPressureProbe.swift` | 69 | 已写入（未验证） | `Hosts/rust/desktop/application/memory.rs` | 未编译、未行为对照 |
| `Hosts/Desktop/Sources/CoveUI/Application/Runtime.swift` | 911 | 已写入（未验证） | `Hosts/rust/desktop/application/runtime.rs` | 未编译、未行为对照 |
| `Hosts/Desktop/Sources/CoveUI/Application/Services.swift` | 31 | 已写入（未验证） | `Hosts/rust/desktop/application/services.rs` | 未编译、未行为对照 |
| `Hosts/Desktop/Sources/CoveUI/Application/Typography.swift` | 7 | 已写入（未验证） | `Hosts/rust/desktop/application/typography.rs` | 未编译、未行为对照 |
| `Hosts/Desktop/Sources/CoveUI/Diagnostics/Checkpoint.swift` | 13 | 已写入（未验证） | `Hosts/rust/desktop/diagnostics/checkpoint.rs` | 未编译、未行为对照 |
| `Hosts/Desktop/Sources/CoveUI/Diagnostics/FrameMetrics.swift` | 44 | 已写入（未验证） | `Hosts/rust/desktop/diagnostics/frame_metrics.rs` | 未编译、未行为对照 |
| `Hosts/Desktop/Sources/CoveUI/Diagnostics/PerformanceHUD.swift` | 30 | 已写入（未验证） | `Hosts/rust/desktop/diagnostics/performance_hud.rs` | 未编译、未行为对照 |
| `Hosts/Desktop/Sources/CoveUI/Diagnostics/Recording.swift` | 40 | 已写入（未验证） | `Hosts/rust/desktop/diagnostics/recording.rs` | 未编译、未行为对照 |
| `Hosts/Desktop/Sources/CoveUI/Diagnostics/Session.swift` | 7 | 已写入（未验证） | `Hosts/rust/desktop/diagnostics/session.rs` | 未编译、未行为对照 |
| `Hosts/Desktop/Sources/CoveUI/Display/Controls.swift` | 30 | 已写入（未验证） | `Hosts/rust/desktop/display/controls.rs` | 未编译、未行为对照 |
| `Hosts/Desktop/Sources/CoveUI/Display/Handle.swift` | 303 | 已写入（未验证） | `Hosts/rust/desktop/display/handle.rs` | 未编译、未行为对照 |
| `Hosts/Desktop/Sources/CoveUI/Exports.swift` | 7 | 已写入（未验证） | `Logging/rust/exports.rs` | 未编译、未行为对照 |
| `Hosts/Desktop/Sources/CoveUI/Rendering/Context.swift` | 45 | 已写入（未验证） | `Hosts/rust/desktop/rendering/context.rs` | 未编译、未行为对照 |
| `Hosts/Desktop/Sources/CoveUI/ResourcesSupport/Bundle.swift` | 9 | 已写入（未验证） | `Compose/rust/primitives/media/ui_icon_catalog.rs`<br>`Compose/rust/resource_bundle.rs` | 未编译、未行为对照 |
| `Hosts/Desktop/Sources/CoveUI/ResourcesSupport/Lookup.swift` | 6 | 已写入（未验证） | `Hosts/rust/desktop/resources_support/mod.rs` | 未编译、未行为对照 |
| `Hosts/Desktop/Sources/CoveUI/Window/Auxiliary.swift` | 202 | 已写入（未验证） | `Hosts/rust/desktop/window/auxiliary.rs` | 未编译、未行为对照 |
| `Hosts/Desktop/Sources/CoveUI/Window/NativeMenu.swift` | 228 | 已写入（未验证） | `Hosts/rust/desktop/window/native_menu.rs` | 未编译、未行为对照 |
| `Hosts/Desktop/Sources/CoveUI/Window/NativeRenderer.swift` | 115 | 已写入（未验证） | `Hosts/rust/desktop/window/native_renderer.rs` | 未编译、未行为对照 |
| `Hosts/Desktop/Sources/CoveUI/Window/Renderer.swift` | 15 | 已写入（未验证） | `Core/rust/scene/layer_aware_node_renderer.rs`<br>`Core/rust/scene/node_renderer.rs`<br>`Hosts/rust/desktop/window/native_renderer.rs` | 未编译、未行为对照 |
| `Hosts/Desktop/Sources/CoveUI/Window/State.swift` | 15 | 已写入（未验证） | `Hosts/rust/desktop/window/state.rs` | 未编译、未行为对照 |
| `Hosts/Desktop/Sources/CoveUI/Window/WindowChrome.swift` | 397 | 已写入（未验证） | `Hosts/rust/desktop/window/chrome.rs` | 未编译、未行为对照 |

### Terminal

共 29 个文件：已写入 28，进行中 0，未开始 1。

| 源文件 | 行 | 状态 | Rust/C 目标 | 依赖或阻塞 |
| --- | ---: | --- | --- | --- |
| `Terminal/Sources/CoveUITerminal/Engine/Engine.swift` | 202 | 已写入（未验证） | `Compose/rust/primitives/text/text_field/text_field_layout_engine.rs`<br>`Sources/rust/webview/mod.rs` | 未编译、未行为对照 |
| `Terminal/Sources/CoveUITerminal/Engine/Frame.swift` | 100 | 已写入（未验证） | `Terminal/rust/engine/frame.rs` | 未编译、未行为对照 |
| `Terminal/Sources/CoveUITerminal/Engine/FrameReader.swift` | 140 | 已写入（未验证） | `Terminal/rust/engine/frame_reader.rs` | 未编译、未行为对照 |
| `Terminal/Sources/CoveUITerminal/Engine/Hyperlink.swift` | 18 | 已写入（未验证） | `Terminal/rust/engine/hyperlink.rs` | 未编译、未行为对照 |
| `Terminal/Sources/CoveUITerminal/Engine/Images.swift` | 136 | 已写入（未验证） | `Terminal/rust/engine/images.rs` | 未编译、未行为对照 |
| `Terminal/Sources/CoveUITerminal/Engine/KeyEncoder.swift` | 123 | 已写入（未验证） | `Terminal/rust/engine/key_encoder.rs` | 未编译、未行为对照 |
| `Terminal/Sources/CoveUITerminal/Engine/Mouse.swift` | 94 | 已写入（未验证） | `Terminal/rust/engine/mouse.rs` | 未编译、未行为对照 |
| `Terminal/Sources/CoveUITerminal/Engine/PlaceholderDiacritics.swift` | 30 | 已写入（未验证） | `Terminal/rust/engine/placeholder_diacritics.rs` | 未编译、未行为对照 |
| `Terminal/Sources/CoveUITerminal/Engine/Placeholders.swift` | 182 | 已写入（未验证） | `Terminal/rust/engine/placeholders.rs` | 未编译、未行为对照 |
| `Terminal/Sources/CoveUITerminal/Engine/Selection.swift` | 144 | 已写入（未验证） | `Terminal/rust/engine/selection.rs` | 未编译、未行为对照 |
| `Terminal/Sources/CoveUITerminal/Engine/Sized.swift` | 7 | 未开始 | — | 等待实现 |
| `Terminal/Sources/CoveUITerminal/Process/Command.swift` | 77 | 已写入（未验证） | `Terminal/rust/process/command.rs` | 未编译、未行为对照 |
| `Terminal/Sources/CoveUITerminal/Process/ConPTY.swift` | 179 | 已写入（未验证） | `Terminal/rust/lib.rs`<br>`Terminal/rust/process/conpty.rs`<br>`Terminal/rust/process/windows_command_line.rs` | 未编译、未行为对照 |
| `Terminal/Sources/CoveUITerminal/Process/POSIX.swift` | 176 | 已写入（未验证） | `Terminal/rust/process/posix.rs` | 未编译、未行为对照 |
| `Terminal/Sources/CoveUITerminal/Process/PseudoTerminal.swift` | 83 | 已写入（未验证） | `Terminal/rust/process/mod.rs` | 未编译、未行为对照 |
| `Terminal/Sources/CoveUITerminal/Process/WindowsCommandLine.swift` | 30 | 已写入（未验证） | `Terminal/rust/process/windows_command_line.rs` | 未编译、未行为对照 |
| `Terminal/Sources/CoveUITerminal/Session.swift` | 259 | 已写入（未验证） | `DevTools/Protocol/rust/session.rs` | 未编译、未行为对照 |
| `Terminal/Sources/CoveUITerminal/View/BlinkTimer.swift` | 54 | 已写入（未验证） | `Terminal/rust/view/blink_timer.rs` | 未编译、未行为对照 |
| `Terminal/Sources/CoveUITerminal/View/BoxDrawing.swift` | 246 | 已写入（未验证） | `Terminal/rust/view/box_drawing.rs` | 未编译、未行为对照 |
| `Terminal/Sources/CoveUITerminal/View/CellMetrics.swift` | 24 | 已写入（未验证） | `Terminal/rust/view/cell_metrics.rs` | 未编译、未行为对照 |
| `Terminal/Sources/CoveUITerminal/View/ImageTextures.swift` | 27 | 已写入（未验证） | `Terminal/rust/view/image_textures.rs` | 未编译、未行为对照 |
| `Terminal/Sources/CoveUITerminal/View/KeyRouting.swift` | 71 | 已写入（未验证） | `Terminal/rust/view/key_routing.rs` | 未编译、未行为对照 |
| `Terminal/Sources/CoveUITerminal/View/LinkOpener.swift` | 26 | 已写入（未验证） | `Terminal/rust/view/link_opener.rs` | 未编译、未行为对照 |
| `Terminal/Sources/CoveUITerminal/View/Painter.swift` | 173 | 已写入（未验证） | `Core/rust/scene/rendering/scaled_painting.rs` | 未编译、未行为对照 |
| `Terminal/Sources/CoveUITerminal/View/PointerRouter.swift` | 136 | 已写入（未验证） | `Terminal/rust/view/pointer_router.rs` | 未编译、未行为对照 |
| `Terminal/Sources/CoveUITerminal/View/SurfaceState.swift` | 87 | 已写入（未验证） | `Hosts/rust/android/integration/surface/state.rs` | 未编译、未行为对照 |
| `Terminal/Sources/CoveUITerminal/View/TerminalView.swift` | 174 | 已写入（未验证） | `Terminal/rust/view/terminal_view.rs` | 未编译、未行为对照 |
| `Terminal/Sources/CoveUITerminal/View/URLDetector.swift` | 59 | 已写入（未验证） | `Terminal/rust/view/url_detector.rs` | 未编译、未行为对照 |
| `Terminal/Sources/TerminalDemo/main.swift` | 31 | 已写入（未验证） | `Examples/RustCounter/src/main.rs` | 未编译、未行为对照 |

### Sources

共 19 个文件：已写入 18，进行中 0，未开始 1。

| 源文件 | 行 | 状态 | Rust/C 目标 | 依赖或阻塞 |
| --- | ---: | --- | --- | --- |
| `Sources/CoveUIWebView/Apple/WebKitHost.swift` | 22 | 已写入（未验证） | `Compose/rust/primitives/controls/fields/slider/host.rs` | 未编译、未行为对照 |
| `Sources/CoveUIWebView/Apple/WebKitSurface.swift` | 232 | 已写入（未验证） | `Compose/Composition/rust/adaptive/surface.rs` | 未编译、未行为对照 |
| `Sources/CoveUIWebView/WebView.swift` | 170 | 已写入（未验证） | `Sources/rust/webview/view.rs` | 未编译、未行为对照 |
| `Sources/CoveUIWebView/WebViewHandle.swift` | 51 | 已写入（未验证） | `Sources/rust/webview/mod.rs` | 未编译、未行为对照 |
| `Sources/CoveUIWebView/WebViewState.swift` | 46 | 已写入（未验证） | `Sources/rust/webview/mod.rs` | 未编译、未行为对照 |
| `Sources/CoveUIWebView/WebViewSurface.swift` | 77 | 已写入（未验证） | `Sources/rust/webview/mod.rs` | 未编译、未行为对照 |
| `Sources/CoveUIWorkspace/Icons/CoveUIWorkspaceResourceBundle.swift` | 6 | 已写入（未验证） | `Compose/rust/resource_bundle.rs` | 未编译、未行为对照 |
| `Sources/CoveUIWorkspace/Icons/WorkspacePanelIconCatalog.swift` | 17 | 已写入（未验证） | `Sources/rust/workspace/icons.rs` | 未编译、未行为对照 |
| `Sources/CoveUIWorkspace/Layout/Split.swift` | 32 | 未开始 | — | 等待实现 |
| `Sources/CoveUIWorkspace/Layout/Tree.swift` | 49 | 已写入（未验证） | `Compose/Composition/rust/layout_tree.rs`<br>`Compose/Composition/rust/view_graph.rs`<br>`Compose/rust/tests/row_slot.rs` | 未编译、未行为对照 |
| `Sources/CoveUIWorkspace/Observation.swift` | 14 | 已写入（未验证） | `Logging/rust/observation.rs` | 未编译、未行为对照 |
| `Sources/CoveUIWorkspace/Sizing/Panel.swift` | 47 | 已写入（未验证） | `Compose/rust/primitives/layout/panel.rs` | 未编译、未行为对照 |
| `Sources/CoveUIWorkspace/Sizing/Resolution.swift` | 172 | 已写入（未验证） | `Core/rust/bidi.rs` | 未编译、未行为对照 |
| `Sources/CoveUIWorkspace/Theme/TabStyle.swift` | 22 | 已写入（未验证） | `Sources/rust/workspace/tab_style.rs` | 未编译、未行为对照 |
| `Sources/CoveUIWorkspace/Theme/WorkspaceRegionTransition.swift` | 179 | 已写入（未验证） | `Sources/rust/workspace/view.rs` | 未编译、未行为对照 |
| `Sources/CoveUIWorkspace/Theme/WorkspaceTheme.swift` | 51 | 已写入（未验证） | `Sources/rust/workspace/theme.rs` | 未编译、未行为对照 |
| `Sources/CoveUIWorkspace/WorkspaceController.swift` | 915 | 已写入（未验证） | `Sources/rust/workspace/controller.rs` | 未编译、未行为对照 |
| `Sources/CoveUIWorkspace/WorkspaceModel.swift` | 590 | 已写入（未验证） | `Sources/rust/workspace/model.rs` | 未编译、未行为对照 |
| `Sources/CoveUIWorkspace/WorkspaceView.swift` | 2169 | 已写入（未验证） | `Sources/rust/webview/view.rs` | 未编译、未行为对照 |

### System

共 17 个文件：已写入 17，进行中 0，未开始 0。

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
| `System/Sources/CoveProcessFixture/main.swift` | 41 | 已写入（未验证） | `Examples/RustCounter/src/main.rs` | 未编译、未行为对照 |

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

### Shell

共 13 个文件：已写入 13，进行中 0，未开始 0。

| 源文件 | 行 | 状态 | Rust/C 目标 | 依赖或阻塞 |
| --- | ---: | --- | --- | --- |
| `Shell/Sources/PlatformShell/ApplicationEntry.swift` | 34 | 已写入（未验证） | `Shell/rust/application_entry.rs` | 未编译、未行为对照 |
| `Shell/Sources/PlatformShell/Cursor.swift` | 2 | 已写入（未验证） | `Shell/rust/cursor.rs` | 未编译、未行为对照 |
| `Shell/Sources/PlatformShell/Diagnostics/SDL.swift` | 18 | 已写入（未验证） | `Shell/rust/diagnostics/mod.rs` | 未编译、未行为对照 |
| `Shell/Sources/PlatformShell/DisplayRefresh/CoreVideo.swift` | 62 | 已写入（未验证） | `Shell/rust/display_refresh/mod.rs` | 未编译、未行为对照 |
| `Shell/Sources/PlatformShell/DisplayRefresh/MacDisplayLink.swift` | 29 | 已写入（未验证） | `Shell/rust/display_refresh/mod.rs` | 未编译、未行为对照 |
| `Shell/Sources/PlatformShell/DisplayRefresh/Signal.swift` | 50 | 已写入（未验证） | `Shell/rust/display_refresh/signal.rs` | 未编译、未行为对照 |
| `Shell/Sources/PlatformShell/DisplayRefresh/WindowDisplayLink.swift` | 45 | 已写入（未验证） | `Shell/rust/display_refresh/mod.rs` | 未编译、未行为对照 |
| `Shell/Sources/PlatformShell/Log.swift` | 5 | 已写入（未验证） | `Shell/rust/log.rs` | 未编译、未行为对照 |
| `Shell/Sources/PlatformShell/NativeMenuPopover.swift` | 100 | 已写入（未验证） | `Shell/rust/native_menu_popover.rs` | 未编译、未行为对照 |
| `Shell/Sources/PlatformShell/PlatformShell.swift` | 579 | 已写入（未验证） | `Shell/rust/platform_shell.rs`<br>`Shell/rust/sdl3_shell.rs` | 未编译、未行为对照 |
| `Shell/Sources/PlatformShell/SDL3Clipboard.swift` | 22 | 已写入（未验证） | `Shell/rust/sdl3_clipboard.rs` | 未编译、未行为对照 |
| `Shell/Sources/PlatformShell/SDL3Shell.swift` | 1510 | 已写入（未验证） | `Shell/rust/sdl3_shell.rs` | 未编译、未行为对照 |
| `Shell/Sources/PlatformShell/Scroll/AppKitInput.swift` | 73 | 已写入（未验证） | `Shell/rust/scroll/mod.rs` | 未编译、未行为对照 |

### Runtime

共 11 个文件：已写入 11，进行中 0，未开始 0。

| 源文件 | 行 | 状态 | Rust/C 目标 | 依赖或阻塞 |
| --- | ---: | --- | --- | --- |
| `Runtime/Sources/CoveUIRuntime/Core/AppBridge.swift` | 34 | 已写入（未验证） | `Runtime/rust/app_bridge.rs` | 未编译、未行为对照 |
| `Runtime/Sources/CoveUIRuntime/Core/CoveUIRuntime.swift` | 17 | 已写入（未验证） | `Runtime/rust/lib.rs` | 未编译、未行为对照 |
| `Runtime/Sources/CoveUIRuntime/Core/FrameDeadline.swift` | 28 | 已写入（未验证） | `Runtime/rust/frame_deadline.rs` | 未编译、未行为对照 |
| `Runtime/Sources/CoveUIRuntime/Core/FrameDrive.swift` | 24 | 已写入（未验证） | `Runtime/rust/frame_drive.rs` | 未编译、未行为对照 |
| `Runtime/Sources/CoveUIRuntime/Core/Log.swift` | 9 | 已写入（未验证） | `Runtime/rust/log.rs` | 未编译、未行为对照 |
| `Runtime/Sources/CoveUIRuntime/Core/NativeAccessibility.swift` | 303 | 已写入（未验证） | `Runtime/rust/native_accessibility.rs` | 未编译、未行为对照 |
| `Runtime/Sources/CoveUIRuntime/Core/PlatformHost.swift` | 17 | 已写入（未验证） | `Runtime/rust/platform_host.rs` | 未编译、未行为对照 |
| `Runtime/Sources/CoveUIRuntime/Core/SDL3PlatformHost.swift` | 993 | 已写入（未验证） | `Runtime/rust/sdl3_platform_host.rs` | 未编译、未行为对照 |
| `Runtime/Sources/CoveUIRuntime/Core/iOS/DisplayDriver.swift` | 22 | 已写入（未验证） | `Runtime/rust/ios/mod.rs` | 未编译、未行为对照 |
| `Runtime/Sources/CoveUIRuntime/Exports.swift` | 4 | 已写入（未验证） | `Runtime/rust/lib.rs` | 未编译、未行为对照 |
| `Runtime/Sources/CoveUIRuntime/Rendering/SurfaceFactory.swift` | 34 | 已写入（未验证） | `Runtime/rust/surface_factory.rs` | 未编译、未行为对照 |

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

### Browser

共 1 个文件：已写入 1，进行中 0，未开始 0。

| 源文件 | 行 | 状态 | Rust/C 目标 | 依赖或阻塞 |
| --- | ---: | --- | --- | --- |
| `Browser/Sources/CoveUIBrowserPrototype/main.swift` | 400 | 已写入（未验证） | `Examples/RustCounter/src/main.rs` | 未编译、未行为对照 |

