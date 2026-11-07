# Stepper

`Stepper::try_new(items, selection)` 是受控的流程导航器：选中项、每步结果与可用性都由调用方持有，组件只通过 `Binding<Option<Id>>` 读选并回写。`id` 重复时返回 `DuplicateId`。

`StepperItem::new(id, title)` 声明一步，`.detail(...)` 提供次级文字，`.status(...)` 设定结果，`.enabled(false)` 锁定该步。选择某步或完成某步都不会改写其他步的状态：回看更早的步时，之后的结果原样保留。

标记圆盘按状态着色，直径跟随 `ControlSizeEnvironment`。已完成是 success 实心加勾，失败是 error 实心加叉，进行中是 accent 实心加指示器，等待是描边空心加序号；当前步若尚未有结果，也用 accent 实心标记。连接线位于该步单元格内，横跨到下一个标记的真实间距（横向为 1pt 发丝线，纵向为连续竖轨），颜色取自它离开的那一步的结果：success、error 或中性 `border`。选中只强调标记与标题（标题用 accent），不在整行背后画矩形；hover/按下用状态层，键盘焦点用焦点环，禁用时整步透明度 0.5。状态与交互切换使用主题的 fast 动效。

`StepperOptions::axis` 默认 `Horizontal`：组件测量自身容器宽度，宽度不足预算时堆叠为纵向列表并把方向键换成 Up/Down。预算为 `max(80, minimum_horizontal_step_width) * 步数 + (步数 - 1) * 32`，默认 `minimum_horizontal_step_width` 为 148，因此三步需要 508pt。标题长于预算时应提高该值而不是让单元格压缩。`StepperAxis::Vertical` 始终堆叠，跳过测量。

键盘导航是一个 roving 组：整组只占一个 Tab 停靠点，Left/Right（纵向时为 Up/Down）在可用步之间移动，Home/End 跳到首尾，`allows_wrap` 决定越界是否回绕，`is_enabled(false)` 让点击与按键都不生效。无障碍语义为 `List` 套 `listItem`，每步的 value 与工具提示同为 “Step i of n · 状态”。`label` 默认 “Steps”。

共享 Rust 实现覆盖所有宿主平台。自动化场景验证与原生视觉、键盘验收分别记录在 Gallery 验收表中。
