# 视图与布局

Cove 的界面是 Rust 代码：视图是普通结构体，实现 `View` 后由框架负责构建、布局、绘制与局部失效。没有模板语言，也没有命令式的重绘入口。

```rust
use cove_ui::prelude::*;

struct Greeting {
    name: String,
}

impl View for Greeting {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(Text::new(format!("你好，{}", self.name))).padding(12.0)
    }
}
```

`body` 返回 `AnyView`，分支因此可以用同一类型表达。视图是值：一次性布局直接写构造表达式，不要为它新增结构体。

```rust
fn retry_row(attempt: u32) -> AnyView {
    if attempt == 0 {
        AnyView::new(Text::new("首次请求"))
    } else {
        AnyView::new(Text::new(format!("重试 {attempt} 次")))
    }
}
```

## 状态驱动

`State` 持有可变值：读取登记依赖，写入只让订阅它的视图失效。

```rust
use cove_ui::prelude::*;

let count = State::new(0_i32);
let label = AnyView::new(Text::new(count.get().to_string()));
let increment = {
    let count = count.clone();
    Button::new(Text::new("加一"), move || count.update(|value| *value += 1))
};
```

`State::binding()` 返回 `Binding`，是控件（`TextField`、`Select`、`TabView`）的标准输入；跨视图共享同一状态时克隆 `State`，不要缓存读出的值。

## 布局容器

几何只有一个引擎：Taffy 的 Flexbox + CSS Grid + `calc`。容器 API 是它的一层映射，不另起算法。

| 容器 | 对应 CSS | 典型用途 |
|------|----------|---------|
| `Column::new(alignment, spacing, content)` | `flex-direction: column` | 垂直排列 |
| `Row::new(alignment, spacing, content)` | `flex-direction: row` | 水平排列 |
| `BoxView::new(direction, align, justify, wrap, spacing, content)` | 完整 flex 控制 | 需要 `justify` / `wrap` 的容器 |
| `ZStack` | 层叠 | 覆盖与悬浮层 |
| `ScrollView` | 滚动容器 | 超出一屏的内容 |
| `Grid` | 表格策略 | 列宽策略 + 行窗口，几何仍由 Taffy 解算 |

```rust
use cove_ui::prelude::*;

let avatar = AnyView::new(Image::from_texture(avatar_id, Size { width: 40.0, height: 40.0 }));
let identity = AnyView::new(Column::new(
    HorizontalAlignment::Leading,
    4.0,
    AnyView::new(TupleView::new(vec![
        AnyView::new(Text::new(user.name.clone())),
        AnyView::new(Text::new(user.bio.clone()).role(TextRole::Heading(3))),
    ])),
));
let row = AnyView::new(Row::new(
    VerticalAlignment::Center,
    12.0,
    AnyView::new(TupleView::new(vec![avatar, identity])),
));
```

### 网格

`.grid(GridSpec)` 把容器变成 CSS Grid，`.grid_cell(GridCell)` 给子项放置。轨道、间距、命名区域与逐项放置与 CSS 一一对应，没有 Cove 专有词汇。

```rust
use cove_ui::prelude::*;

let template = GridAreaTemplate::new(&[
    "sidebar main",
    "sidebar main",
])
.expect("rows are rectangular and every name fills one rectangle");
let spec = GridSpec::areas(template)
    .columns([
        GridTemplateTrack::from(GridTrack::points(220.0)),
        GridTemplateTrack::from(GridTrack::fr(1.0)),
    ])
    .gap(12.0);

let sidebar = AnyView::new(Text::new("导航"))
    .grid_cell(spec.cell("sidebar").expect("template declares sidebar"));
let main = AnyView::new(Text::new("正文"))
    .grid_cell(spec.cell("main").expect("template declares main"));
let shell = AnyView::new(TupleView::new(vec![sidebar, main])).grid(spec);
```

自动放置遵循 CSS 的 sparse 流，`GridSpec::flow` 可切到 `RowDense`、`Column`、`ColumnDense`。跨轨用 `.grid_span(columns, rows)`，显式线位用 `GridCell::at`、`GridCell::spanning`、`GridCell::lines`，命名线用 `GridSpec::column_names` / `row_names` 加 `GridCell::named`，逐项对齐用 `.grid_justify_self(Align::Center)` / `.grid_align_self(Align::Center)`。

### 填满视口

布局根节点填满视口，但它的子项按内容尺寸。需要覆盖整个窗口的壳层调用 `.fill()`（`width: 100%; height: 100%`），否则 `fr` 行在内容高度为 0 时会算出 0：

```rust
let shell = AnyView::new(content).fill().grid(spec);
```

只在一个轴上覆盖时用 `.fill_width()` / `.fill_height()`。这是 CSS 百分比语义，不是 Grid 的特殊行为。

## 修饰符链

修饰符按书写顺序包裹视图，顺序影响结果：

```rust
AnyView::new(Text::new("Cove"))
    .padding(20.0)          // 内边距
    .frame_height(120.0)    // 再定外框
    .background_color(panel)
    .corner_radius(12.0)    // 裁剪圆角
```

布局修饰符在 `modifiers::layout`：`padding`、`padding_edges`、`frame`、`frame_size`、`frame_width`、`frame_height`、`fill`、`flex`、`shrink`、`grid`、`grid_cell`、`grid_span`。外观修饰符在 `modifiers::style`：`background_color`、`foreground_color`、`corner_radius`、`border_color`、`shadow`、`opacity`、`z_index`、`clipped`。

## 文本与字体

文本管线由 FreeType 与 HarfBuzz 驱动，支持复杂脚本、连字与 emoji：

```rust
use cove_ui::prelude::*;

let plain = Text::new("Hello 世界 🌊");
let styled = Text::runs(vec![
    TextRun::new("Read the "),
    TextRun::with_traits("Cove guide", TextTraits::BOLD).link("guide://start"),
])
.on_open_link(|destination| println!("{destination}"));
```

`role(TextRole::Heading(1))`、`CodeBlock` 切到语义排版；`alignment`、`line_limit`、`truncated`、`display_only` 控制换行与选择，字号与行高用 `.font(...)`、`.line_height(...)`、`.letter_spacing(...)`。

## 图像

```rust
use cove_ui::prelude::*;

// GPU 纹理：直接进入绘制与资源持有流程
let texture = Image::from_texture(texture_id, Size { width: 320.0, height: 200.0 })
    .content_mode(ImageContentMode::Fit);

// 远程图片：占位、失败与重试由 AsyncImage 负责
let remote = AsyncImage::new(Some(url), 320.0, 200.0, |phase| match phase {
    AsyncImagePhase::Success(image) => AnyView::new(image),
    AsyncImagePhase::Failure(_) => AnyView::new(Text::new("图片加载失败")),
    _ => AnyView::new(Spinner::new()),
});
```

## 列表与虚拟化

大量数据用 `List`，只构建可见行；行内状态按行键缓存。

```rust
use cove_ui::prelude::*;

let list = List::new((0..items.len()).collect(), |index: &usize, _row| {
    AnyView::new(ListRow::new(items[*index].name.clone()))
})
.row_height(36.0);
```

`VirtualWindow` 与 `LazyGrid` 暴露窗口与行高策略，用于自绘列表；`Masonry` 按列最短优先排布；`Grid` 把列宽策略与行窗口编译成 CSS Grid，由 Taffy 完成放置。
