# Notification / Toast

`ToastController` 持有一个有界队列，`NotificationHost` 在窗口边缘渲染卡片。超出 `max_visible` 的通知排队等待，前面的卡片离开后按序补位。

```rust
let controller = ToastController::new(3);
controller.post(
    ToastNotification::new("Saved")
        .id("save")
        .message("The workspace was written to disk.")
        .duration(Some(6.0)),
);
NotificationHost::new(controller).width(320.0);
```

## 生命周期

- `duration(None)` 保持可见，直到 `dismiss` 或 `clear`；默认时长 5s。
- 同一 `id` 再次发布替换原卡片而不是叠加，适合“保存中 → 已保存”的稳定状态。
- 卡片进入与离开各有弹簧动效；减弱动效时直接呈现。
- 最多渲染两个 `ToastAction`，多余动作视为装饰。

## 悬停暂停

指针停在卡片上时，该通知的倒计时暂停（拖动卡片留白也算），移开后从剩余时间继续。暂停挂在整张卡片上，不只是按钮，所以阅读消息或准备点击时不会在指针下消失。

## 安全区与位置

`ToastPosition` 有四个角，`Automatic` 固定解析为右上。卡片栈按窗口安全区留出顶部或底部的额外内边距，刘海、圆角与触摸条不会盖住文字；折叠屏的一段窗口作为独立边界参与定位。

## 时钟

只有存在“可见且未暂停、且带剩余时长”的通知时 `needs_clock()` 才为真，宿主据此决定是否推进时钟。通知全部暂停或永久可见时不产生无意义的逐帧唤醒。

共享 Rust 实现覆盖所有宿主平台。自动化场景验证与原生视觉、键盘及 IME 验收分别记录在 Gallery 验收表中。
