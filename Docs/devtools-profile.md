# DevTools CPU profile

性能面板的 CPU profile 有两个来源，答案用 `source` 字段区分：

| `source` | 宿主 | 单位 |
| --- | --- | --- |
| `sampler` | 64 位 macOS、iOS/iPadOS、Windows、Linux、Android | 采样数（`samples`） |
| `instrumented` | WebAssembly（浏览器） | 实测毫秒（一单位 = 1 ms） |

## 为什么 Web 不能采样

`crates/profiling` 的采样器在目标线程上装信号/中断处理来抓栈，前提是能从外部把线程挂起。WebAssembly 运行在页面的唯一线程上，宿主只能通过事件循环与它对话，没有 `task_for_pid`、没有线程枚举、也无法在指令边界暂停：`crates/profiling` 对 wasm 目标直接报 `Unsupported`。要让 Web 也出 profile，只有两条路——伪造样本（不接受），或者让运行时自己报出它能测到的部分。

运行时本来就在 `crates/browser/src/prototype/prototype.rs::render` 里给每个帧阶段计时。协作式阶段计时把这些**已经测过的时长**累积成 profile 形状，因此每一个毫秒都是真的，只是来源不是栈采样。

## 打点位置

`render` 帧首取一次 `self.probe.profiling_phases()`：没有运行中的协作 profile 时，下面所有 `Instant::now()` 都被跳过，热路径不多花时间。

| 阶段 | 覆盖范围 |
| --- | --- |
| `recompose` | `recompose` 调用（既有的 `recompositionMs`） |
| `layout` | 布局 `layout`（既有的 `layoutMs`） |
| `encode` | 绘制列表编码、atlas 重编码、probe 叠加层 |
| `upload` | glyph atlas 像素上传 |
| `submit` | `gpu.draw` 的提交与帧槽记录 |

帧尾 `record_frame(total_ms)` 关闭这一帧：总时与已记录阶段之差成为帧的**自时**（`frame` 自身）。

## 聚合口径（`crates/devtools/src/profile/cooperative.rs`）

- 一个 `thread_local` `Recorder`：`MAX_DISTINCT_PHASES = 12` 个不同阶段、`MAX_DURATION_MS = 10_000`、`MAX_STACKS = 24`。
- 单位换算：总时四舍五入成毫秒单位，各阶段同理，**余额归 `frame` 自时**，所以「自时 + 各阶段」严格等于帧总时，不会像「一帧一样本」那样重复计数。
- `stacks`：根 `frame` 一条 + 每个阶段一条 `frame → 阶段`；`percent` 按单位占比给出。
- `samplesTaken` 是记录到的**帧数**，`retainedSamples` 是单位总量（毫秒）。
- `hz = 0`、`sampleIntervalMs = 0`：没有采样频率可报。
- 超过 12 个不同 label 计入 `samplesDropped` 并置 `stacksTruncated`。
- 时限到了就收束：`record_frame` 之外，`read` / `hotspots` / `stop` 也检查时长并 `finish` 成 `Ready`——浏览器按需渲染，静止页面可能再没有下一帧，时限不能依赖新帧到达。
- `hotspots` 以 `frame` 打头（inclusive = 帧总时），其余按 `inclusiveMs` 倒序，`MAX_HOTSPOTS = 200`。

## 客户端表现

- `profile.source === "instrumented"` 时，"Sample rate" 换成静态的 "Phase timing / measured"，启动按钮的 title 说明记录的是帧阶段。
- 统计项：`Samples` → `Frames`（`N recorded`）、`Retained` → `N ms measured`、`Observed rate` → `frames/s`；`Interval` 整项隐藏。
- 面板标题显示 `N frames · M ms measured`；调用树根行显示 `All frames`，列头是 `Self ms` / `Inclusive ms`。
- 启动 toast 不再声称以某个 Hz 采样。
- 停止提示与面板摘要保留实际帧数。Flame chart 和 Stacks 用毫秒，Methods 显示阶段耗时与占比。

## 验证

- `cargo test -p cove-devtools --lib profile::`：协作式记录器 8 项（含真实转速、上限、自时、单位求和、空跑不造栈、无帧也按到期收束）。
- `cargo check -p cove-browser --target wasm32-wasip1`（`WASI_SDK_PATH` 指向 `cargo cove setup --wasm` 装的 wasi-sdk）：wasm 专属分支（`profile/wasm.rs` 与 `render` 的 submit 打点）在真实目标上编译通过。
- 真实链路：`cargo run -p cove-devtools-cli -- serve --web-root crates/browser/dist` + 新构建的 wasm 页面（headless Chrome）。Web 运行时能力表已含 `cpu_profile`，`start_ui_profile` 返回 `source: "instrumented"`、`hz: 0`；抓帧后 `get_ui_profile` 为 `status: ready`、`samplesTaken: 1`、`retainedSamples: 2`、阶段行 `encode 2 ms`，时限过后无需新帧即收束。
- `node crates/devtools/client/tests/profile-harness.mjs instrumented`：无 Hz 选择、`Phase timing` 标签、`24 recorded`、`400 ms measured`、无 `Interval`、阶段行按毫秒显示。
- `node crates/devtools/client/tests/profile-harness.mjs windows linux instrumented-stop`：已公布能力的 Windows 和 Linux 目标可启动采样并读取热点，Web 停止后仍显示 24 帧和 400 ms。用 `CHROME_PATH` 指向本机 Chromium 浏览器。
- 新构建的 WASM Counter 在无头 Edge 上经 CLI 取点、点击并回读计数节点裁图。增加和重置期间记录 6 帧、21 ms，聚合栈毫秒之和等于实测总量。
