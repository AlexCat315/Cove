# 原生媒体的许可证与分发

日期：2026-11-05。适用范围：`cove-media` / `cove-media-rhi` 的固定 GStreamer 源码 profile。Cove 自有代码沿用 Apache-2.0；第三方声明见源码根目录及许可证包内的 `THIRD_PARTY_NOTICES.md`。

## 谁需要这些工具链

普通用户安装已打包的应用，所需媒体库、插件和许可证随应用交付，不需要安装 Rust、Meson、Ninja、编译器或 GStreamer 开发 SDK。源码和重新链接材料提供给希望修改库的接收者，不要求每位用户执行构建。

当前应用开发者从源码使用 Cove 媒体时，需要 Rust、C/C++ 编译器、Meson、Ninja、目标平台 SDK，以及上游构建要求的代码生成工具。当前 macOS 源码版本还需要较新的 Bison，系统自带版本不能满足要求。这些是构建机依赖，不能变成应用启动时的安装要求。

开发者沿用当前源码构建入口，按目标平台保留独立的原生输出目录，后续由 Meson/Ninja 增量构建，Cargo 复用已编译结果。构建器可在原生构建完成后直接运行 Cargo，例如在 Cove 源码工作区执行：

```sh
cargo run --locked -p cove-native-build --example gstreamer -- \
  --out-dir target/media-source-build --jobs 6 -- \
  build --locked -p cove-media --features gst
```

许可证收集接在这次构建中完成。当前不增加独立的原生 SDK 发布、下载和版本管理流程。CI 可运行同一源码构建入口并复用匹配工具链和目标的缓存；缓存用于提速，首次构建仍需必要工具。工具检查/初始化和下游应用接入体验的进一步简化属于后续构建工作，本次没有自动安装工具链。

预构建 SDK 只作为未来有明确需求时的可选优化，届时需要维护目标架构、最低系统版本、C/C++ ABI、MSVC runtime、功能 profile、源码 revision 和包校验。现有 pkg-config 文件含构建机绝对路径，不能直接把本机 `install/` 压缩后宣称为可用的通用 SDK。采用预构建分发时仍须保留可复现源码构建和 LGPL 修改/重新链接入口。

## 生成和随包保留的材料

正常源码构建会在安装前缀中生成 `share/licenses/cove-media/`。已有构建可以单独刷新声明，不会重新编译或启动应用：

```sh
cargo run --locked -p cove-native-build --example gstreamer -- \
  --out-dir target/media-source-build --notices-only
```

交叉构建须同时传入与该输出目录一致的 `--target <triple>`。每个平台使用独立输出目录。生成机制覆盖 macOS、iOS、Windows、Linux、Android；目前只有 macOS 的实际媒体构建包已验证，其他平台的设备/分发验收仍待完成。

目录内容：

| 文件 | 用途 |
| --- | --- |
| `THIRD_PARTY_NOTICES.md`、`native/`、`rust/` | 逐组件声明及上游版权、完整许可、免责和附加条款 |
| `Cove-LICENSE` | Cove 自有代码的 Apache-2.0 全文 |
| `inventory.json`、`Cargo.lock` | 目标、原生版本、Rust 版本/来源/声明/可用校验值；保守包含 normal 和 build 闭包，不遍历 dev-only 边 |
| `build-manifest.txt`、`meson-*.json` | 实际构建参数、编译器、Meson 启用项目、目标及依赖探测记录 |
| `source-records/`、`gstreamer-changes.patch` | wraps 的来源/哈希、补丁及 GStreamer 主仓库已跟踪文件的本地差异 |
| `DISTRIBUTION.md` | 本分发说明 |

把整个许可证目录安装到应用资源目录或发行包中，确保接收者能够阅读和复制。应用可从“关于/许可证”入口打开这些本地文件；不必为每个播放器重复显示声明。没有运行时 UI 的库、CLI 和 SDK 同样保留这些文件。

生成器遇到未登记的原生子项目、缺少许可证全文或过期的补充记录会报错，不会静默跳过。构建 profile 显式设置 `gpl=disabled`、`ugly=disabled`、`libav=disabled`；这些选项有助于控制选型，但不能代替最终产品的许可核对。GLib 等源码包的补充文本也包含未编入产品的测试/工具许可；不可据此宣称整个可执行文件采用相同许可。

## 静态链接 LGPL 的源码和重新链接材料

