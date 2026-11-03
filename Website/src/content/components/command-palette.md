# CommandPalette

`CommandPalette` 根据查询筛选命令，禁用的命令不进入可执行结果。键盘移动、最近记录和激活使用同一结果集合。

`PaletteCommand::enabled(false)` 禁止该命令显示和执行；菜单中保留禁用项的语义由 `Menu` 单独提供。

共享 Rust 实现覆盖所有宿主平台。自动化场景验证与原生视觉、键盘及 IME 验收分别记录在 Gallery 验收表中。
