# Tree

`Tree` 是虚拟化的层级列表：只投影已展开分支，行高固定，可见行才挂载。选择、勾选、展开、搜索、懒加载与拖拽都是独立配置组，可以按需组合。

```rust
let tree = Tree::new(
    roots,
    |node: &Node| node.id.clone(),        // nodeKey / fieldNames.key
    |node: &Node| node.children.clone(),  // fieldNames.children
    |node: &Node, state: RowState| AnyView::new(Text::new(node.title.clone())),
)
.has_children(|node: &Node| node.has_children) // isLeaf 的补集
.options(TreeOptions {
    selection: TreeSelection::local(["scene".to_string()]),
    check: TreeCheck {
        is_enabled: true,
        default_checked: vec!["camera".to_string()],
        ..TreeCheck::default()
    },
    ..TreeOptions::default()
});
```

`id_of`、`children` 与 `has_children` 就是 `fieldNames` / `nodeKey` / `isLeaf` 在类型化 Rust 里的形态：字段名由取值闭包代替，节点标识与子节点解析保持编译期类型安全。

## 选择

`TreeSelection::ids` 是受控单选或多选；`TreeSelection::local` / `local_multiple` 使用树自身持有的状态，等价于非受控的 `defaultSelectedKeys`。

- `is_selectable: false`（或 `TreeSelection::disabled()`）关闭行选择，但方向键仍移动光标。
- `expanded` / `expanded_keys` 是受控展开；`default_expanded` / `default_expanded_keys` 只作为首次挂载的种子。
- `get_selected_ids()`、`get_selected_keys()`、`get_current_key()` 读取状态。
- `events.on_select_change(node, selected, selected_keys)` 在选择变化后回调；旧的 `on_select(node)` 保留。
- `on_node_click(node, modifiers)`、`on_double_click(node)`、`on_node_context_menu(node, point)` 分别覆盖点击、双击与右键。

## 勾选

`TreeCheck` 打开复选框：

- `checked` / `checked_keys` 为受控状态，`default_checked` 为非受控种子。
- 默认父子级联：勾选分支会勾选整棵子树；子节点变化会重算祖先的选中/半选状态。
- `check_strictly: true` 关闭级联，父子互不影响，也不会产生半选。
- `get_checked_keys()`、`get_half_checked_keys()`、`get_checked_nodes()` 读取结果；
  `get_checked_nodes_filtered(leaf_only, include_half_checked)` 对应带过滤参数的
  `getCheckedNodes`。
- `set_checked_keys(keys, leaf_only)`、`set_checked(key, checked, deep)`、
  `set_checked_element(node, checked, deep)` 写入；`deep = Some(false)` 让单次调用按
  `checkStrictly` 处理。
- 可勾选时 `Space` 切换当前行的勾选；复选框是独立控件，点击它不会改变行选择。
- `check.on_change(node, checked, checked_keys, half_checked_keys)` 在每次变化后回调。

## 展开

`TreeExpansion` 提供策略层：

- `default_expand_all`：首次挂载展开所有分支。
- `auto_expand_parent`：展开、`scroll_to_key`、`set_current_key` 时补齐祖先路径。
- `accordion`：同一层级同时只保留一个展开的分支。
- `events.on_expand_change(node, expanded, expanded_keys)` 报告变化。
- `expand_all()`、`collapse_all()`、`expand_key(key)`、`collapse_key(key)`、`toggle_key(key)`。

## 搜索与过滤

`TreeSearch`：

- `query` 为静态查询；`Tree::filter(value)` 写入编程式查询并覆盖静态值，直到
  `clear_filter()`。
- `text` 提取行文本；`filter` 是自定义谓词，接收节点和归一化后的查询，优先于文本匹配。
- `ignore_case`、`match_mode`（`Contains` / `Prefix` / `Exact`）控制比较。
- `policy` 为 `FilterAndAutoExpand` 时隐藏非匹配并展开匹配祖先；`HighlightOnly` 只标记
  `is_search_hit`，保留所有已展开行。
- `scroll_to_key(key)` 会先展开祖先，再让下一次投影滚动到该行。

## 懒加载

