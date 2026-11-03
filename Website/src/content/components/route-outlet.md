# RouteOutlet

`RouteOutlet::new(selection, destination)` 只构造选中的目的地，以路由作为视图标识。路由变化独立更新 outlet，返回已离开的页面时重新建立页面局部状态。

应用壳放在 outlet 外部；状态通过绑定传入，精确读取追踪下外壳不需要为页面切换重建。

共享 Rust 实现覆盖所有宿主平台。自动化场景验证与原生视觉、键盘及 IME 验收分别记录在 Gallery 验收表中。
