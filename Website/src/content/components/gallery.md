# Gallery 使用

Gallery 展示 Cove 的 Rust 组件。组件页的 Size、Scale、Disabled 和 Reset example 分别调整密度、文字缩放、交互状态，并将当前示例恢复到初始值。

在 Windows 上运行 `cargo run -p cove-examples --bin gallery` 打开 Gallery。Windows 默认使用 Direct3D 12，设备初始化失败时尝试 Vulkan。传入 `-- --component dashboard` 或 `-- --component calendar` 可直接打开对应页面。

浏览器版可用 `?component=scrollView` 或 `?component=textFields` 打开对应页面。在画布内滚动或点击字段输入时，事件会交给同一套 Rust 组件。

Navigate 的 Dashboard 和 Calendar 共用一份菜单定义。Windows 在 Cove 标题栏显示 Navigate，可用 `-- --menu-bar separated` 或侧栏底部按钮切换分割线。macOS 在屏幕顶部的系统菜单栏显示同一个 Navigate。Linux、移动端和 Web 可从侧栏访问这两个页面。`-- --menu-bar none` 关闭 Gallery 提供的菜单。Rust 用法见 [窗口菜单栏](/docs/window-menu-bar)。

侧栏底部的主题按钮同时更新 Gallery 正文和 Windows 标题栏。

Dashboard 的 Asset import 卡片用 [Badge](/docs/badge) 显示当前状态。点击 Pause 或 Resume 暂停或继续导入，Advance 推进进度，完成后显示 Complete。

Graphics → [Ray-Traced Shadows](/docs/ray-traced-shadows) 可移动面光源并切换阴影渲染方式。用 `-- --component ray-traced-shadows` 直接打开。

侧栏搜索按名称、分类和简介过滤导航，不改变当前页面。输入多个词时，条目需同时匹配这些词。按 Escape 或点击清空按钮恢复完整导航。

当前已补充使用说明的页面：

- [Slider](/docs/slider)：连续值、步长、刻度、垂直方向与禁用。
- [RangeSlider](/docs/range-slider)：两个端点、步长、最小间距与禁用。
- [ReorderableList](/docs/reorderable-list)：拖拽手柄、拖动中的实时重排、落位弹簧、纵向与横向轴向与边缘自动滚动。
- [Rating](/docs/rating)：整星、分数、自定义图形、只读、密度与布局方向。
- [SearchField](/docs/search-field)：查询绑定、建议、提交与清空。
- [TextField](/docs/text-field)：原生输入、选区、剪贴板与当前能力范围。
- [文本选择](/docs/text-selection)：正文拖选、复制与控件手势边界。

输入类控件的 Gallery 页面已收敛为两组：Numeric（NumberField + Vec3Field）与 Text Fields（TextField / SearchField / TextArea / JsonField）；上表列的是各自的组件文档。

浅色和深色、紧凑窗口、触摸输入、从右到左与较大文字应分别检查。示例数量不代表全部功能都已验收；其他页面的使用文档随组件核对与修复继续补充。