`TreeLazy` 让宿主决定何时加载子节点：

- `is_leaf(node)` 显式标记叶子；未加载的分支仍然显示展开控件。
- `load_data(node)` 在分支首次展开时调用一次；宿主把子节点写回自己的数据并记录
  `loaded_keys`。
- `loaded_keys` 可以是受控绑定，否则由树持有；`get_loaded_keys()` 读取。
- `is_loading(node)` 为真的行用进度环替代展开箭头，适合宿主异步加载期间显示。
- 展开/收起/再次展开不会重复请求已经记录的节点。

## 拖拽

`TreeDragOperations`：

- `is_enabled` 与 `can_drag(node)` 控制哪些行可以拖动。
- `can_drop(source, target, position)` 决定放置是否合法；位置是 `Before` / `Inside` / `After`，
  按行高的上下 28% 判定，中间区域进入目标节点。
- `on_drag_start`、`on_drag_enter`、`on_drag_leave`、`on_drop`、`on_drag_end` 覆盖完整生命周期。
- 拖动超过 `DRAG_THRESHOLD`（6pt）死区才进入拖拽，点击不会被误判。投影只改位移不改数据：展平顺序、布局与命中
  仍由宿主模型决定，每行按 `(投影槽位 - 自身索引) * 行距` 位移，源行降到 0.38 不透明度，
  其余行用弹簧跟随新槽位。松手后模型落位与拖拽中绘制的位置一致，不会二次跳动。
- `on_drop` 是唯一入口：键盘移动走同一份 `can_drag` / `can_drop` / `on_drop` 契约。
- 命中判定只用行的绝对 frame：行本身铺满整行插槽，缩进留白与展开控件都在 frame 内，
  因此落在缩进列里即命中该行，树左侧的留白不会被扩大的命中区误吞。
- 放进折叠分支的 `Inside` 放置会自动展开目标，避免移动后的行被隐藏。

键盘移动（行获得焦点后）：

- `Alt+↑` / `Alt+↓`：与上一个/下一个兄弟交换，等价于 `Before` / `After` 放置。
- `Alt+Shift+←`：移出父节点，放在父节点之后。
- `Alt+Shift+→`：放进上一个兄弟，等价于 `Inside` 放置。
- `Alt+→` / `Alt+←`：展开全部 / 收起全部。
- 被 `can_drop` 拒绝的组合退回普通方向键导航，不会吞掉按键。

## 虚拟滚动

行栈默认由 `FixedVirtualStack` 虚拟化，可视区外只保留超出扫描行。`TreeLayout::virtual_scroll(false)`
会改为在 `ScrollView` 中挂载全部可见行，适合行内有长期存活的编辑器；此时
`scroll_to_key` 用行高换算滚动偏移。

`TreeLayout` 的几何字段都有同名链式方法：`row_height`、`row_spacing`、`indentation`、
`disclosure_width`、`trailing_slot_width`、`block_node`（选择带是否满行）、`leading_padding`
（第一层缩进前的留白）、`trailing_padding`（尾部插槽后的留白）。参考线由
`guide_style`、`guide_width`、`guide_position` 控制：`guide_width` 单位为 pt 且不随字号缩放，
默认 1.5；`guide_position` 取 0.0（贴住缩进阶梯前沿）到 1.0（贴住后沿），默认 0.5。
`show_line(bool)` 保留为 `guide_style` 的 `Straight` / `None` 简写。

```rust
let layout = TreeLayout::default()
    .guide_style(TreeGuideStyle::Elbow)
    .guide_width(2.0)
    .guide_position(0.38)
    .indentation(20.0)
    .leading_padding(8.0)
    .trailing_padding(8.0);
```

## 样式与插槽

- `row_content` 即 `titleRender`：完整接管行内容。
- `TreeSlots::icon` 在展开控件/复选框之后、标题之前插入前导图标。
- `TreeSlots::disclosure` 替换默认的展开/收起箭头，接收展开状态。
- `TreeSlots::trailing` 追加尾部操作区。
- `TreeRowStyle` 控制整行底色与选中带；默认样式使用主题的 accent 选中带、
  `state_layer_hover` 悬停、`state_layer_selected` 搜索命中，并为键盘光标绘制较弱的
  `state_layer_selected` 底。悬停与键盘光标的底带画在行内容所在的插槽内，与选中带共用
  左边界和圆角，不会再叠出第二层；缩进与展开控件所在的层级留白始终不被高亮覆盖。
