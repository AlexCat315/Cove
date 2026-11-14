# Drag & Drop

`Draggable<T>` / `DropTarget<T>` 在 `DragScope` 内共享一个类型化的拖拽 payload。payload 类型由调用方决定，不同目标可以声明不同的 payload 类型；作用域内的会话负责指针捕获、预览与命中。

```rust
let scope = DragScope::new(Column::new(
    HorizontalAlignment::Leading,
    16.0,
    AnyView::new(TupleView::new(vec![
        AnyView::new(Draggable::new(
            "Design",                       // payload: &'static str
            AnyView::new(Text::new("Design")), // 拖拽预览
            chip("Design"),                 // 常驻内容
        )),
        AnyView::new(
            DropTarget::new(
                |payload: &&'static str| *payload == "Design",
                |payload| println!("dropped {payload}"),
                lane("Design lane"),
            )
            .on_targeted(|is_targeted| { /* 高亮切换 */ }),
        ),
    ])),
));
```

## 会话与重组

`DragScope` 的会话按视图身份通过 `Scope::remember` 保留，调用方可以在 `body` 里重建 `DragScope`，拖拽不会因为一次重组而丢失 payload。需要在多个作用域间共享会话时，用 `DragScope::with_session(session, content)`。

## 指针与手势

- 指针按下时先以“可转移手势”捕获，这样第一次移动已经离开源节点时仍能收到阈值事件。
- 越过 6pt 阈值后升级为独占捕获并发布预览；此后滚动容器不能再抢走手势。
- 未越过阈值就释放时只释放捕获，不产生 drop。
- 文字/预览由 portal 渲染，不受裁剪影响；释放到所有目标之外只取消，不触发 drop。
- `on_targeted` 在目标进入/离开时回调，供调用方切换高亮。

## 预览跟随与淡入淡出

预览不是逐帧贴死指针，而是由 `DragPreviewFollower` 用弹簧追到指针位置：移动只走 `PortalStore::update_position`，不触发重组，因此快速拖动时预览保持轻微滞后与惯性，看起来像被拎着的实体。出现时淡入、取消或落下时原地淡出；淡出未完成就开始下一次拖拽会直接接管同一个入口，不会排队。开启减弱动效时预览钉在指针上并跳过淡变。

## 阈值统一

“按下还是拖拽”的判定集中在 `primitives::interaction::drag_threshold`：`DRAG_THRESHOLD`（6pt）与 `crossed` / `crossed_along_axis`。通用拖放、`Tree` 行拖拽、`SwipeList` 滑动、`Table` 列拖拽与 `WheelPicker` 都读同一组值，调灵敏度只需要改一处。

## 与 Tree 拖拽的区别

`Tree` 的行拖拽是集合内部的排序/归属操作，使用 `TreeDragOperations`（`can_drag` / `can_drop` / `on_drop` 与生命周期事件）。它与通用 `Draggable` 共用指针捕获规则：按下可让外层滚动容器滚动，越过阈值后独占。行拖拽的落点是 `Before` / `Inside` / `After` 三个带区。

## 资产拖放目标

`AssetDropTarget` 把自身登记到进程级 `AssetDropRegistry`，并按 `accepted_kinds` 过滤载荷。空集合接受任意类型。落点命中多个重叠目标时，**最后登记（最上层绘制）的目标生效**，与 Swift 的命中规则一致；同帧更新只替换原登记，不会重复入列。

`can_drop` 用于只判断某个具体目标是否接受当前载荷（例如字段高亮），不触发投递。

## Gallery

Gallery 的 Drag & Drop 页演示两个不同 payload 的标签拖入对应 lane：拖动期间目标 lane 使用选中态高亮，预览由 portal 渲染，释放后只在匹配的 lane 写入结果。Tree 的拖拽生命周期由 `list_tree_tests` 与 `tree_features_tests` 覆盖。

共享 Rust 实现覆盖所有宿主平台；结构、场景与 Gallery 挂载验证记录在 `Docs/gallery-audit.md`。
