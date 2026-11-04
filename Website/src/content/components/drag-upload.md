# DragUpload / FileDropZone

`DragUpload` 接收从应用外部拖入的文件，并把它们上传。它把三件通常需要手写的事组合在一起：拖放区 `FileDropZone`、上传状态机 `UploadState` 和进度显示。

```rust
DragUpload::new(UploadRequest::new("https://example.test/upload", Vec::new()))
    .accepted_extensions(["png".to_owned(), "jpg".to_owned()])
    .maximum_files(4)
    .reader(|file| Ok(UploadFilePart::new("file", file.name.clone(), "image/png", std::fs::read(&file.path)?)))
```

## 默认外观

拖放区自带静态外观，不依赖拖拽状态才出现：

- 下沉表面 + 主题圆角 + 虚线描边，安静状态即可读作投放目标。
- 图标芯片、标题、类型/数量说明与 “Browse files” 次级按钮按层级排列，文案不重复。
- 拖拽悬停或类型被接受时描边转为强调色；拒绝时转为错误色；禁用时使用分隔线色与禁用文字色。
- 上传进度只在有上传任务时出现：进行中显示进度条、百分比与取消按钮；结束显示成功或失败状态条，不再常驻空进度条。

## 两条入口

同一个回调有两条到达路径，这样触屏和键盘用户不必依赖拖拽：

- **平台拖入**：系统拖入事件经 `InputEvent::ExternalDrop` 进入，由 `FileDropRegistry` 按节点命中分发到拖放区。
- **浏览按钮**：`FileDropPrompt` 内的 “Browse files” 经 `FilePicker` 打开系统面板。

`FilePicker` 未安装时浏览按钮禁用，而不是打开一个空面板。

## 类型过滤

`accepted_extensions` 是**调用方自选的清单**，不是框架的类型白名单。空集合表示接受任意文件：

```rust
// 任意类型，包括压缩包和未知格式
DragUpload::new(request)

// 只要图片
DragUpload::new(request).accepted_extensions(["png".to_owned(), "jpg".to_owned()])
```

匹配规则：

- 大小写不敏感，清单里不含点。
- **没有扩展名的文件不参与白名单匹配**：`README`、`.gitignore` 在空清单下通过，在非空清单下明确拒绝。开头的点表示隐藏文件，不是扩展名。
- 压缩包（zip、tar、gz、7z、rar 等）与普通文件同等对待，只是 Content-Type 不同。解压不由拖放区决定。

## 接受规则

`FileDropZone` 按四层判断是否接受：

1. `accepted_extensions`：如上一节。
2. `maximum_files`：**整批**文件的数量上限。平台按文件逐条上报，注册表累积到 `Complete` 再一次性投递，所以上限看到的是整次拖入而不是单个文件。
3. `allows_directories`：默认 `false`。文件夹没有单一载荷，按默认读作文件只会得到空内容。
4. `accepts`：调用方补充的谓词，例如控件忙或载荷超限。

被拒绝的拖入不是错误：区域显示拒绝色，`on_drop` 不触发，调用方状态不变。

## 目录

拖入文件夹默认被拒绝，区域显示拒绝色而不是静默失败。确实要接收目录的调用方显式打开：

```rust
DragUpload::new(request)
    .allows_directories(true)
    .reader(|file| {
        // 展开目录内容由调用方负责；框架不递归读取。
        Ok(UploadFilePart::new("file", file.name.clone(), "application/octet-stream", vec![]))
    })
```

目录判定来自 `std::fs::metadata`（`DroppedFile::inspected`）。无法读取的路径按文件处理，由 reader 报告真实失败，而不是在拖入阶段丢弃。

## Content-Type

`content_type_for(name)` 按扩展名查表，覆盖常见图像、文档、音视频、字体与压缩包；未知扩展名与无扩展名回落 `application/octet-stream`。这是查表而非嗅探：拖入阶段不读取字节。

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
