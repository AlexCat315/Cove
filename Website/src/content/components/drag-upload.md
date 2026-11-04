# DragUpload / FileDropZone

`DragUpload` 接收从应用外部拖入的文件，并把它们上传。它把三件通常需要手写的事组合在一起：拖放区 `FileDropZone`、上传状态机 `UploadState` 和进度显示。

```rust
DragUpload::new(UploadRequest::new("https://example.test/upload", Vec::new()))
    .accepted_extensions(["png".to_owned(), "jpg".to_owned()])
    .maximum_files(4)
    .reader(|file| Ok(UploadFilePart::new("file", file.name.clone(), "image/png", std::fs::read(&file.path)?)))
```

## 两条入口

同一个回调有两条到达路径，这样触屏和键盘用户不必依赖拖拽：

- **平台拖入**：系统拖入事件经 `InputEvent::ExternalDrop` 进入，由 `FileDropRegistry` 按节点命中分发到拖放区。
- **浏览按钮**：`FileDropPrompt` 内的 “Browse…” 经 `FilePicker` 打开系统面板。

`FilePicker` 未安装时浏览按钮禁用，而不是打开一个空面板。

## 接受规则

`FileDropZone` 按三层判断是否接受：

1. `accepted_extensions`：扩展名（不含点，大小写不敏感）。空表示接受所有类型。
2. `maximum_files`：一次拖入的文件数量上限。
3. `accepts`：调用方补充的谓词，例如控件忙或载荷超限。

被拒绝的拖入不是错误：区域显示拒绝色，`on_drop` 不触发，调用方状态不变。

## 上传状态

`UploadState` 是 `Idle`、`Preparing`、`Uploading { sent, total }`、`Succeeded`、`Failed` 五态。`progress()` 只在可确定进度时返回分数，`status_text()` 给出可显示文案。

上传在 worker 线程执行，结果经 `cove_core::work::spawn_then` 回到 UI 线程写 `State`，因此 UI 线程不阻塞在网络或文件读取上。

## 传输接缝

`UploadTransport` 由宿主提供：

```rust
pub trait UploadTransport: Send + Sync {
    fn send(&self, request: &UploadRequest, report: &(dyn Fn(u64) + Send + Sync))
        -> Result<UploadResponse, UploadError>;
}
```

桌面宿主安装 `HttpUploadTransport`（基于 `cove-http`）。Wasm 没有 HTTP 后端，holder 为空，控件保持可用并报告 `HostUnavailable`，而不是静默失效。

## 大小边界

`cove-http` 的 body 是 `Vec<u8>`，所以上传有明确上限：`UploadRequest::maximum_bytes`，默认 64 MiB。超限在编码前就返回 `PayloadTooLarge`，不会先分配再失败。流式 body 不在本次范围内。

## 平台覆盖

| 平台 | 拖入来源 | 浏览兜底 |
| --- | --- | --- |
| macOS / Windows / Linux | SDL3 文件拖放 | 系统打开面板 |
| Web（Emscripten） | HTML5 拖放 | 系统打开面板 |
| iOS | “用…打开”交付的文档 | 系统打开面板 |
| Android | SDL 后端不上报拖放 | 系统打开面板 |

Android 与 iOS 上拖入不总是可用，浏览按钮是这些平台的正式入口，不是降级说明。

## 结构验证与原生验收

`crates/compose/tests/file_drop_tests.rs` 覆盖命中、类型拒绝、数量上限、进入/离开高亮、禁用、超限、失败与缺失 transport；`crates/shell/tests/external_drop_tests.rs` 覆盖五个阶段的解码。这些是结构测试：从其他应用真实拖入 OS 文件的操作尚未在本机原生验收。
