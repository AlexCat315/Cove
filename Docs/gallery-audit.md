# Gallery 逐项验收记录

更新：2026-11-09。目录共 103 页（Numeric 合并 NumberField+Vec3Field，Text Fields 合并 TextField+SearchField+TextArea+JsonField）。当前控件页的验收以以下 Swift 对照表为准；后续旧轮记录保留其原有范围。

Swift 源码逐页对照见文末《Swift 源码对照（逐页差异清单）》。

## 2026-11-09：平台自适应 P1（弹层区段、SegmentedLayout、PostureLayout、Android 首次运行）

- 页面：Viewport Traits 页新增 Window segments 演示（`segments-single` / `segments-first` / `segments-second` / `segments-top` / `segments-bottom`）；目录仍为 103 页。用户文档 `Website/src/content/components/adaptive.md` 增加「跨折痕布局」「弹层不跨折痕」。
- 结构验证：compose `fold_overlay_tests` 10 项，core `presentations_choose_the_controls_or_leading_segment`，xtask `jni_on_load_must_be_a_defined_export`。
- 桌面实测（DevTools）：桌面显示单面板；`dual-screen` 两栏 x 330–540 与 574–1064（铰链 540–574 留空）；`fold-tabletop` 下该节整体位于折痕下方（y=684），保持单面板。
- Android 模拟器实测：`FoldingFeature` 在平放、合上、桌面支架、书本、旋转下的上报与解码正确（见 CHANGELOG）。Android 渲染、弹层与触摸**未验收**：模拟器两种 GPU 后端都无法渲染 Cove，需真机。

## 2026-11-09：平台自适应 P0（平台、设备类别、折叠姿态、铰链）

- 页面：Viewport Traits 页的 Idiom 行改为 `Device: <platform> · <design language> · <device class>`，新增 `Posture: <posture> · Segments: <n>` 行（`viewport-device`、`viewport-posture`），阈值说明改为五档。新增用户文档 `Website/src/content/components/adaptive.md` 并登记到「工具链」组；目录仍为 103 页。
- 结构验证：core `adaptive_environment` 11 项（五档阈值、平台→设计语言、平放折痕不分段、半开折痕分段、遮挡铰链移除区域并把居中对话框移入前一段、13 个预设完整、覆盖抬高安全区与 JSON 往返）；hosts `surface_metrics_tests` 新增折叠数据解码与限长、DevTools 覆盖叠加在宿主上报之上并可清除；compose `handheld_devices_present_overlays_as_sheets`；gallery Viewport Traits 文本与调试名。
- 修复与实测：桌面宿主此前把根包在 `.text_scaler(...)` 里，环境变化到不了内容，本页一直停在首帧；修复后用 DevTools 在 macOS 上实测：`resize_ui` 700×800 → `Viewport: 700 × 800 / medium`；`set_ui_environment` 的 `dual-screen`、`fold-tabletop`、`fold-book`、`clear` 分别得到 `foldable-dual-screen · Segments: 2`、`tabletop · Segments: 2`、`book · Segments: 2`、`macos · cupertino · desktop`。
- 原生验收：**未做**。Android `FoldingFeature`（需折叠屏模拟器或真机）、iPad `SDL_IsTablet`、Windows 平板模式均未实测；本机无 Android SDK，Java 未编译。`set_ui_environment` 只替换上报事实，不能验证宿主数据源。

## 2026-11-09：移动端能力补齐与文档修复

- 页面：新增用户文档 `Website/src/content/components/mobile.md`（移动端支持）并登记到 `Website/src/utils/docs.ts`「工具链」组；未新增 Gallery 控件页，目录仍为 103 页。
- 结构验证：`app_storage` 的 iOS/Android/桌面目录策略纯 Rust 单测（`crates/compose/src/app_storage.rs`）；`cove-system` 移动端 `ProcessError::Unavailable` 守卫单测（`crates/system/src/command.rs`）；iOS `Info.plist` 方向键单测（`xtask/src/apple.rs`）；`Shell` 软键盘 API 默认无副作用单测（`crates/runtime/tests/platform_bridge_tests.rs`）；`cargo check --all-targets` 与全 workspace clippy / fmt / 测试通过。
- 原生验收：**未做**。iOS 键盘高度与软键盘显隐、SDL3 UIKit 的 IME 组合预览、Android `filesDir` 持久化、屏幕方向、进程不可用行为均未在设备或模拟器实测；DevTools 文本注入不能验证原生键盘/IME 激活。
- 未完成：iOS 独立宿主与 `UITextInput`、原生 IME 组合预览、Android 通用剪贴板 JNI、多指手势、SAF/文件权限、前后台生命周期通知、原生 WebView、Android https OpenSSL 打包。以上为已知缺口，已在平台矩阵与用户文档中列明，不是平台空实现。
- 文档：修复 `README.md` / `Website/src/content/cargo.md` 的过时包名与已删除脚本，`AGENTS.md` 残缺 skill 引用，`Docs/architecture.md` 三处悬空链接/脚本，`crates/hosts/src/PLATFORM_COVERAGE.md` 的 DevTools 描述，`crates/system/README.md` 的 Swift 风格 API 名，`editor_interaction_tests.rs` 与代码矛盾的迁移期注释，以及三个 `Platforms/*/README.md` 的过时构建说明。

## 2026-11-08（二）：Media Player 改为画面内控制、弹幕、全屏/画中画与本地/网络来源

- 页面：去掉随仓库样片与合成源，改为“Open file…”（原生面板）、拖放到播放器/音频卡片、URL 输入三种来源；视频同名 `.xml` 弹幕自动加载，弹幕输入框可选 6 色发送，也可导入 XML；音频在后台线程解码波形与标签。需 `--features media-ffmpeg`。
- 结构验证：compose `media_player_tests` 13 项全部通过（新增：画面内控制栏位置与 16:9、播放按钮与快捷键 →/M/D、全屏与画中画在无宿主时的窗口内回退与 Return、到期弹幕渲染在画面内且可关闭、进度条拖动中为预览、松手提交）；`cove-media` 集成 11 项（新增 HTTP 206 探测与播放到结束、700 帧超队列读前无丢帧）。
- 原生验收（DevTools，FFmpeg 构建，30 秒 1280×720 H.264/AAC 本地文件）：本地路径打开后自动播放并加载 160 条 XML 弹幕，滚动/顶部/底部弹幕带阴影渲染、随媒体时间移动；悬停显示底部渐变控制栏、中央播放按钮与时间；控制栏图标（播放、音量、时间、弹幕、设置、画中画、全屏）就位。
- 用户反馈（拍照）：二级菜单上下闪跃，两个位置相差一行。根因是播放中每次位置上报都重建播放器整层控件，已打开的子菜单被反复重新呈现、在原位与贴边夹紧两处交替。播放器主体改为只订阅稳定快照，位置相关部分拆为独立子视图；`position_reports_do_not_re_present_an_open_menu` 覆盖并变异验证。修复后的观感需前台窗口复验。
- 用户反馈：无视频时设置菜单闪烁并最终报 `RHI: layout mismatch` 退出。根因在渲染层：绑定集 LRU 缓存淘汰了 UI 纹理槽/图集/背景模糊通道长期持有的句柄（玻璃菜单每帧新建绑定集，迅速填满 1024 容量）。改为每次向设备取用并移除句柄缓存，`binding_set_eviction_tests` 覆盖且变异验证过；graphics 全量通过。修复后的长时间打开菜单尚未在前台窗口复验。
- 用户反馈：播放中移向设置菜单时菜单消失。空闲计时在指针进入菜单后把控制栏隐藏。`MenuButton::on_open_change` 通知播放器，菜单打开期间不隐藏；`an_open_settings_menu_keeps_the_controls_up` 覆盖。
- 用户反馈：播放中控制栏不停闪烁。根因是自动隐藏会卸载控制栏，命中目标变化被当作指针移动而重新显示，形成显示/隐藏循环；改为常驻挂载、只切换不透明度与命中，并仅以真实位移计为活动，`hidden_controls_stay_mounted_and_ignore_a_resting_pointer` 覆盖。
- 验收中发现并修复：解复用路由写反导致约 17.5 秒处画面冻结（引擎层复现并修复，有回归测试）；暂停时到期的弹幕停在屏外（创建时即按估算宽度定位）；音量滑块拇指压住时间标签（改为外层留白）。
- 框架层：弹出层定位改为在布局回写中先于内容确定（`Node::set_frame_resolver`），菜单行锚定下一级子菜单时读到的是父面板翻转/夹紧后的位置，消除嵌套子菜单在两个锚点间跳位与重新弹出首帧未夹紧。三级菜单贴边、播放器设置子菜单逐帧重建两项场景覆盖，前者修复前失败。前台窗口复验待做。
- 未完成：设置菜单、真实系统全屏与画中画窗口、HTTPS 远程 URL、音频卡片拖放尚未在原生窗口逐项目检——验收后半段 gallery 窗口不再输出帧（DevTools 截图与输入超时，进程空闲等待显示刷新，判断为窗口不可见），需窗口在前台时补测。iOS/Android/Windows/Linux 未做设备验收；移动端画中画为窗口内浮动回退，非系统画中画。

## 2026-11-08：音视频组件（Media Player）

- 新页：Media 分类下的 Media Player，覆盖 `VideoPlayer`、`AudioPlayer`、`MediaTransport`、`VideoView`、`AudioLevelMeter`、`Waveform`。默认构建由基于公开底层 API 的合成源驱动（`VideoOutput` 发布 `SharedPlanes` 帧、`AudioMeter::record` 计量）；`--features media-ffmpeg` 播放 `examples/gallery/resources/media/sample.mp4`（384×216 H.264 + AAC 调幅正弦，8s），波形由 `MediaDecoder` 解出。
- 结构验证：compose `media_player_tests` 6 项（同一纹理 id 提交最新帧且背景铺满、跨线程变更经 UI 收件箱刷新、播放/暂停/结束后从头播放/静音、拖动进度条松手为精确 seek、无时长时进度条禁用、波形按指针比例 seek、视图卸载即注销监听）；`cove-media` `decoder` 9 项（内存 WAV/Y4M 与真实 H.264/AAC：探测、重采样、缩放、seek、取消、分离请求、播放到结束、结束后 seek）。compose 全量 1710 项通过。
- 原生验收：DevTools 真机操作（1280×720，FFmpeg 与合成两种构建）。H.264 首帧即显示，播放时画面与时间码推进、暂停图标与时间跟随快照，播放到 0:08 回到 Play；暂停时拖动进度条到 0:05 画面同步更新；音频走 CoreAudio，电平条实时跳动、停止后归零；波形按播放进度着 accent 色，结束后点击 25% 回到 0:02 并暂停；浅/深主题目检通过。
- 验收中发现并修复：视频帧首帧落入图集后同 id 的后续帧不更新（画面停在第一帧）→ 纹理存储把同 id 新所有者作为流式纹理移出图集、原地更新专用槽；设备音频时钟在 flush 后不复位，结束后 seek 显示仍停在末尾；文件末尾未清空包队列就排空解码器，短片段停在 Playing；`from_micros` 放大 1000 倍；未指定声道布局的 WAV 第二帧起重采样失败；控制条时间标签被滑块拇指遮挡、行未拉伸、音频卡片缺内边距；x86-64 内联汇编 `1:` 标签在 Intel 语法下不合法（Linux/Windows 编译会失败）。
- 未验证 / 既有问题：x86-64 SSE 内核只做了 Linux/Windows 目标的编译，未在 x86 机器上运行；iOS/Android/Windows/Linux 未做设备验收。主题切换的圆形揭示首帧在 DevTools 截图中为整屏品红，不含媒体的页面同样出现，属既有问题，与本轮无关。
- 文档：新增 `Website/src/content/components/media-player.md` 并登记导航；更新 `Docs/media-ffmpeg.md`、`crates/media/README.md`。

