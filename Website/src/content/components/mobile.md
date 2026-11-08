# 移动端支持

组件层是共享 Rust 实现：桌面、iOS、Android 使用同一套节点树、布局、排版与输入分发。差异集中在宿主层的窗口、软键盘、IME、安全区与文件目录。逐页验收状态见 `Docs/gallery-audit.md`。

## 宿主与构建

| 平台 | 入口 | 状态 |
| --- | --- | --- |
| Android | `MainActivity` → JNI → `cove_android_app`，Gradle 产出 APK | 生命周期、Choreographer、Vulkan 交换链、触摸、IME、安全区链路完整；未做设备验收 |
| iOS | `cove_ui::application!` → `SDL_RunApp` + `CADisplayLink`，Rust 二进制直接组成 `.app` | 模拟器构建并签名完成，尚未启动；无独立 iOS 宿主目录 |

```sh
cargo xtask package android --release --package cove-examples
cargo xtask run ios --simulator --package cove-examples
```

`COVE_PREVIEW_DEVICE=phone` 可在桌面预览手机表面（390pt 宽度、触摸输入密度）。

## 输入与键盘

- `Shell::show_soft_keyboard` / `hide_soft_keyboard` / `keyboard_height()` 是共享 API。iOS 由 `UIKeyboardWillShow/WillHide` 通知缓存键盘高度并唤醒事件循环，`Environment.keyboard` 每帧刷新；Android 由 `WindowInsets.Type.ime()` 提供高度，`Component` 侧的键盘避让与桌面一致。
- IME 组合预览：Android 有完整 `InputConnection`（组合文本、选区、删除周围文本）；iOS 依赖 SDL3 UIKit，仓库内未找到 `SDL_SendEditingText` 发送路径，原生中文/日文输入法预编辑**未做设备验收**。
- 触摸：主指针 down/move/up、长按选词、上下文菜单、惯性滚动有实现；副指针、捏合、旋转未接入，圆形选择手柄与放大镜尚未接线。
- 剪贴板：Android 目前只有 IME 上下文菜单的 copy/cut/paste；通用 `ClipboardHolder` 后端未接 JNI。iOS/Android 走 SDL3 剪贴板路径，未设备验收。

## 文件与持久化

`FileAppStorageStore::default_path()` 的平台策略：

| 平台 | 目录 |
| --- | --- |
| iOS | 沙盒 `HOME/Library/Application Support` |
| Android | JNI `createApplication` 注入的 `COVE_APP_STORAGE_DIR`（应用私有 `filesDir`），否则 `PACKAGE_RESOURCE_BUNDLE_PATH` 的父目录 |
| Windows | `APPDATA` |
| macOS | `~/Library/Application Support` |
| Linux | `XDG_CONFIG_HOME`，否则 `~/.config` |

该存储只适合键值偏好。写作软件的长文档需自行实现文件 IO；Android 目前没有 SAF、共享存储或运行时权限，https 还需要 `COVE_OPENSSL_ANDROID_DIR` 提供 `libssl.so` / `libcrypto.so`。

## 平台能力边界

- `cove-system` 的进程服务在 iOS、Android、tvOS、watchOS、visionOS 上由 `command::process_execution_available()` 守卫，所有启动入口返回 `ProcessError::Unavailable`（聚合层为 `io::ErrorKind::Unsupported`），不会落到 fork/exec。
- iOS `Info.plist` 声明 iPhone 与 iPad 的四种方向；Android 方向由 `MainActivity.setRequestedOrientation` 控制。
- iOS / Android / Wasm 不提供屏幕取色器；原生 WebView 宿主缺失；移动画中画是窗口内浮动回退。

## 验收口径

结构测试（场景事件注入、纯 Rust 单测）不等于原生视觉、键盘或 IME 设备验收；DevTools 文本注入不能验证原生键盘/IME 激活。原生 Windows、Linux、iOS、Android 尚未设备实测。

**仍未完成**：iOS 独立宿主与 `UITextInput`、原生 IME 组合预览、Android 通用剪贴板 JNI、多指手势、SAF/文件权限、移动前后台生命周期通知、原生 WebView。以上为已知缺口，不是平台空实现。

共享 Rust 实现覆盖所有宿主平台。自动化场景验证与原生视觉、键盘及 IME 验收分别记录在 Gallery 验收表中。