GStreamer 的许可允许应用选择自己的许可证。[GStreamer 官方许可说明](https://gstreamer.freedesktop.org/documentation/frequently-asked-questions/licensing.html)解释了插件和外部库需要逐项判断的原因。当前静态构建包含 LGPL 库，分发二进制时应按相应版本的 LGPL，提供适用的对应源码和允许接收者修改库、重新链接应用的材料；[GNU FAQ](https://www.gnu.org/licenses/gpl-faq.en.html#LGPLStaticVsDynamic)也说明了静态链接时的应用目标文件要求。完整条款以随包 LGPL 2.0 / 2.1 原文为准。

本项目采用“对应源码与构建材料随版本一起提供”的分发方式，不依赖一条未经落实的源码请求承诺。准备发行包时同时提供：

1. **库的完整对应源码**：GStreamer 固定 revision、全部实际使用的 wrap 源码、wrap 补丁、嵌套子项目及本地修改。保留原始版权、许可证和修改记录。只给 GitHub 首页、版本号、wrap 下载地址或静态 `.a` 文件不足以替代对应源码。
2. **构建材料**：Cove 的原生构建代码、实际 Meson 选项、交叉文件、工具/SDK 版本、目标和链接步骤。`build-manifest.txt` 不包含交叉文件的完整内容，分发时须另附实际 `cross.ini` / 用户提供的 cross file 及工具链配置。
3. **应用重新链接材料**：可以提供足以重建/重新链接的应用源码及依赖、配置、脚本；也可以按 LGPL 条款提供适合与修改后的库重新链接的应用目标文件、必要的数据/工具及操作说明。闭源应用不因此自动变成 LGPL，但仍须落实可用的重新链接方案。Rust LTO 产物、单独的最终可执行文件或缺少依赖的 `.rlib` 不应未经验证就当作完整方案。
4. **允许修改和调试的条款**：应用分发条款须允许为用户自身使用修改作品，并允许为调试库的修改而进行逆向工程，不得被应用 EULA 的普遍禁止条款抵消。保留 LGPL 所要求的使用提示、版权、许可证和免责说明。

采用应用源码方案时，可在保存好本次完整源码树后运行 `cargo vendor --locked --versioned-dirs <vendor-dir>`，连同 Cargo 输出的 source 配置和锁文件一起提供。原生源码归档须包含已经展开的 wraps 和本地补丁；`git archive` 只导出受跟踪文件，会漏掉子模块和 Meson 下载的源码。不要归档构建机的凭据、`.git`、`target/` 或无关缓存。发行材料中记录归档校验值，并验证从这些材料构建时不再下载未提供的库源码。

每个实际发布的应用在隔离目录中演练一次：从随版本提供的材料构建，修改一个 LGPL 库的可观察行为，重新链接应用并运行，确认修改生效。再确认静态插件注册和普通媒体播放有效。签名、安装和商店条款也须与接收者按许可行使权利的方式一致；本次许可证生成不代表已完成商店分发验收。

## 修改 LGPL 库的构建入口

固定 revision 的构建入口用于复现官方 profile，会拒绝主仓库中未记录的修改。接收者仍可直接使用随包 Meson 构建目录/配方重建修改后的库：

```sh
meson compile -C <native-out>/build
meson install -C <native-out>/build --no-rebuild --only-changed
```

随后用新的 Cargo 输出目录构建应用，令官方 `*-sys` crate 使用该前缀的静态 pkg-config 文件；`PKG_CONFIG_LIBDIR` 需同时包含 `<prefix>/lib/pkgconfig` 和 `<prefix>/lib/gstreamer-1.0/pkgconfig`，`PKG_CONFIG_PATH` 置空，`PKG_CONFIG_ALL_STATIC=1`，交叉构建另设 `PKG_CONFIG_ALLOW_CROSS=1`。使用新的 `CARGO_TARGET_DIR` 避免缓存旧链接结果。应用发行包应给出其目标对应的完整命令，而不是要求接收者调用拒绝修改的固定 revision 检查。若从源码重新配置 Meson，选项及 cross file 以随版本提供的构建记录为准。

修改后重新生成声明，并另行记录修改者、日期和内容。`gstreamer-changes.patch` 只记录主仓库已跟踪文件的差异；ignored wrap 目录、嵌套仓库和新增文件必须体现在实际提供的对应源码中，不能用该 patch 代替完整源码。

## 目标平台和范围

Windows 启用 DirectX-Headers / DirectXMath fallback 时，生成器从实际源码收集 MIT 原文。Android JNI 和平台插件的许可随 GStreamer 模块保留。Linux profile 使用系统 ALSA；如果最终发行包再分发或静态链接 libasound，分发方还须收集该具体版本的许可和对应源码，不能将其当作已由当前 GStreamer wraps 包含。其余系统库、C/C++ 运行库和 SDK 亦按实际链接/再分发内容处理。

`meson-dependencies.json` 含配置过程中探测到的依赖，其中有些只用于工具，不能直接等同于最终二进制依赖。最终产品应结合链接命令和平台依赖检查工具核对；例如 macOS 的 `otool -L`、Linux 的 `readelf -d` 和 Windows 的 `dumpbin /dependents`，并检查静态链接记录。

本次清单覆盖媒体 Meson 子项目及媒体 Rust 依赖闭包；`cove-media-rhi` 引入的 Cove 图形/文本/图像原生库、字体、其他可选功能和最终应用依赖仍须合并到产品级清单。新增插件、网络协议、gst-libav、软件编解码器、资源或平台 fallback 时，更新实际清单和源码材料。许可证声明本身不授予编解码器专利、商标或平台 SDK 的额外权利。