## 2026-11-07：Stepper 外观重做与堆叠分支塌陷

- 外观：标记改为按状态的实心圆盘（完成 success + 勾、失败 error + 叉、进行中 accent + 指示器、等待描边空心 + 序号），选中不再画整行底色，只强调标记与标题；连接线搬进该步自己的单元格，横向跨到下一个标记的真实间距（1pt 发丝线，`min_width` 24 兜底），纵向竖轨从标记底边连到下一个标记顶边，不再留断口；连接线颜色取它离开的那一步的结果（success/error/中性 border）。状态与交互切换走主题 fast 动效。
- 根因修复：自适应堆叠分支把三步压成同一行。`FlexModifier::apply` 只在 `basis` 为 `Some` 时写回 flex basis，`None` 保留节点上一次的样式；宽屏分支给每步 `basis 0`，切到堆叠分支后这个 0 留在同一批复用节点上，而容器主轴已换成高度，于是每个单元格高度为 0。`crates/compose/src/modifiers/layout/mod.rs` 改为完整表达作者意图（`None` 显式写回 auto）。该缺陷属于共享修饰符，任何按分支切换主轴的自适应组件都可能命中。
- 结构验证：`stepper_appearance_tests` 6 项（横向连接线跨距、纵向竖轨连续、连接线随结果着色、标记填充、选中不改底色、堆叠分支不塌陷）+ 原有 `product_interaction_regression_tests::stepper_owns_one_tab_stop_and_preserves_outcomes`、`narrow_reflow_tests::stepper_stacks_on_a_narrow_container_and_keeps_the_axis_aware_keyboard_path`，共 8 项通过；移除 basis 修复后堆叠用例失败，逐项变异验证过。compose 全量 1702 项中 1697 通过，5 项失败（`control_motion_tests::switch_value_changes_immediately_while_the_thumb_travels`、`list_tree_tests::list_selection`、`list_tree_tests::selected_row_fits_its_slot`、`sectioned_list_tests::sticky_headers`、`mobile_presentation_tests::a_tap_opens_a_touch_section_row_...`）在 HEAD 干净工作区同样失败，属本轮之前的既有失败，与 Stepper 无关。
- 原生验收：DevTools 真机操作，600×820 与 1180×820 两种窗口。窄容器（内容 476pt < 508pt 预算）堆叠分支目检三步依次落位、竖轨连续、下方 Start/Fail/Complete 与结果条不被压叠；Fail 后该步为 error 实心叉、其下方连接线转红；宽容器恢复横向行、连接线落在标记中线；Tab 停靠点内 ArrowDown 移动焦点并出现焦点环；深色主题下等待步描边、中性连接线与 muted 文字均可读。
- 文档：新增 `Website/src/content/components/stepper.md` 并登记到文档导航。

## 2026-11-06：Tree 参考线落位、投影列索引与悬停底带

- 根因修复：`TreeGuideOverlay` 的布局表面没有子节点、高度为 0，被 `ZStack(alignment: .leading)` 按垂直居中摆到行的中线上，而绘制钩子把 `origin.y` 当行顶用——`Straight` 每行只画下半段、行间接缝断开，末子的收束段掉到行下方悬空。表面改为与行等高后竖线接成连续一条，`Elbow` 的横向连接与收束点回到行中心。
- 拖拽投影：`reparent::guide_flags` 原先把深度 0 祖先的兄弟状态也写进列表（长度 `depth`），展平投影与绘制循环却按 `level` 索引、约定根层不占列（长度 `depth - 1`），投影生效时最外列读到不存在的根层、父级真正的续接位被丢弃。改为按祖先深度写入 `column = ancestor_depth - 1` 并跳过根层。
- 悬停：行节点此前用 `radius.sm` 铺满整行（含缩进与展开控件），自定义 `TreeRowStyle` 或 `TreeSlots::trailing` 场景下样式又用 `radius.md` 在内容区铺第二层，内容区比留白区深一档、出现接缝，高亮左边界也与 accent 选中带不一致。新增 `TreeRowStateLayer` 占据行内容插槽、在绘制阶段读取悬停标记，行节点自身保持无底色；样式自行接管时该层让位。悬停与键盘光标仍不排入 Tree 重算。
- 小项：拖动中的源行连同其参考线一起降低不透明度（此前只有行变淡）；`Dashed` / `Dotted` 的分段相位改为按绝对 y 排布，跨行不再每行重新起相。
- 验证：`list_tree_tests` 新增 `tree_guide_columns_are_drawn_from_the_row_top`（参考线节点与行同 y、同高）与 `tree_row_hover_paints_one_band_in_the_content_slot`（只有一层底带、比行窄、与行等高、离开后消失），`default_tree_row_hover_does_not_recompose_tree` 改为断言底带落在状态层上；`tree_reparent_tests` 新增 `guide_flags_index_columns_by_ancestor_depth`，既有列索引断言同步收敛。compose 全量 1687 项通过；移除表面高度设置后落位用例以 `45.0 != 30.0` 失败。
- 原生验收：DevTools 真机指针与拖拽，Scene hierarchy 页展开全树。`Straight` 下最外列逐像素为 431→597 的连续线（旧实现是 120-149/180-209 这类下半段），末子收束正好停在行中心；`Elbow` 的横向连接与竖段落在行中心；`Dashed` 在同一列跨 6 行保持 6pt 周期、`Dotted` 保持 4pt 周期，行界处不重新起相；悬停行底带左边界随缩进与选中带同步（深度 1 为 x=365、深度 2 为 x=381，差值正好一个 16pt 缩进），无第二层底色；真机拖拽 `Lights` 到 `Objects` 内部，预览中祖先列按新层级落位（296 连续、312 止于 Lights 中心、328 覆盖 Key Light→Fill Light 中心），被拖动子树的参考线墨色与标签同步变淡（202 → 243），松手后回到满墨；浅色与深色主题均成立。
- 待办：无（该轮三项待办已在下面《2026-11-06：Tree 悬停主干按单层整组加深、命中区与悬停依赖收敛》一并处理）。

## 2026-11-06：Tree 悬停主干按单层整组加深、命中区与悬停依赖收敛

