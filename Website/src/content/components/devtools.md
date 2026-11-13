# DevTools

启动控制器，再从 Rust 工程启动带 DevTools 的调试版。

```sh
cargo run -p cove-devtools-cli -- serve
COVE_DEVTOOLS=1 cargo run -p cove-examples --bin gallery
```

在 Inspector 选中应用，点击 **Capture** 查看 Cove 渲染画面。**Pick** 查找像素下的组件，**Interact** 在应用内点击、拖动或滚动。键盘和文本输入发往选中运行时内的控件。

```sh
cargo run --quiet -p cove-devtools-cli -- call get_ui_session '{}'
cargo run --quiet -p cove-devtools-cli -- call capture_ui '{"target":"RUNTIME_ID"}' --image frame.png
cargo run --quiet -p cove-devtools-cli -- call click_ui '{"target":"RUNTIME_ID","capture_id":"CAPTURE_ID","x":120,"y":84}'
```

`x` 和 `y` 是图片左上角起算的像素坐标。窗口尺寸改变后重新获取画面。用 `wait_for_ui` 检查操作后的控件状态。输入回执表示事件已交付，原生键盘和 IME 需要独立验收。

画面与输入范围为 Cove 场景。系统菜单、原生文件面板和输入法候选窗由操作系统管理。