- 缩进参考线有五种 `TreeGuideStyle`：`None`（只靠缩进）、`Straight`（默认，每层一条竖线，
  最深一列在没有后续兄弟时止于行中心）、`Elbow`（最深一列折进本行）、`Dashed`、`Dotted`。
  第 `level` 列对应深度 `level + 1` 的祖先，根层不占列，展平与拖拽投影共用这一约定。
  参考线表面与行等高，相邻行的竖线首尾接成一条连续线；`Dashed` / `Dotted` 的分段相位按
  绝对 y 排布，跨行不重新起相。线宽由 `guide_width` 决定。
- 鼠标悬停某一行时加深的只有这一行悬挂其上的那一层主干：同一父节点下整组兄弟共用的一列
  参考线一起变深，包括指针上方与下方的兄弟带；祖先层的列、其它分组的列保持默认墨色。
  `Elbow` 的横向连接段属于每一行自己的标签，只有指针所在的那一行弯头加深，同组兄弟的弯头
  不变。加深在绘制阶段按悬停记录完成，不触发行重算；只有自定义 `TreeRowStyle` 或配置了
  `trailing` 插槽的行才按悬停重算。悬停行被折叠或过滤移出树时，主干立刻回到默认墨色。
  拖动中的源行连同它的参考线一起降低不透明度。

## 命令式方法

| 方法 | 作用 |
| --- | --- |
| `filter(value)` / `clear_filter()` | 编程式过滤 |
| `get_checked_keys()` / `get_checked_nodes()` | 读取勾选 |
| `get_half_checked_keys()` | 读取半选 |
| `set_checked_keys(keys, leaf_only)` | 替换勾选 |
| `set_checked(key, checked, deep)` / `set_checked_element(node, checked, deep)` | 设置单节点 |
| `check_all()` / `uncheck_all()` | 全选/全不选 |
| `get_current_key()` / `get_current_node()` / `set_current_key(key)` / `clear_current_key()` | 光标读写 |
| `get_path(key)` / `get_key_path(key)` | 根到目标的元素/键路径 |
| `scroll_to_key(key)` / `scroll_to_key_aligned(key, align)` / `expand_to(key)` | 展开路径并滚动；`TreeScrollAlign` 支持 `Auto` / `Start` / `Center` / `End` |
| `get_expanded_keys()` / `get_expanded_ids()` | 展开状态 |
| `append(data, parent)` / `append_at(parent_key, data)` | 追加节点（委托宿主） |
| `insert_before(reference, data)` / `insert_after(reference, data)` | 同级插入 |
| `remove(node)` / `remove_key(key)` | 删除 |
| `update_children(parent, children)` / `update_children_at(key, children)` | 替换子节点 |

结构性编辑通过 `TreeEdit` 回调交给宿主模型：树不拥有节点存储，因此这些方法在未安装回调时
是空操作。

## 可访问性

容器暴露 tree 角色，行暴露 tree item 角色并报告选中、展开与勾选（`checked` /
`mixed` / `unchecked`）值；复选框本身是 checkbox 角色，展开控件是带 `Expand` /
`Collapse` 标签的按钮。行本身可聚焦，键盘支持方向键、Home/End、Return/Space，以及上文
“拖拽”一节的 `Alt` 移动与展开/收起快捷键；收起整棵树会移除当前焦点行，焦点会自动回到树容器，
后续快捷键仍然有效。

共享 Rust 实现覆盖全部宿主平台。Gallery 的 Scene hierarchy 页演示搜索、多选、级联/严格勾选、手风琴、懒加载 `Objects` 分支、前导图标、拖拽与 `Alt` 键盘移动，以及参考线样式与 `Indent` / `Lead` 两个几何滑杆；结构、场景与浅/深主题挂载验证记录在 `Docs/gallery-audit.md`。
