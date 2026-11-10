# NativeRHI

NativeRHI 是 Cove 的原生渲染硬件接口（Rendering Hardware Interface）。它在 Metal、DirectX 12 与 Vulkan 之上提供统一抽象，让同一份 Swift 代码在所有平台获得 GPU 加速。

## 为什么需要 RHI

三大图形 API 在设备创建、资源管理、命令录制与同步模型上差异巨大。直接面向它们编写 UI 会导致大量平台分支。NativeRHI 把这些差异收敛为一套面向 UI 合成场景的精简接口：

- 设备与交换链（swap chain）管理
- 纹理、缓冲与采样器
- 渲染通道与命令录制
- 同步（信号量 / fence）

## 后端矩阵

| 平台 | 后端 | 状态 |
|------|------|------|
| macOS / iOS | Metal | 系统内置 |
| Windows | DirectX 12 | 默认。初始化失败时尝试 Vulkan。DXIL shader 可由 DXC 或 Slang 编译 |
| Linux / Windows | Vulkan | SDK 固定头文件，运行时加载系统驱动 |

DX12 的 `draw_indirect` 和 `dispatch_indirect` 使用带 `BufferUsage::INDIRECT` 的缓冲区，偏移按 4 字节对齐。绘制参数块依次为 4 个 `u32`（顶点数、实例数、首顶点、首实例），调度参数块为 3 个 `u32`（X、Y、Z 组数）。

`device.capabilities().mesh_shading.mesh` 表示可创建网格管线并调用 `draw_mesh_tasks`。可选 Task 着色器由 `mesh_shading.task` 标识。DXR 通过 `make_acceleration_structure` 和 `record_build` 构建三角形 BLAS 与实例 TLAS。使用 `compute_ray_query` 检查是否可以在计算着色器中查询绑定的 TLAS。
| Android | Vulkan | 随 APK 构建 |

## 统一渲染管线

```
视图树 → 布局计算 → 绘制指令 → NativeRHI 命令缓冲
                                 ├─ Metal (MTLCommandBuffer)
                                 ├─ DX12  (ID3D12CommandList)
                                 └─ Vulkan (VkCommandBuffer)
```

## 离屏渲染与快照

`OffscreenView` 把任意视图渲染到离屏纹理并返回快照，适合：

- 图表、缩略图生成
- 跨进程 / 跨表面共享内容
- 导出与预览

```swift
let snapshot = OffscreenView {
  ComplexChart(data: data)
}
```

## GPU 与图像管线互通

GPU 生产的图像通过 `TextureImage` 直接进入现有的图像绘制与资源持有流程，无需回读 CPU；解码后的位图也会被立即上传为 GPU 纹理，保持全链路 GPU 驻留。

## 着色器

各后端的着色器通过统一的中间表示与平台编译器处理：

- **Metal**：Metal Shading Language，系统编译
- **DX12 / Vulkan**：通过 DXC 编译，跨后端共享 HLSL 来源

## 验证

渲染生命周期与内存管理、MSAA 等评估记录位于仓库 `Docs/` 目录下，包含 `rendering-lifecycle-and-memory.md` 与 `msaa-evaluation.md`，用于追踪各后端的一致性与资源回收质量。