- 悬停加深范围：先按「整条上游路径逐层加深」实现，又在中间一轮改为「在祖先中线把高亮交给下一层」；两种写法都找不到对应先例，逐条查证后确认业界约定是：文件树/大纲树的缩进参考线本身是静态的（VS Code Explorer `tree.indentGuidesStroke`、IntelliJ *Show tree indent guides*、Ant Design Tree 的 hover 只改行底色），唯一会「跟随光标」的是编辑器的 active indent guide（Sublime `indent_guide_options: draw_active`、VS Code `editor.guides.highlightActiveIndentation`），其语义是**只高亮光标所在那一层、并覆盖该缩进块的整段范围**。最终按此实现：悬停行只加深自己悬挂的那一列（层 = 深度 − 1），该列经过的**整组兄弟**一起变深到 0.62，祖先各列、其他分组的列、更深层的列保持 0.36；`Elbow` 的弯头属于每行自己的标签，只有指针所在那一行的弯头加深。根层行不悬挂任何主干，悬停不加深参考线。判定改为 `TreeGuideHover` 记录 `(行号, 祖先路径, 层)`，绘制阶段用 `deepens(path, level)` 比较前 `level + 1` 段索引（`TreeNodeKey.path` 含根索引，故层 `L` 的分组标识是 `path[..L+1]`，写成 `path[..L]` 会让旁支的列误跟着加深）；跨行失效仍由参考线节点自身承担，不排入 Tree 重算。悬停行被折叠或过滤移出树时，`retain_active` 在展平阶段核对记录，主干立刻回到默认墨色，不留残影。
- 悬停依赖收敛：`TreeRow::body` 此前无条件读 `hovered.get()`，默认样式下每次指针移动都会把整棵树排入重算——之所以没暴露，是因为 `hovered` State 从未出现在 `Tree::dynamic_properties()` 里，写进去的人其实谁也没通知（顺带的真实缺陷：自定义 `TreeRowStyle` 因此从来没收到过 `is_hovered`）。现补上 `DynamicField::state("hovered", &self.hovered)`，`body` 不再读取该状态，改为把判定封成 `hover_snapshot` 闭包传入行；`children()` 只在「有 `TreeSlots::trailing`」或「样式 `requires_hover_recompose()`」时调用它，默认样式改读行节点上的悬停标记附件。`TreeRow::update_node` 的悬停回调同样只在这两种情况下调用宿主 `hover_action`。默认样式回归为纯绘制阶段响应，自定义路径才订阅。
- 拖拽命中区：`TreeDragRegistry::hit` 原先在行的绝对 frame 之外再向左扩 `ROW_GUTTER`，而行节点本身已经铺满整行插槽（缩进留白与展开控件都在 frame 内），等于把同一份留白算了两遍，树左侧的空白区也会被误判成命中。改为只用原始 frame 判定。
- 验证：`list_tree_tests` 的 `tree_guide_hover_deepens_the_path_to_the_hovered_row` 断言悬停叶子行时全树只有一列变深（`(leaf_y, 23.0, 0, 中线, deep)`，祖先列与父级续接全部保持默认），悬停父级行时该列在 Group / Leaf / Sibling 三段一起变深，Leave 回到基准，折叠悬停行后残留清零；新增 `tree_guide_hover_deepens_only_the_trunk_a_row_hangs_from`（`Elbow`，`Sibling` 下两个子行），断言共用主干跨兄弟带加深、指针上方兄弟的弯头仍是默认墨色、只有指针行的弯头加深。三个变异各自失败：去掉 `deepens` 的层级判定（祖先层跟着变深）、把主干收窄到悬停行自身路径（兄弟带不变深）、让弯头不看指针行（兄弟弯头跟着变深）。`tree_drag_resolves_the_indent_gutter_inside_the_row_frame`、`a_custom_tree_row_style_receives_the_hovered_row` 同轮保留。compose 全量 1690 项通过；doctest 由 1 passed / 1 ignored 变为 2 passed / 0 ignored（`ToggleButtonStyle` 的示例从 ` ```ignore ` 改成可编译 doctest，同时把 Swift 风格的 `Button(isSelected:action:)` 写法换成真实 Rust API）。
- 原生验收：DevTools 真机指针，Scene hierarchy 页（Scene › Lights › Key Light / Fill Light，Objects，Camera），基线指针停在 900,150 的树外，逐像素取参考线墨色。`Straight` 悬停 Fill Light：二级列 x312 在 460..507（Key Light 整带 + Fill Light 到中线收束）202 → 164，祖先主干列 x296 全程 202 → 202 不变。悬停 Lights：x296 在 430..567 连续一段 202 → 164，覆盖 Lights、Key Light、Fill Light、Objects 直到 Camera 的中线，x312 不变。`Elbow` 悬停 Fill Light：x312 主干 202 → 164 跨两行，本行弯头 x314-318 208/201/202 → 177/159/169，Key Light 的弯头 206/204/203 → 203/200/198（仅 JPEG 噪声级）；悬停 Key Light 时加深的是它自己的弯头 206 → 159，Fill Light 的弯头回到 208 → 214 噪声级，主干仍整组 202 → 164。深色主题同页复核：主干 x312 84 → 112、祖先列 x296 84 → 84、指针行弯头 80/88/87 → 109/113/117、兄弟弯头 83/82/86 → 79/81/85。

## 2026-11-05：Tree 参考线加粗与悬停加深

- 外观：`guide_width` 默认 1.0 → 1.5pt，参考线基础墨色由 `on_surface_muted` 的 0.24 提到 0.36（1pt 细线在高分屏上几乎不可读）。
- 悬停：鼠标停在某一行时只加深该行自己绘制的参考线（含 `Elbow` 横向连接段）到 0.62，其余行保持默认。参考线在绘制阶段读取行的 `__tree_row_hovered` 标记，行把参考线节点登记在 `__tree_row_guides` 上、由悬停回调逐个 `mark_render_dirty`，因此指针移动不排入 Tree 重算（`default_tree_row_hover_does_not_recompose_tree` 的约束仍然成立）。
- 验证：`list_tree_tests` 新增 `tree_guide_hover_deepens_only_the_hovered_row`，断言墨色只变深一行、失效标记恰好落在该行的参考线节点上、且不排入重算；移除墨色分支或移除 `mark_render_dirty` 后该用例分别失败。原 `tree_guide_lines_are_pixel_aligned`、`tree_guide_lines_include_ancestor_continuations`、`tree_guide_style_straight_draws_unbroken_columns` 的 1pt/0.25 断言同步到 1.5pt/0.4。compose 全量 1684 项通过。原生验收：DevTools 真机指针悬停 `Key Light` 行，`Straight` 与 `Elbow` 两种样式下该行两列明显加深、相邻行墨色不变，深色主题同样成立。
- 待办：悬停加深只作用于参考线与行底色，未提供开关；减弱动效下无过渡动画（当前为即时切换）。

## 2026-11-05：Tree 拖拽实时重排与参考线样式

- 能力：Tree 行拖拽改为实时重排，删除插入线。投影只改位移不改数据，展平顺序、布局与命中仍由宿主模型决定，每行按 `(投影槽位 - 自身索引) * 行距` 位移；源行 0.38 不透明度，其余行弹簧跟随，松手后模型落位与拖拽中绘制的位置逐行一致。`Before` / `Inside` / `After` 由行高上下 28% 判定。
- 根因修复：`FixedVirtualStack` 给每个 `RowSlot` 加了裁剪，行的 `frame` 是父级相对坐标，位移作用在行上会把行推出唯一可见区域，拖动中整行消失（`frame: null`、`inputEnabled: false`）。新增 `ROW_SHIFT_ANCHOR` 标记与 `ReorderRowShift::anchor`，位移改作用在裁剪槽上；ReorderableList 使用自身绝对定位槽，不受影响。
- 键盘：`Alt+↑`/`Alt+↓` 兄弟移动、`Alt+Shift+←`/`Alt+Shift+→` 出/入层级，与指针共用 `can_drag`/`can_drop`/`on_drop` 契约；被拒绝的组合退回普通方向键导航。`TreeRow::on_key_command` 此前无条件吞掉四个方向键（不检查 `event.modifiers`），Alt 移动因此只在直接调用 `handle_key` 的单测里成立；现先交给会话的 Alt 感知路径，`HOME`/`END` 等回落同一入口。
- 外观：参考线拆成 `TreeGuideStyle::{None, Straight, Elbow, Dashed, Dotted}` 加 `guide_width`（pt，不随字号缩放）与 `guide_position`（0.0 到 1.0）；新增 `leading_padding`/`trailing_padding` 链式方法并补齐缺失的 `indentation`。`Inside` 放置自动展开目标分支，避免移入折叠节点的行被隐藏。
- 焦点：Tree 行改为可聚焦，点击行即可用键盘；`SelectionKeyHost` 记录子树是否持有焦点，焦点行随收起/过滤离开输入图后把焦点收回容器，`Alt+←` 收起全树后的快捷键不再静默失效。
- Gallery：Scene hierarchy 页新增 `None`/`Straight`/`Elbow`/`Dashed`/`Dotted` 样式条与 `Indent`/`Lead` 两个滑杆，状态行同时报告拖放结果（`Moved X into Y` 等，自动展开不再覆盖该消息）；目录副标题同步。
- 验证：`tree_reparent_tests` 10 项、`list_tree_tests` 新增 14 项通过，其中 `tree_drag_paints_projected_rows_inside_their_slots` 断言每行仍在其最近 `clips_to_bounds()` 祖先内（早期版本只断言宽高，漏掉了本缺陷），`tree_drag_reveals_the_branch_an_inside_drop_creates`、`alt_arrow_on_a_focused_row_moves_the_branch`、`collapse_all_keeps_the_tree_on_the_keyboard` 三项均验证过移除对应修复后失败。compose 全量 1683 项通过。原生视觉与交互验收已完成：DevTools 真机指针拖拽验证三种放置位置（`Moved Sphere after Fill Light`、`Moved Camera before Key Light`、`Moved Camera into Key Light`）、四种键盘移动、展开/收起全部与焦点回收；浅色与深色主题下目检五种参考线、两个滑杆与拖动中的投影，行槽连续无空隙无重叠。
- 待办：拖到视口边缘不自动滚动；键盘重排立即改结构，不走弹簧动画；减弱动效只有代码路径验证；Gallery 的 Reset example 不会恢复 Tree 模型（列表页同样如此，属既有行为）。

## 2026-11-05：ReorderableList 拖拽实时重排

- 能力：删除 2pt 插入线（`visible_boundary`/`drop_indicator_y`/`reorder-drop-indicator`），拖拽过程中直接按落点重排——被拖动行的槽位跟随落点，越过的行反向让出一个步长，画面等于松手后的顺序；`ReorderRowShift` 改为行槽上的弹簧位移资源，新增 `.axis(ScrollAxis::Horizontal)` 让滚动、拖拽、重排与落位整体转横轴。
- 外观：落位不再二次滑动，释放只清除投影并回调 `on_move`；浮起副本弹簧落位、占位行透明度回到满值；Escape 取消时投影弹回、副本飞回原槽位。通用拖放预览补弹簧跟随与淡入淡出；各组件阈值统一为 6pt（删除 SwipeList 的 4pt 死区）。
- Gallery：Reordering 页新增第二条横轴卡片条（6 张卡片、112×8 尺寸、96pt 视口）与顺序读数，纵向条保留。
- 验证：`reorder_model` + `reorderable_list_tests` 共 34 项通过，新增 `drag_reflows_the_rows_live`、`horizontal_axis_reorder`、`horizontal_keyboard_reorder`；原插入线断言改为位移与绘制顺序断言。compose 全量 1654 项通过。原生视觉验收已完成：DevTools 真机指针拖拽在浅/深两套主题下分别目检纵轴与横轴——拖动中各行槽位连续无空隙无重叠、浮起副本跟随指针、松手后停在投影位置不再二次滑动、顺序读数与画面一致（纵轴 `Portrait · Macro · Landscape · Night · Panorama`，横轴 `One · Two · Three · Four · Five · Six`）。减弱动效（reduce motion）下的表现仍只有代码路径验证。

## 2026-11-04：Drag & Drop 会话与指针修复

- 根因：Gallery 每次 body 重建 `DragScope`，旧实现同时新建 `DragSession`；拖拽中的一次重组把 pointer-up 交给空会话，payload 丢失。会话改为按视图身份 `Scope::remember`，并新增 `DragScope::with_session` 供共享。
- 指针：按下先可转移捕获，首次快速移动离开源节点仍能越过阈值；越过 6pt 后升级为独占捕获，滚动容器不能再抢走。释放先结束会话再释放捕获，避免 lost-capture 回调把 payload 取消。
- 类型：`Draggable<T>` / `DropTarget<T>` 改为真正泛型 payload（`dyn Any` 会话存储），不再只支持 `u64` token；不同类型目标可在同一 scope 共存。
- Tree：行拖拽同样改为按下可转移、阈值后独占；新增外层 ScrollView 下的行拖拽回归。
- Gallery：Drag & Drop 页的标签改为有底/圆角的 chip，预览同步；目标 lane 在悬停时用 `state_layer_selected` 高亮，释放后清除。
- 验证：`drag_and_drop_tests` 6 项（含快速单次移动、ScrollView、异类型 payload）、`list_tree_tests` 16 项、Gallery Drag & Drop 场景测试 1 项通过。

## 2026-11-04：Tree 能力扩展与外观收敛

- 能力：新增父子级联勾选（含半选）、`checkStrictly`、默认/受控勾选、`defaultExpandAll`、`autoExpandParent`、手风琴展开、自定义过滤谓词与 `filter()`、懒加载 `load_data`/`loadedKeys`/加载指示、拖拽生命周期、非虚拟滚动开关，以及 `getPath`/`scrollTo`/`setCurrentKey`/`setCheckedKeys` 等命令式方法。
- 外观：默认行样式统一为 accent 选中带、`state_layer_hover` 悬停、`state_layer_selected` 搜索命中；键盘光标使用更弱的选中底色。复选框、前导图标、自定义展开控件、加载进度环与缩进参考线共用同一行槽，叶子行保留缩进与展开栏。
- Gallery：Scene hierarchy 页接入受控勾选、严格勾选、手风琴、懒加载 `Objects` 分支、Check all / Clear checks、前导文件夹/文件图标与事件状态行；保留搜索、多选、展开/折叠工具栏。
- 验证：`tree_behavior` 11 项、`tree_features_tests` 15 项、`list_tree_tests` 15 项、`row_density_tests` 4 项通过；Gallery `tree` 5 项场景测试用真实指针事件验证复选框级联、严格勾选、手风琴与 `Objects` 分支首次展开加载。`Tree` 页在 DevTools 下的浅/深主题目检仍待执行。

## 2026-11-04：List / Tree / SwipeList 排版与示例卡片

- 通用修复：`ListRow` 补 8pt 槽间距、选中行墨色继承、行槽裁剪；`Tree` 行内容撑满行槽、叶子行保留缩进/disclosure 栏、选中带满宽；`SwipeList` 内容满宽且动作条只在打开时挂载。
- Gallery 卡片：`StorySection` 由背景 + 1px 线框改为 `surface_raised` + `radius.xl` + 低 elevation 阴影 + 20pt 内边距；删除重复且未使用的 `GalleryStorySection`。
- 示例对齐：List 52pt 两行行高 + 图标 + 默认选中；SwipeList 52pt 两行内容 + 圆形标记 + 2pt 行距；RefreshableList 264pt 视口完整显示 6 行；Tree 简化行内容并补回搜索/展开/折叠工具栏。
- 结构验证：`list_row_tests` 6 项、`list_tree_tests` 15 项、`swipe_list_tests` 7 项通过；Gallery 全量测试通过。DevTools 在 1400×1000 下逐页截图目检 List、SwipeList、RefreshableList、Tree；共享卡片在浅色与深色主题下均已目检。

## 2026-11-04：Gallery 侧栏搜索清除按钮裁切

- 输入 `hh` 后清除图标贴右边框并被裁切，根因是通用 `TextField` 的绘制把绝对原点叠加了两次，不是 Gallery 用法错误。修复后图标与命中区使用同一个绝对起点；新增偏移容器回归测试。
- DevTools 在 Gallery 侧栏实际输入 `hh` 后截图目检：清除图标完整显示，右侧内边距正常。`text_field_tests` 51 项通过。

## 2026-11-04：DragUpload 默认外观

- `FileDropZone` 补上安静状态的沉降表面、圆角与虚线框；悬停/接受/拒绝/禁用各自有独立描边与填充。虚线由圆角轮廓连续分段绘制。
- `FileDropPrompt` 改为图标芯片 + headline 标题 + caption 过滤说明 + 次级 “Browse files” 按钮；移除重复的 “or choose files” 与正文样式的过滤行。
- `UploadProgress` 只在有任务时出现：进行中为进度条 + 百分比 + 取消；结束为成功/失败色调状态条。Gallery 示例把诊断行收为 caption。
- 验证：`file_drop_tests` 22 项通过（新增空闲无进度、进行中显示进度/取消、结束显示状态条三项）。Gallery Drag Upload 页在 1400×1000 浅色与深色主题下由 DevTools 截图目检，圆角虚线框、居中图标芯片、按钮层级与 caption 行均按预期呈现；进行中/成功/失败状态的视觉效果尚未实机截图验收。

## 2026-11-04：拖入批次与类型语义修复

- **修复**：`DropPhase::File` 曾被直接映射成一次投递，而 SDL 每个文件发一条 `DROP_FILE`，导致 `maximum_files` 在真实拖入里只数到 1。改为 `Begin` 开批次、`File` 累积、`Complete` 一次性投递；`cancel` 丢弃未投递批次。宿主改用 `DroppedFile::inspected`，目录与大小来自 `fs::metadata`。
- 目录语义：`DroppedFile::is_directory` 与 `FileDropZone::allows_directories`（默认拒绝）。拒绝是显式结果，不是读取失败。
- 扩展名语义：新增 `DroppedFile::extension()`；无扩展名与隐藏文件（`.gitignore`）在非空白名单下明确拒绝，在空清单下通过。`file_kind` 不再把前导点当扩展名。
- Content-Type：新增 `upload/content_type.rs` 查表（图像/文档/音视频/字体/压缩包），未知回落 `application/octet-stream`；`FileDropPrompt` 对空过滤显示 “Any file” 而非省略该行。
- 覆盖：`file_drop_tests` 18 项（批次三文件单次投递、超限整批拒绝、批次清除、取消、目录拒绝与放行），`content_type` 6 项单元测试。
- **原生验收仍未完成**：以上为结构测试与 DevTools 注入验证，真实 OS 拖入（含多选、文件夹）与原生打开面板交互尚未实机验收。

## 2026-11-04：系统文件拖入与上传

- 平台层：`cove-platform` 新增 `ExternalDropEvent` 与 `DropPhase::{Begin, File, Text, Position, Complete}`，挂为 `InputEvent::ExternalDrop`；`native.c` 的 `cove_shell_poll` 接入 `SDL_EVENT_DROP_*`，drop 坐标按 `coordinate_scale` 单独换算，`CoveShellEvent` 新增 `aux` 承载拖拽来源。`sdl3_shell` 的 `decode_event` 解码 25…29，并把无窗口归属的 drop 路由到主窗口（iOS 以 null window 交付文档）。
- 组件层：新增 `FileDropZone` / `FileDropRegistry`（节点命中、顶层优先、类型与数量过滤、进入/离开回调）与 `DroppedFile`；新增 `DragUpload`，组合拖放区、上传状态机与进度显示。`FilePicker` 作为 core ambient 提供浏览兜底，触屏与键盘用户走同一回调。
- 上传：新增 `primitives/upload`，含 `multipart` 编码（流式拼接、文件名转义、编码前超限拒绝）、`UploadTransport` 接缝与 `UploadState` 五态。上传在 worker 线程执行，经 `cove_core::work::spawn_then` 回 UI 线程写 `State`。
- 宿主：`crates/hosts/src/application_services/upload.rs` 提供 `HttpUploadTransport` 并在 `install_platform_services()` 安装；桌面运行时把 `InputEvent::ExternalDrop` 路由进 per-window 注册表，并安装 `FilePicker`。删除 `composition.rs` 中无调用者的旧 `AssetDropRegistry` 死代码，改为复用 `FileDropRegistry`。
- 结构验证：`file_drop_tests` 13 项、`external_drop_tests` 6 项、`upload` 单元测试与 hosts transport 测试通过；Gallery 全量 104 项通过。**原生验收未完成**：从其他应用真实拖入文件、原生打开面板交互均未在本机验收，结构测试不代表原生拖放已验收。

## 2026-11-04：屏幕取色后端

- ColorField 吸管链路：`cove-platform` 只保留 `ScreenColor` / `ScreenColorSample` / `ScreenColorCompletion` / `ScreenColorBackend` 契约；取色实现下沉到 `cove-shell/src/color_sampler/`，按窗口系统分 macOS（`NSColorSampler` / `objc2-app-kit`）、Windows（Win32 屏幕 DC）、Linux（X11 `XGetImage` 截屏采样 / XDG `PickColor` portal）、iOS/Android/Wasm（`Unsupported`）后端，全部异步回调且不阻塞平台线程。宿主按后端可用性安装 ambient，不支持时吸管隐藏。
- 结构验证：`cove-platform` 契约、`cove-shell` 12 项后端测试（异步队列/唤醒/去重/取消、Win32 `COLORREF`、X11 掩码像素、D-Bus 编解码与假总线完整 `PickColor` 流程）、`cove-ui-compose` 吸管 fake sampler 场景，以及 `cargo check --workspace --all-targets` 全绿。Windows/Linux 后端源码在本机用带桩契约的临时 harness 完成类型检查。
- 原生验收：macOS `NSColorSampler`、Windows `GetPixel` 轮询、Linux X11/portal 拾色的实机视觉与交互尚未验收；结构测试与假总线不等于原生拾色、原生键盘或 IME 激活已验收。

## 2026-11-04：字段 API 收敛

- 输入框外壳统一：`InputChromeState` 是唯一的交互快照，`TextField`、数值前缀、`Select`、`TimeInput` 与 JSON 编辑器共用 `InputAppearance` 的填充、边框与圆角推导，删除各自手写的背景/边框。
- 新增独立 `Field` 外壳（与 `Form` 同一套标签/帮助/错误/可访问性布局），Gallery 各 field 页由临时的 `field_row` 改用 `Field`。
- 数值配置收敛为 `NumberFieldOptions`；`Vec3Field` 组合三个 `NumberField` 并共用该配置，不再复刻其 builder。
- 目录由 106 页收敛为 102 页：`Numeric` 合并 `NumberField`+`Vec3Field`，`Text Fields` 合并 `TextField`+`SearchField`+`TextArea`+`JsonField`；对应旧页的 `GalleryPage` 变体、catalog 条目、图标与 dispatch 一并删除。
- ColorField 增加色轮（`ColorWheel`），提供环样式（色相环 + 饱和度/明度方块）与盘样式（角度=色相、半径=饱和度），浮层默认展示环样式，`shows_wheel(false)` 可关闭；色轮属于取色器，不作为 Slider 类型。修复拖动坐标混用（局部 vs 窗口）导致的映射错误，新增浮层内拖动回归测试。浮层新增透明度棋盘、预设色板（`shows_swatches`/`swatches`）、可编辑 RGBA 数值、HEX/HSL 文本切换、最近颜色与吸管钩子（`on_sample`，平台取色由宿主实现）。`cove_core` 新增逐顶点着色原语 `add_disk_shaded` / `add_shaded_annulus`。Gallery 展示环/盘两种样式，模型、指针交互与着色几何各有单元/场景测试。
- 结构验证：`catalog_audit` 102 页 / 204 个宽屏窄屏场景通过。页级视觉与交互验收仍按原队列逐项进行。


## 2026-11-04：九页 Swift 对照

基准是本地 `swift/Examples/Sources/GalleryContent/Stories/Controls/ControlStories.swift`、`ToolbarStory.swift`、`TextEditorStory.swift`，以及 `swift/Compose/Sources/CoveUICompose/Primitives/Controls/CompletionPopover.swift`。全部实现使用共享 Rust，无新增 Swift。CompletionPopover 没有对应的 Swift Gallery story，按组件本身的十行窗口、详情、文档与编辑器导航语义接入示例。

| 页面 | 对齐的布局、状态与交互 | 结构/场景验证 | macOS 原生验收 | Rust 文档 |
| --- | --- | --- | --- | --- |
| SplitButton | Save / Save as / Export / Discard、200pt 菜单、结果状态、独立命中区域、单一 Tab 停靠点随左右键移动 | 主操作和菜单结果、Tab 停靠点与禁用回归 | 待检 | split-button.md |
| Toolbar | 自适应命令、播放/格式组、搜索、50…200% 缩放、Apply、纵向工具与八项纵向溢出 | Run、Zoom、Apply 结果；窄屏控件边界 | 待检 | toolbar.md |
| CompletionPopover | 真实 TextField 请求/接受、20 项候选的十行窗口、详情和文档 | Control-Space / Return 接受结果，宽窄布局 | 待检 | completion-popover.md |
| TextArea | Swift 三行样本、90…320pt 可拖动高度、520pt 最大宽度、字符计数 | 挂载真实编辑区与手柄，宽窄布局 | 待检 | text-area.md |
| Text Editor | 文档头、320pt 可调编辑区、行列状态、代码/正文、断点/折叠、书签/运行/警告标记、暂停行、诊断悬停、print/private 补全、20 万行导航 | 编辑/模式往返保留文档、完整几何矩阵、词法/折叠/诊断及压力数据模型 | 待检 | text-editor.md |
| NumberField | 3.14 / 0…10 / 0.25 步进、W 拖动/双击复位、混合/禁用、共享 1.25 的逗号/小数点输入 | 数值格式和键盘步进、宽窄布局 | 待检 | number-field.md |
| Vec3Field | (1,2,3)、0.1 步进、全宽/240pt 混合 X/Z、整体复制/粘贴/复位、宽排窄叠 | 每轴步长、默认平台异步剪贴板、前缀布局和编解码 | 待检 | vec3-field.md |
| ColorField | (88,126,231)、行内数值和 RGB 两个触发器、200×60 预览；当前主题、视口限宽、编辑草稿跨重组 | 色值编解码/浮层打开、宽窄布局 | 待检 | color-field.md |
| Select | fast / balanced / high / offline、默认 high、完整长标签与组合 emoji、360pt 最大宽度 | 菜单键盘/禁用项与组合框语义；宽窄布局 | 待检 | select.md |

`control_story_parity` 验证九页在 390 / 720 / 1280pt 下的真实控件边界；`text_editor_layout_tests` 另覆盖 1×/2× 字体与浅/深主题。场景事件注入仅验证 Rust 输入分发，不等于原生键盘或 IME 激活。各平台共享实现；其他平台原生视觉/输入尚未在本机验收。

## 本轮结论

API 收敛：删除 RichText 类型与导出，无兼容别名。普通文字与样式片段统一为 Text::new / Text::runs，TextRun / TextTraits；字体、颜色、对齐、行数、截断、正文读取共用同一 Text 配置与节点。Gallery 的 RichText 页改为 Text，SingleLineText 页标为 Text · Truncation；目录仍为 106 个能力页面。用户文档：`Website/src/content/components/text.md`。

2026-11-07 macOS 风格组件库：默认主题改为 macOS 窗口结构（白色内容区、`surface_variant` 侧栏与分组），`Button` 默认改为推按钮，输入/弹出按钮/搜索/复选/单选/开关/滑块/分段/Tab/列表/表格/日历/提示/进度/滚动条/对话框/通知统一外观；Sidebar、ButtonGroup、TabView 共用 `SlidingSelection` 弹簧滑动选中。修复 TabView 标题宽度塌为 0（原版即存在）、节点自身阴影被自身裁剪、列表分隔线残留。DevTools 截图目检（1280×720，浅色为主，深色抽查 Dashboard 与 Text Fields）：Dashboard、Checkbox、Toggle、Text Fields、TabView、ButtonGroup、Tree、DataTable、Calendar、Accordion、Toolbar、SplitButton、List、SectionedList、ReorderableList、Alert、Dialog。PropertyGrid 页内容不渲染的问题已于同日修复：先是挂载缺口，随后内容列在 `HorizontalScrollOrigin` 的 flex-start 测量探针内塌为固有宽度（约 60pt）；内容列改为 `Percent(100)` 宽度后填满滚动视口（扣除稳定滚动条槽），三种滚动轴模式与无作者宽度均有回归（`property_grid_sizing`），并经 DevTools 截图目检与折叠/填值交互后置条件验证。用户文档：`theme.md`、`motion.md`（新增）、`sidebar.md`、`slider.md`、`property-grid.md`（新增）。

2026-11-03 主题与选区追加：默认浅/深色改为中性桌面配色，圆角 0/4/6/8/12；正文/样式片段/输入框选区共用轻微圆角与两端短竖线，输入框移除重复透明度。Text 样式片段补齐共享选择注册、键盘扩展、独立选择与 display-only；跨链接拖选不激活链接。选择 scope 随挂载节点保留。触控长按已复用；圆形手柄与放大镜尚未接线，未宣称已完成。鼠标问题：用户确认浏览器也会闪回箭头，Codex 正常，暂不能归因 Cove；不添加推测性的原生光标覆盖。应用 I 形指针节点及焦点/拖选/重建路径另行回归，操作系统实际光标仍待系统恢复后原生复核。用户文档：`Website/src/content/components/theme.md` 与 `text-selection.md`。

| 项目 | 原因 / 实现状态 | 修复与展示 | 用户文档 |
| --- | --- | --- | --- |
| 搜索点击后不能输入，删除键仍有效 | TextField 未安装平台输入区域 resolver；SDL 文本输入和光标刷新未启动，DevTools 直接文本注入会绕过缺口 | 补齐焦点/布局提交时的 IME anchor、光标闪烁；侧栏使用 SearchField；搜索/清空/Escape 回归；用户已确认原生输入与光标正常 | Website/src/content/components/search-field.md |
| Slider hover 出现整条灰色背景 | 绘制滑块时把阴影写到了整个轨道节点上 | 阴影限定滑块；默认轮廓区分白色背景；补上大小、刻度、垂直与编辑状态示例 | Website/src/content/components/slider.md |
| Slider / RangeSlider 是否合并 | 一个绑定 f64，另一个绑定 RangeValue；双端点有独立焦点和最小间距语义 | 保留独立类型，共享样式；RangeSlider 补上滑块阴影、键盘焦点环、RTL 映射、端点可访问边界与点击焦点、步长/最小间距/禁用示例 | Website/src/content/components/range-slider.md |
| Rating 方块、重建后交互失效、密度示例无变化 | 绘制列表未保留 SVG 图片；重建使用了未挂载的新 session；示例回调写错状态且显示常量 | 保留资源与挂载 session，禁用/只读更新命中标志；独立示例值，恢复十颗星、清空和横向滚动 | Website/src/content/components/rating.md |
| 文本选择不一致 | 正文选择区域与控件手势边界需要分开；键盘扩展原先按 Unicode 标量而非字素移动 | 所有 106 页选择区域结构检查、15 种页壳的拖选复制检查；组合 emoji/重音字母按完整字素移动；输入框补上点击定位、拖选、双击词/三击全选、快捷键和及时重绘；导出 TextSelectionViewExt | Website/src/content/components/text-selection.md |
| 平台输入接口修改丢失或拆开 emoji | select / delete_surrounding 只写内部状态，UTF-16 按标量映射 | 选区对齐字素边界，删除/组合提交写回绑定并重绘；全选/复制/剪切与异步系统粘贴共享绑定 | Website/src/content/components/text-field.md |
| TextField 迁移未完成 | 撤销/重做、多行测量与滚轮、词/垂直导航、语法配色、前后缀与长度限制只有 Swift 实现；工作区因半成品改动无法编译 | 编辑事务历史（相邻输入合并、选中态随文本恢复、外部改写清空栈）接到菜单与快捷键；行/页/文档级光标移动与软换行行亲和；逐字素着色的语法颜色；前置/后置/前缀/后缀/字数计数绘制与命中区；输入、粘贴与 IME 提交遵守 `max_length`；平台删除与组合提交进入历史 | Website/src/content/components/text-field.md |
| 代码编辑器行号、横向滚动、覆盖层与取消 | 行号栏、长行横向揭示与横向滚轮、`__textfield_document_layout`、Escape 取消、hover 滚动条都只有 Swift 实现；更新阶段还不发布滚动界限 | 三数位行号栏（可配前景与背景色）参与测量与命中；绘制阶段按光标揭示双向滚动并发布滚动界限与文档布局；横向滚轮与纵向一致按 30pt 步进；溢出字段独占滚轮（含边界），聚焦时设置 `PreferFocused`；hover 时绘制滚动条覆盖层；Escape 调用 `on_cancel` 并把焦点交还最近的焦点祖先；Control-Home/End 到文档首尾；单行字段 Shift-Return 插入换行 | Website/src/content/components/text-field.md |
| TextFieldStyle 焦点/编辑状态与字素计数 | 样式响应式状态链未接完；rope、光标映射和部分调用方仍计标量 | 样式处理焦点、hover、IME 组合/提交/取消并保留输入节点；禁用复位；rope 与光标/密码/正文选择/snippet/补全统一字素；CRLF 与编辑接缝正确合并；Gallery 新增样式示例与实时字素计数 | Website/src/content/components/text-field-style.md、text-buffer.md |

## 已确认、仍待逐项修复的缺失

- TextFieldStyle 焦点/编辑状态接入、TextBuffer 字素计数本轮已补齐。当前测试还验证了 IME 区域重发布、组合预览缓存和真实文本环境下的双击选词，旧缺失说明已清理。代码编辑器 F1 悬停请求、诊断及补全的完整能力仍属于后续页级核对范围；不据此声称整个编辑器或全部 106 页已完成验收。
- iOS 原生组合预览事件的宿主路径仍需单独核对：本地 SDL UIKit 未发现 `SDL_SendEditingText` 发送路径。本轮共享 Rust 的样式/字素处理已实现，不能据此声称 iOS 原生 IME 预览已设备验收。
- 本轮将迁移断言改为验证 Swift 预期行为。结构与场景测试不替代原生视觉、键盘及 IME 验收。

## 验收范围

- 使用 Git `1d633f78^` 的 Swift Gallery / RatingStory / RatingSample 对照本轮已处理的示例能力。
- 原生 macOS Gallery：用户确认搜索点击后可输入、删除，光标恢复闪烁；DevTools 镜像确认 Rating 图形、半星与十颗星恢复，Slider hover 灰底消失，RangeSlider 拖动端点更新数值。
- 最新运行窗口中连续填入 Slider / Rating / RangeSlider 并清空查询，绑定与导航后置条件通过；Rating 指针选择从 3 更新到 4；RangeSlider 点击下端点保持 20，右方向键更新到 21，Tab 后上端点左方向键更新到 64，焦点环跟随当前端点。
- 212 个挂载/布局场景：每页各检查宽屏 1280×820、指针、浅色、100% 字号，以及 390×820、触摸、深色、200% 字号。
- 上述场景无无效数值/负尺寸几何。正文选择检查覆盖 106 页，跨页壳拖选复制覆盖 15 种页壳。
- 单元/场景验证不等于所有页面已做视觉和交互验收；原生 Windows、Linux、iOS、Android 尚未设备实测。修改位于共享 Rust 实现，未增加平台空实现。
- `clippedOverflow` 是诊断线索，包含可能有意裁剪的控件，不直接认定为缺陷；DataTable、WheelPicker、Carousel、SwipeList、Tree 的线索仍待逐项观察。

本轮检查结果（TextFieldStyle / TextBuffer）：

- Compose：1550 项通过；Core / Platform：495 项通过；Gallery：490 项通过。该数字是此前阶段记录；本轮结果另见下表，不代表全部能力已原生验收。
- Windows SDL 的 UTF-16 组合位置已在 Rust 宿主入口转换，Android / 浏览器沿用同一规范化入口；新增构造器的 surrogate / emoji / 未指定位置测试通过，Platform / Shell 共 20 项通过。`cargo check --workspace --lib --locked` 通过。
- Gallery 组件验收：4 项通过，覆盖逐字搜索/IME 预览/清空、真实文本度量下点击与拖选/多次点击、独立评分状态、LTR/RTL 双端点边界与焦点。
- Gallery 选择页壳：8 项通过；Rating 布局：2 项通过；目录宽/窄屏布局：212 场景通过。
- Website：类型检查与生产构建通过；新增 TextFieldStyle / TextBuffer 两段 Rust 示例编译通过。
- macOS DevTools：同一 Styled text 节点在焦点高亮、编辑、禁用/启用后保持 ID；`é👩🏽‍💻🇨🇳` 显示 3 characters，删除国旗后剩两个字素，左移再删除重音字母后只剩完整 emoji。焦点操作 CPU trace 记录 66 个事件、0 丢失；不据此推断 GPU 性能。
- 严格 Clippy（`-D warnings`）仍被仓库已有的模块同名、参数数量、枚举尺寸等问题阻断；未把它记为通过。

复跑：

```sh
COVE_GALLERY_AUDIT_OUT=/tmp/cove-gallery-audit.json cargo test -p cove-examples --test catalog_audit --locked
cargo test -p cove-examples --test component_acceptance --test selection_shell_tests --test rating_layout_tests --locked
cargo test -p cove-ui-compose --test compose --locked
```

## 逐页队列

“段落数”只统计 `story-section-*` 调试节点，不能据此认定功能缺失。未标为本轮处理的页面仍需对照源能力、逐项执行交互、检查截图并写用户文档。

| 页面 | 宽屏段落数 | 宽屏 / 窄屏裁剪线索 | 状态 |
| --- | ---: | ---: | --- |
| Dashboard | 0 | 0 / 0 | 2026-11-07 分组卡片、数字滚动，截图目检通过；逐帧动效未验收 |
| Button | 5 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| Checkbox | 1 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| Toggle / Switch | 2 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| Slider | 5 | 0 / 0 | 本轮组件/页面修复；文档已写 |
| RangeSlider | 4 | 0 / 0 | 本轮组件/页面修复；文档已写 |
| Rating | 6 | 0 / 0 | 本轮组件/页面修复；文档已写 |
| Text Fields | — | 0 / 0 | 合并原 TextField / SearchField / TextArea / JsonField；段落数需重新测量；字段壳改用 `Field`；待逐项交互和视觉验收 |
| Text Editor | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| Numeric | — | 0 / 0 | 合并原 NumberField / Vec3Field；`Vec3Field` 组合三个 `NumberField` 并共用 `NumberFieldOptions`；待逐项交互和视觉验收 |
| ColorField | — | 0 / 0 | 浮层新增色轮（`ColorWheel`）与独立示例；模型/指针测试通过；待逐项交互和视觉验收 |
| Select | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| AssetRef / DropTarget | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| List | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收；2026-11-07 列表重设计：焦点驱动的强调/中性选中、与选中带同几何的悬停、ListRow 图标/值/徽标/附件 API，DevTools 截图与点击后置条件（选中与强调色带）验证 |
| SectionedList | 0 | 0 / 0 | 挂载/几何基础检查通过；2026-11-07 内嵌盒左右对称（覆盖式滚动条），选中带与分隔线 DevTools 截图目检；交互逐项验收待做 |
| ReorderableList | 0 | 0 / 0 | 拖拽改为实时重排（删除插入线）、落位弹簧、横轴轴向与 6pt 统一阈值；纵向/横轴指针与键盘回归通过；浅/深主题下纵轴与横轴的真机指针拖拽视觉目检已完成，减弱动效仍待目检；文档已写 |
| SwipeList | 0 | 0 / 5 | 挂载/几何基础检查通过；2026-11-07 放入内嵌盒、ListRow 内容、焦点强调选中与操作按钮墨水，DevTools 截图与点击后置条件验证；滑动手势原生验收待做 |
| RefreshableList | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| Scrollbar | 0 | 0 / 0 | 挂载/几何基础检查通过；2026-11-07 页面补上被驱动的文档与常显滑块，End 键后置条件（400 / 400 pt、文档到第 11–15 行）验证 |
| Tree | 0 | 0 / 6 | 拖拽改为实时重排（删除插入线）并共用键盘移动契约；五种参考线样式与 `guide_width`/`guide_position`/`leading_padding`/`trailing_padding` 几何 API；参考线默认 1.5pt 且悬停行自身参考线加深（不重算）；参考线落位改为行顶起算（`Straight` 连续、末子收束止于行中心）、投影祖先列按深度索引、悬停与光标底带并入内容插槽不再叠第二层；行可聚焦与焦点回收回归通过；浅/深主题下三种放置、四种键盘移动、样式与滑杆、悬停加深、拖拽预览列位与源行淡出的真机目检已完成，边缘自动滚动与减弱动效仍待处理；文档已写 |
| VirtualStack | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| Chart | 13 | 8 / 8 | 段落与 Swift 对齐；轴标签、覆盖层分级、图例键盘激活、缩放/刷选交互、数据动画与缓存失效已有回归；待逐项原生视觉验收 |
| Icon | 3 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| Avatar | 4 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| Image | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| Text | 4 | 0 / 0 | 已补齐选区注册、两端标记、Shift 扩选与链接拖选；链接锚点布局/焦点已接入；触控圆形手柄与放大镜仍待完善；用户文档已写 |
| Markdown | 2 | 0 / 0 | 挂载/几何基础检查通过；主题分隔线已由 8pt 色带修正为 1pt 细线（`thematic_break_keeps_its_air_outside_the_hairline`）；待逐项交互和视觉验收 |
| WebView | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| Menu | 3 | 0 / 0 | 挂载/几何基础检查通过；DevTools 实测行高 24、面板内缩 4、行首留白 8、分隔带 11（含 1pt 细线）；减弱动效与关闭时长由结构用例覆盖；逐像素目检因本机无屏幕录制权限未完成 |
| MenuButton | 0 | 0 / 0 | 挂载/几何基础检查通过；Menu 页 `Actions` 菜单实测开启/Escape 关闭均不残留 portal；待逐项交互和视觉验收 |
| ContextMenu | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| Popover | 2 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| Popconfirm | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| ActionSheet | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| Modal | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| DisclosureGroup | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| TabView | 4 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| Panel | 2 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| SplitView | 2 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| ScrollView | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| BoundedScrollView | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| PropertyGrid | 0 | 0 / 0 | 内容列宽度塌陷已修复（`Percent(100)` 填满滚动视口），三种滚动轴、有界父容器与自适应标签回归通过；故事补齐 Swift Inspector 段落（Toggle/NumberField 值槽、空状态）；macOS DevTools 截图目检与折叠/填值后置条件通过；其他平台原生视觉待验收；文档已写 |
| Workspace / Dock | 1 | 0 / 0 | 2026-11-07 补上 Dock workspace 故事；修复槽位拆分不分配空间、分隔条 1pt；标签胶囊滑动选中；DevTools 实测 Editor 208pt / Preview 151pt、分隔条 6pt，切换标签后选中态正确 |
| Box / Row / Column | 6 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| Masonry | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| ZStack / Overlay | 2 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| Animation | 4 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| Viewport Traits | 2 | 0 / 0 | 挂载/几何基础检查通过；设备、姿态、区段行有结构测试；待各预设下的视觉验收 |
| ViewThatFits | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| Theme / Typography | 2 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| Badge | 4 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| Alert | 4 | 0 / 0 | 2026-11-07 段落结构对齐 Swift（Actions & dismissal、Inline feedback），并补 Variants、Density 两段；状态图标改为 SF Symbols 式实心挖空符号，Soft 用 10% 色调底 + 18% 同色发丝边，标题与说明按 2pt 配对，关闭按钮改中性灰；dismiss → Show alert、Disabled 熄灭与关闭失活经 DevTools 后置条件验证；浅色/深色截图目检通过；文档已写（`alert.md`）；其他平台原生视觉待验收 |
| Progress / Spinner | 2 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| EmptyState | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| RadioGroup | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| Form | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| Combobox | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| Notification | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| Tooltip | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| Sidebar | 0 | 0 / 0 | 2026-11-07 贴边全高、中性滑动选中、强调色图标，明暗截图目检通过；同日 Gallery 窗口改为 macOS 侧栏材质（侧栏无填充）、正文放入圆角卡片，DevTools 截图确认卡片与侧栏透明区，系统材质的实际合成未截图验证（无屏幕录制权限） |
| NavigationDrawer | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| NavigationRail | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| AppBar / NavigationBar | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| Table | 2 | 6 / 6 | 段落与 Swift 对齐（10,000 行库存表 + 空数据）；排序循环、隐藏排序列、列宽约束、窄宽丢列、空数据保留表头与单选已有回归；待逐项原生视觉验收 |
| DataTable | 3 | 3 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| Calendar | 2 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| DatePicker | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| DateRangePicker | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| TimeField | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| TimePicker | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| WheelPicker | 0 | 2 / 2 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| Accordion | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| Breadcrumb | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| Pagination | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| ButtonGroup | 0 | 0 / 0 | 2026-11-07 胶囊槽 + 弹簧滑动拇指，截到中途帧 |
| SplitButton | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| Tag | 3 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| Kbd / KeyCap | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| Label | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| Dialog | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| AlertDialog | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| Sheet | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| BottomSheet | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| Command Palette | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| NavigationStack | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| Drag & Drop | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| Drag Upload | 0 | 3 / 0 | 拖放区命中、类型/数量拒绝、批次聚合、目录拒绝、空过滤与 Content-Type 查表有结构回归；DevTools 注入 begin→file→complete 已端到端验证；原生 OS 拖入与原生打开面板尚未验收 |
| HoverCard | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| Timeline | 2 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| Toolbar | 0 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| Link | 3 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| StatusBar | 3 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| Carousel | 5 | 1 / 3 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| Skeleton | 2 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| Stepper | 3 | 0 / 0 | 2026-11-07 重做外观与自适应堆叠：实心状态标记、跨越真实间距的连接线、选中只强调标记与标题；堆叠分支塌陷已修复并有回归；浅/深主题与宽窄布局目检通过；文档已写 |
| Media Player | 2 | 0 / 0 | 2026-11-08 改为画面内控制、弹幕、全屏/画中画、本地文件/拖放/URL 来源；播放、弹幕、控制栏原生目检通过；设置菜单、系统全屏、画中画窗口、HTTPS 待前台补测；文档已写 |
| SingleLineText | 2 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| CompletionPopover | 2 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| RouteOutlet | 2 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| ProgressValue | 2 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| HorizontalSwipe | 2 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| EdgeAutoScroll | 2 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| ScrollReveal | 2 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |
| ActionGroupLayout | 2 | 0 / 0 | 挂载/几何基础检查通过；待逐项交互和视觉验收 |

## Swift 源码对照（逐页差异清单）

更新：2026-11-03。对照对象：`swift/`（Swift 树最后一个完整提交 `9738de61`，删除发生在 `1d633f78`）与当前 `examples/gallery`。

口径：

- 统计单位是 `StorySection`：页面内的一组能力（标题 + 说明 + 示例）。Swift 取 `StorySection("…")`；Rust 取 `StorySection::new("…")` 及同文件内的包装函数（`self.labeled(...)`、`section(...)`），并跟进页面组合的子视图类型。
- 每个 Swift 段落用标题词比对 Rust 段落：同页 ≥50% 词重合算同页有对应；同页对不上但其他页面 ≥80% 算被挪走；都没有算无对应。
- 段数不等于功能数量。Rust 常把一个 Swift 段落拆成多段（Slider 3 → 5），段数多不代表能力更全。
- 另查“孤儿 story”：Rust 里存在、自带段落，但没有被 `StoryDispatch::view` 任何分支引用、也没有被已挂页面组合的类型，即为已实现但没有页面展示。
- 以下是当时静态源码比对的历史快照，不是本轮测试结果。当时工作区因 `crates/compose/src/primitives/text/text_field/` 未完成的改动无法编译（17 个错误），`catalog_audit` 跑不起来，所以本轮没有运行时段落节点数据。

### 结论

- 目录：Swift 98 页全部进入 Rust catalog（106 页），没有丢页；标题、分类、摘要逐字一致（0 处文案差异）。
- 段落：Swift 191 段，Rust 169 段；同页有对应 143 段，被挪到其他页面 2 段，全库找不到对应 46 段。
- 页面级缺口：5 页在 Rust 侧完全没有分段标题；15 页段落数少于 Swift；Workspace 页已于 2026-11-07 补上 story。
- 已实现但没有页面展示的 story：1 个（2 段），见下表。
- 结论：不是整页没迁移。少的是页面内的能力分组、说明和示例密度；部分能力被挪到新页面，部分只是段落标题改了措辞。

### Rust 无分段结构的页面

这些页面在 Swift 里由 1–2 个带标题和说明的段落组成，Rust 侧内容是平铺的，没有分组标题，看起来就是"比 Swift 少"。

| 页面 | Rust story | Swift 段 | Swift 段落标题 |
| --- | --- | ---: | --- |
| ActionSheet | `ActionSheetStory` | 2 | Project actions、Long action lists |
| EmptyState | `EmptyStory` | 1 | Empty content |
| Table | `TableStory` | 2 | Inventory · 10,000 rows、Empty data |
| Toolbar | `ToolbarStory` | 1 | Toolbar |
| Workspace / Dock | Dock workspace | 1 | Dock workspace |

### 已实现但没有页面展示的 story

这些类型带着分段和说明存在于 Rust 源码，`StoryDispatch::view` 没有引用它们，所以 Gallery 里看不到。

| story 类型 | 源文件 | 段 | 段落标题 |
| --- | --- | ---: | --- |
| `LazyGridStory` | layout.rs | 2 | Lazy grid · 100,000 cells、Wide lazy grid · 200 × 200 |

### 段落数少于 Swift 的页面

| 页面 | Swift 段 | Rust 段 | Swift 段落标题 | Rust 现有段落标题 |
| --- | ---: | ---: | --- | --- |
| Box / Row / Column | 10 | 4 | Flex layout、Measured grid、Candidate layouts、Container queries、Intrinsic fitting、Logical edges and safe area、Lazy grid · 100,000 cells、Wide lazy grid · 200 × 200、Estimated rows · 10,000 items、Variable rows · 100,000 items | Measured grid、Masonry sample、Dividers、Overlay |
| Animation | 6 | 4 | Animated visibility、Timing curves、Tracks & phases、Shared geometry、Spring release、Numbers & completion | Animated visibility、Timing curves、Tracks & phases、Numbers & completion |
| Sidebar | 3 | 1 | Workspace navigation、Resizable navigation、Nested navigation | Workspace navigation |
| AppBar / NavigationBar | 2 | 1 | Application header、Host safe area | Application header |
| Chart | 13 | 13 | Trend、Grouped and stacked bars、Below the baseline、Dual axis、Log axis、Histogram、Large series、Scatter、Donut、Sparkline、Horizontal bars、Data table、Explore | Trend、Grouped and stacked bars、Below the baseline、Dual axis、Log axis、Histogram、Large series、Scatter、Donut、Sparkline、Horizontal bars、Data table、Explore |
| Checkbox | 2 | 1 | Selection、Disabled | Selection |
| Combobox | 2 | 1 | Searchable selection、Remote results | Searchable selection |
| Command Palette | 2 | 1 | Command palette、Custom footer | Command palette |
| Image | 2 | 1 | Content modes、Asynchronous loading | Asynchronous loading |
| Label | 2 | 1 | Content labels、Associated field labels | Content labels |
| List | 2 | 1 | Selectable list、Multiple selection | Selectable rows |
| NavigationRail | 2 | 1 | Rail destinations、Retained presentations | Rail destinations |
| NumberField | 2 | 1 | Numeric editing、Locale decimals | Bounded number |
| ReorderableList | 2 | 1 | Reorderable list、Long list | Reordering |
| TimePicker | 2 | 1 | Inline time wheels、Segmented time input | Inline time wheels |

### 同页对不上、但在其他页面能找到的 Swift 段落

能力还在 Rust 里，只是 Swift 原页面不再展示；对照时容易被误判为缺失。

| 原页面 | Swift 段落 | Rust 现所在页面 |
| --- | --- | --- |
| Box / Row / Column | Intrinsic fitting | ViewThatFits |
| Checkbox | Disabled | RangeSlider、Rating、Slider、Toggle / Switch |

### 同页和其他页都找不到对应的 Swift 段落

最可能是真的没展示或没迁移，需要逐条判定后再决定补实现还是补示例。

| 页面 | Swift 段落 | 本页源码关键词命中 | 说明 |
| --- | --- | --- | --- |
| ActionSheet | Project actions | 2/2 | 出现在本页源码，未单独成段 |
| ActionSheet | Long action lists | 3/3 | 出现在本页源码，未单独成段 |
| Animation | Shared geometry | 2/2 | 出现在本页源码，未单独成段 |
| Animation | Spring release | 0/2 | 未出现：spring、release |
| AppBar / NavigationBar | Host safe area | 0/3 | 未出现：safe、area、host |
| Box / Row / Column | Flex layout | 2/2 | 出现在本页源码，未单独成段 |
| Box / Row / Column | Candidate layouts | 1/2 | 未出现：candidate |
| Box / Row / Column | Container queries | 0/2 | 未出现：container、querie |
| Box / Row / Column | Logical edges and safe area | 1/4 | 未出现：safe、logical、area |
| Box / Row / Column | Lazy grid · 100,000 cells | 3/3 | 出现在本页源码，未单独成段 |
| Box / Row / Column | Wide lazy grid · 200 × 200 | 3/3 | 出现在本页源码，未单独成段 |
| Box / Row / Column | Estimated rows · 10,000 items | 1/2 | 未出现：estimated |
| Box / Row / Column | Variable rows · 100,000 items | 1/2 | 未出现：variable |
| Chart | Large series | 2/2 | 已单独成段（`chart-large-series`，2000 采样，numeric x） |
| ColorField | Color picker | 2/2 | 出现在本页源码，未单独成段 |
| Combobox | Remote results | 2/2 | 出现在本页源码，未单独成段 |
| Command Palette | Custom footer | 2/2 | 出现在本页源码，未单独成段 |
| ContextMenu | Context actions | 2/2 | 出现在本页源码，未单独成段 |
| DataTable | TreeTable | 1/1 | 出现在本页源码，未单独成段 |
| DatePicker | Date input | 1/2 | 未出现：input |
| EmptyState | Empty content | 2/2 | 出现在本页源码，未单独成段 |
| Image | Content modes | 2/2 | 出现在本页源码，未单独成段 |
| JsonField | Structured input | 1/2 | 未出现：structured |
| Kbd / KeyCap | Keyboard hints | 1/2 | 未出现：hint |
| Label | Associated field labels | 2/3 | 未出现：associated |
| List | Multiple selection | 1/2 | 未出现：multiple |
| NavigationRail | Retained presentations | 1/2 | 未出现：retained |
| NumberField | Numeric editing | 2/2 | 出现在本页源码，未单独成段 |
| NumberField | Locale decimals | 1/2 | 未出现：locale |
| ReorderableList | Reorderable list | 2/2 | 出现在本页源码，未单独成段 |
| ReorderableList | Long list | 2/2 | 出现在本页源码，未单独成段 |
| ScrollView | Native scroll stream | 1/3 | 未出现：native、stream |
| SearchField | Search input | 2/2 | 出现在本页源码，未单独成段 |
| Select | Typed options | 2/2 | 出现在本页源码，未单独成段 |
| Table | Inventory · 10,000 rows | 1/1 | 已单独成段（`gallery-table`，10,000 行） |
| Table | Empty data | 2/2 | 已单独成段（保留表头 + 空状态） |
| TextArea | Resizable text area | 2/3 | 未出现：resizable |
| TimeField | Time input | 1/2 | 未出现：input |
| TimePicker | Segmented time input | 1/3 | 未出现：input、segmented |
| Toolbar | Toolbar | 1/1 | 出现在本页源码，未单独成段 |
| Tooltip | Managed descriptions | 0/2 | 未出现：managed、description |
| Vec3Field | Transform | 0/1 | 未出现：transform |
| Workspace / Dock | Dock workspace | 2/2 | 已出现：workspace、dock |

### 逐页统计

两侧都是 0 段的页面使用内联组合，没有分段标题，不构成差异。

| 页面 | Swift 段 | Rust 段 | 同页对应 | 挪走 | 无对应 | 判定 |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| Accordion | 1 | 1 | 1 | 0 | 0 | 段落数持平 |
| ActionSheet | 2 | 0 | 0 | 0 | 2 | Rust 无段落结构 |
| Alert | 2 | 4 | 2 | 0 | 0 | Rust 多 2 段（Variants、Density） |
| AlertDialog | 1 | 1 | 1 | 0 | 0 | 段落数持平 |
| Animation | 6 | 4 | 4 | 0 | 2 | 段落少 2 段 |
| AppBar / NavigationBar | 2 | 1 | 1 | 0 | 1 | 段落少 1 段 |
| AssetRef / DropTarget | 1 | 1 | 1 | 0 | 0 | 段落数持平 |
| Avatar | 5 | 5 | 5 | 0 | 0 | 段落数持平 |
| Badge | 4 | 4 | 4 | 0 | 0 | 段落数持平 |
| BottomSheet | 1 | 1 | 1 | 0 | 0 | 段落数持平 |
| BoundedScrollView | 1 | 1 | 1 | 0 | 0 | 段落数持平 |
| Box / Row / Column | 10 | 4 | 1 | 1 | 8 | 段落少 6 段 |
| Breadcrumb | 1 | 1 | 1 | 0 | 0 | 段落数持平 |
| Button | 5 | 5 | 5 | 0 | 0 | 段落数持平 |
| ButtonGroup | 1 | 1 | 1 | 0 | 0 | 段落数持平 |
| Calendar | 2 | 2 | 2 | 0 | 0 | 段落数持平 |
| Carousel | 5 | 5 | 5 | 0 | 0 | 段落数持平 |
| Chart | 13 | 13 | 13 | 0 | 0 | 段落数持平 |
| Checkbox | 2 | 1 | 1 | 1 | 0 | 段落少 1 段 |
| ColorField | 1 | 1 | 0 | 0 | 1 | 段落数持平 |
| Combobox | 2 | 1 | 1 | 0 | 1 | 段落少 1 段 |
| Command Palette | 2 | 1 | 1 | 0 | 1 | 段落少 1 段 |
| ContextMenu | 1 | 1 | 0 | 0 | 1 | 段落数持平 |
| Dashboard | 0 | 0 | 0 | 0 | 0 | Rust 无段落结构 |
| DataTable | 2 | 3 | 1 | 0 | 1 | 段落多于 Swift |
| DatePicker | 1 | 1 | 0 | 0 | 1 | 段落数持平 |
| DateRangePicker | 1 | 1 | 1 | 0 | 0 | 段落数持平 |
| Dialog | 1 | 1 | 1 | 0 | 0 | 段落数持平 |
| DisclosureGroup | 1 | 1 | 1 | 0 | 0 | 段落数持平 |
| Drag & Drop | 1 | 1 | 1 | 0 | 0 | 段落数持平 |
| Drag Upload | 0 | 5 | 0 | 0 | 0 | Rust 新增页（Swift 无对应）；Drop to upload / Any type / Archives / Narrow filter / 状态 |
| EmptyState | 1 | 0 | 0 | 0 | 1 | Rust 无段落结构 |
| Form | 1 | 1 | 1 | 0 | 0 | 段落数持平 |
| HoverCard | 1 | 1 | 1 | 0 | 0 | 段落数持平 |
| Icon | 3 | 3 | 3 | 0 | 0 | 段落数持平 |
| Image | 2 | 1 | 1 | 0 | 1 | 段落少 1 段 |
| JsonField | 1 | 1 | 0 | 0 | 1 | 段落数持平 |
| Kbd / KeyCap | 1 | 1 | 0 | 0 | 1 | 段落数持平 |
| Label | 2 | 1 | 1 | 0 | 1 | 段落少 1 段 |
| Link | 3 | 3 | 3 | 0 | 0 | 段落数持平 |
| List | 2 | 1 | 1 | 0 | 1 | 段落少 1 段 |
| Markdown | 2 | 2 | 2 | 0 | 0 | 段落数持平 |
| Masonry | 1 | 1 | 1 | 0 | 0 | 段落数持平 |
| Menu | 3 | 3 | 3 | 0 | 0 | 段落数持平 |
| MenuButton | 1 | 1 | 1 | 0 | 0 | 段落数持平 |
| Modal | 1 | 1 | 1 | 0 | 0 | 段落数持平 |
| NavigationDrawer | 1 | 1 | 1 | 0 | 0 | 段落数持平 |
| NavigationRail | 2 | 1 | 1 | 0 | 1 | 段落少 1 段 |
| NavigationStack | 1 | 1 | 1 | 0 | 0 | 段落数持平 |
| Notification | 1 | 1 | 1 | 0 | 0 | 段落数持平 |
| NumberField | 2 | 1 | 0 | 0 | 2 | 段落少 1 段 |
| Pagination | 1 | 1 | 1 | 0 | 0 | 段落数持平 |
| Panel | 2 | 2 | 2 | 0 | 0 | 段落数持平 |
| Popconfirm | 1 | 1 | 1 | 0 | 0 | 段落数持平 |
| Popover | 1 | 2 | 1 | 0 | 0 | 段落多于 Swift |
| Progress / Spinner | 2 | 2 | 2 | 0 | 0 | 段落数持平 |
| PropertyGrid | 1 | 1 | 0 | 0 | 1 | 段落数持平 |
| RadioGroup | 1 | 1 | 1 | 0 | 0 | 段落数持平 |
| RangeSlider | 1 | 4 | 1 | 0 | 0 | 段落多于 Swift |
| Rating | 6 | 6 | 6 | 0 | 0 | 段落数持平 |
| RefreshableList | 1 | 1 | 1 | 0 | 0 | 段落数持平 |
| ReorderableList | 2 | 1 | 0 | 0 | 2 | 段落少 1 段 |
| Text | 4 | 4 | 4 | 0 | 0 | 段落数持平 |
| Scrollbar | 1 | 1 | 1 | 0 | 0 | 段落数持平 |
| ScrollView | 1 | 1 | 0 | 0 | 1 | 段落数持平 |
| SearchField | 1 | 1 | 0 | 0 | 1 | 段落数持平 |
| SectionedList | 1 | 1 | 1 | 0 | 0 | 段落数持平 |
| Select | 1 | 1 | 0 | 0 | 1 | 段落数持平 |
| Sheet | 1 | 1 | 1 | 0 | 0 | 段落数持平 |
| Sidebar | 3 | 1 | 3 | 0 | 0 | 段落少 2 段 |
| Skeleton | 2 | 2 | 2 | 0 | 0 | 段落数持平 |
| Slider | 3 | 5 | 3 | 0 | 0 | 段落多于 Swift |
| SplitButton | 1 | 1 | 1 | 0 | 0 | 段落数持平 |
| SplitView | 2 | 2 | 2 | 0 | 0 | 段落数持平 |
| StatusBar | 3 | 3 | 3 | 0 | 0 | 段落数持平 |
| Stepper | 3 | 3 | 3 | 0 | 0 | 段落数持平 |
| SwipeList | 1 | 1 | 1 | 0 | 0 | 段落数持平 |
| Table | 2 | 2 | 2 | 0 | 0 | 段落数持平 |
| TabView | 3 | 4 | 3 | 0 | 0 | 段落多于 Swift |
| Tag | 2 | 3 | 2 | 0 | 0 | 段落多于 Swift |
| Text Editor | 1 | 1 | 1 | 0 | 0 | 段落数持平 |
| TextArea | 1 | 1 | 0 | 0 | 1 | 段落数持平 |
| TextField | 4 | 4 | 4 | 0 | 0 | 段落数持平 |
| Theme / Typography | 2 | 2 | 2 | 0 | 0 | 段落数持平 |
| TimeField | 1 | 1 | 0 | 0 | 1 | 段落数持平 |
| Timeline | 2 | 2 | 2 | 0 | 0 | 段落数持平 |
| TimePicker | 2 | 1 | 1 | 0 | 1 | 段落少 1 段 |
| Toggle / Switch | 2 | 2 | 2 | 0 | 0 | 段落数持平 |
| Toolbar | 1 | 0 | 0 | 0 | 1 | Rust 无段落结构 |
| Tooltip | 1 | 1 | 0 | 0 | 1 | 段落数持平 |
| Tree | 1 | 1 | 1 | 0 | 0 | 段落数持平 |
| Vec3Field | 1 | 1 | 0 | 0 | 1 | 段落数持平 |
| Viewport Traits | 1 | 1 | 1 | 0 | 0 | 段落数持平 |
| ViewThatFits | 1 | 1 | 1 | 0 | 0 | 段落数持平 |
| VirtualStack | 1 | 1 | 1 | 0 | 0 | 段落数持平 |
| WebView | 1 | 1 | 1 | 0 | 0 | 段落数持平 |
| WheelPicker | 1 | 1 | 1 | 0 | 0 | 段落数持平 |
| Workspace / Dock | 1 | 1 | 1 | 0 | 0 | 段落数持平 |
| ZStack / Overlay | 2 | 2 | 2 | 0 | 0 | 段落数持平 |

### Rust 独有页面

Swift 目录没有、Rust 新增的 8 页：SingleLineText、CompletionPopover、RouteOutlet、ProgressValue、HorizontalSwipe、EdgeAutoScroll、ScrollReveal、ActionGroupLayout。

### 已知限制

- 静态解析会漏掉变量标题、循环生成或运行时拼接的段落；Swift 侧无法运行，没有交叉验证。
- 标题比对按词重合度，措辞改动大时会被记成"无对应"，需要人工确认。
- 不比控件变体数量、尺寸、间距、颜色和交互；这些要靠 DevTools 逐页做视觉与交互验收。

## 2026-11-03 Swift 行为断言清理

逐项对照 `swift/Compose/Sources/CoveUICompose/`、`swift/Sources/CoveUIWorkspace/WorkspaceView.swift`、`swift/Text/Sources/CoveUIText/FontProvider.swift` 与 `swift/Terminal/`。测试说明中的平台桥接、命名和构建依赖差异不作为错误行为断言保留。

| 页面/组件 | 实现与自动化验收 | 原生视觉/交互验收 | Rust 文档 |
| --- | --- | --- | --- |
| Sidebar / AdaptiveSidebar | 共享 Rust 行为与场景回归 | 本轮未作原生设备验收 | Website/src/content/components/sidebar.md |
| NavigationDrawer | 共享 Rust 行为与场景回归 | 本轮未作原生设备验收 | Website/src/content/components/navigation-drawer.md |
| WorkspaceView | 共享 Rust 行为与场景回归 | 本轮未作原生设备验收 | Website/src/content/components/workspace.md |
| ScrollView | 共享 Rust 行为与场景回归 | 本轮未作原生设备验收 | Website/src/content/components/scroll-view.md |
| Scrollbar | 共享 Rust 行为与场景回归 | 本轮未作原生设备验收 | Website/src/content/components/scrollbar.md |
| RouteOutlet | 共享 Rust 行为与场景回归 | 本轮未作原生设备验收 | Website/src/content/components/route-outlet.md |
| AsyncImage | 共享 Rust 行为与场景回归 | 本轮未作原生设备验收 | Website/src/content/components/async-image.md |
| HoverCard | 共享 Rust 行为与场景回归 | 本轮未作原生设备验收 | Website/src/content/components/hover-card.md |
| CommandPalette | 共享 Rust 行为与场景回归 | 本轮未作原生设备验收 | Website/src/content/components/command-palette.md |
| Menu / MenuItemContent | 共享 Rust 行为与场景回归 | 本轮未作原生设备验收 | Website/src/content/components/menu.md |
| Toolbar / AppBar | 共享 Rust 行为与场景回归 | 本轮未作原生设备验收 | Website/src/content/components/toolbar.md |
| Vec3Field | 共享 Rust 行为与场景回归 | 本轮未作原生设备验收 | Website/src/content/components/vec3-field.md |
| Timeline | 共享 Rust 行为与场景回归 | 本轮未作原生设备验收 | Website/src/content/components/timeline.md |
| TabView | 共享 Rust 行为与场景回归 | 本轮未作原生设备验收 | Website/src/content/components/tab-view.md |
| DateRangeCalendar | 共享 Rust 行为与场景回归 | 本轮未作原生设备验收 | Website/src/content/components/date-range.md |
| TextField / Code gutter | 挂载控件与正文共用几何；真实场景指针点击、输入框 F9/折叠快捷键、标记重基 | 本轮未作原生键盘或 IME 验收 | Website/src/content/components/text-field.md |

未增加按平台跳过的生产实现；原生 Windows、Linux、iOS、Android 的设备验收继续独立跟踪。

逐项源码与断言处理记录见 [swift-gap-audit.md](swift-gap-audit.md)。Resource preparation 自动资源所有权与取消回收的 Rust 文档位于 `Website/src/content/components/resource-preparation.md`；Metal 提交错误与资源回收的原生 API 回归独立于 Gallery 视觉验收。

| 关联组件 | 实现与自动化验收 | 原生视觉/交互验收 | Rust 文档 |
| --- | --- | --- | --- |
| ActionGroupLayout | 软提议与堆叠约束测量分开，关联 Grid/Markdown 横向滚动回归 | 本轮未作原生设备验收 | Website/src/content/components/action-group-layout.md |
| Grid / named columns | 保留必需列固有宽度，收窄后滚轮移动视口；9 项定向场景 | 本轮未作原生设备验收 | Website/src/content/components/grid.md |
| MarkdownView | 长代码不换行并横向滚动，正文换行；14 项定向场景 | 本轮未作原生设备验收 | Website/src/content/components/markdown.md |

### 2026-11-04 最终验证

- `cargo test --workspace --locked --no-fail-fast`：退出码 0，3534 项通过、0 失败、5 项沿用原有忽略（2 项性能测量、3 项文档示例）。未新增忽略或跳过来隐藏回归。
- Gallery `catalog_audit`：106 页、212 个宽屏/窄屏几何场景通过。以上组件的实现与自动化验收通过；原生视觉、平台键盘与 IME 验收仍按各行单独记录。
- 网站 `npm run build`、本轮 Rust 文件格式检查与 `git diff --check` 通过。
- `crates/` 与 `examples/` Rust 扫描：缺失行为前缀断言/说明残留 0。剩余 7 处冒号命中均为合法的 GAP 常量声明。
