# 作用域状态迁移

状态归视图的组合作用域持有。生产视图在 `body(&self, cx: &Scope)` 中调用 `cx.state`，
通过 `Signal` 读写和生成 `Binding`；构造函数只接收配置与外部绑定。父视图重建时保留状态，
作用域卸载时释放。非响应式资源使用 `cx.remember`。

## 2026-11-02 批次

| 视图 | 已迁移状态与验证 |
| --- | --- |
| `CalendarView` | 浏览月份、当前面板、日期焦点。覆盖跨月键盘移动、PageDown 后回车提交、年月面板切换、年份翻页和禁用焦点清理 |
| `Combobox` | 打开状态、查询、候选项、高亮与请求代次。覆盖父视图重建后的输入身份和查询保留、防抖、关闭取消与提交 |
| `NavigationStack` | 已接受的路径、过渡锁与返回手势状态。覆盖过渡中拒绝连续路径长度变更、结束后接受新路径 |
| `Sidebar` | 非受控展开集合。覆盖本地展开、单一 Tab 停靠点、父子方向键、RTL 折叠与自适应外壳 |
| Gallery `CalendarStory` | 日期、周、月、年选择及对应说明文字，覆盖挂载后的初始呈现 |

同批修复 Popconfirm 触发器再次点击关闭、气泡按指定侧边空间收窄，以及关闭滑动行的无障碍和命中测试。
Combobox 原生查询在工作线程执行，经 `SignalSender` 投递到 UI；Wasm 沿用同步数据源接口，
在防抖到期后调用。所有平台均检查请求代次，关闭或卸载后不发布过期结果。

## 剩余清理范围

2026-11-02 按直接声明 `fn dynamic_properties` 盘点：

| 目录 | 文件数 | 方法数 |
| --- | ---: | ---: |
| `crates/compose/src` | 29 | 29 |
| `examples/gallery/src` | 35 | 67 |

这是旧动态字段声明的数量，不等同于全部 `State` 调用量；未声明动态字段的视图、数据模型、
其他 crate 和测试也需要审计。旧 `State`、`DynamicField` 及协调兼容代码继续保留，
待生产、示例和测试调用迁移完成后一起删除。

后续按状态生命周期分批迁移：

1. 布局与虚拟化：`Grid`、`LazyGrid`、三种虚拟栈、`AdaptiveSidebar`、`SplitView`、
   `Accordion`、`Carousel`、方向解析。
2. 数据控件：各类 List、Tree、Table、PropertyGrid、Chart；同时验证选择、滚动与重排后的身份。
3. 控件、动画与辅助视图：Button、NumberField、Toolbar、动画内容/轨道/阶段、代码块/行号、拖放和 Pet。
4. Gallery 外壳、故事与移动示例，以及其余 crate 和测试中的旧调用。

每批验证父视图重建、卸载后重新挂载、真实输入和绑定驱动的重组；异步会话另验证取消与过期回调。
不要通过保留旧对象引用绕过作用域生命周期。

盘点命令：

```sh
rg -n 'fn dynamic_properties' crates/compose/src examples/gallery/src
rg -n '\b(State|DynamicField)\b' crates examples --glob '*.rs'
```

本批未处理的既有行为差异仍保留准确的 GAP：例如侧栏跨紧凑模式 portal 边界重挂载、
滑动行的物理方向和未缩放操作宽度。它们不应被计入本批已完成项。

## 验证

本机 `aarch64-apple-darwin`：

- `cargo check --workspace --locked` 通过。
- Core 单元测试 115 项、Gallery 单元测试 106 项及导航集成测试 4 项通过。
- Compose 14 个相关测试目标共 91 项通过，覆盖本批修复、List/Tree 与标签页回归。
- `cargo clippy -p cove-ui-compose --lib` 通过，仍有仓库既有警告；一并修正近似常量与两处 FFI 安全说明，解除既有 lint 错误。

2026-11-02 GAP 断言清理后，`menu_hierarchy_tests` 12 项通过：

- 包含在行内的多行标题改为包含关系断言，删除“应溢出”的旧预期。
- 菜单视口的行数预算跟随文字缩放，容纳实际增长的多行行高。
- `MenuWindowBounds` 合并 ScrollView 原有的布局回调，恢复滚动范围更新；滚轮用例不再停在偏移 0。

菜单的其余会话差异、List 选中背景的水平位置，以及未声明动态字段的内部会话继续纳入后续审计。
