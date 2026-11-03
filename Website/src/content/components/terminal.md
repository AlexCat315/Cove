# Terminal

`TerminalSession::select` 和 `clear_selection` 在选区变化后立即使缓存帧失效并通知视图；拖选与复制读取同一个引擎选区。`frame()` 返回当前选中单元格状态，不必等待下一批程序输出。

选区与缓存逻辑为所有宿主共享的 Rust 实现；原生平台键盘和剪贴板验收另行记录。
