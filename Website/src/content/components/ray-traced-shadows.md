# Ray-Traced Shadows

在 Gallery 侧栏打开 Graphics → Ray-Traced Shadows，拖动 **Light position** 移动面光源。**Ray tracing** 切换光线查询和光栅阴影，右侧显示当前画面使用的渲染方式。设备没有可用的光线查询时，开关禁用，滑块仍可使用。

```sh
cargo run -p cove-examples --bin gallery -- --component ray-traced-shadows
```

## Rust 实现

`cove_graphics::rhi::Device::capabilities().ray_tracing.compute_ray_query` 表示当前设备和后端可以执行计算着色器中的光线查询。Windows 可用 DX12 或 Vulkan，Linux 和 Android 使用 Vulkan，macOS 和 iOS 使用 Metal。浏览器使用光栅路径。Metal 的能力报告包含可以通过 Metal API 执行查询的设备，实际硬件加速取决于 GPU。

示例在工作线程上创建离屏 RHI 设备，用遮挡物三角形构建 BLAS，再用实例构建 TLAS。移动光源时复用这些结构，每个地板采样点向面光源发出八条可见性射线。光栅路径用同一组光源位置把遮挡物投影到地面。两条路径都生成阴影遮罩，由 Rust 的场景光栅器完成着色。场景每像素做四点覆盖和深度测试，合成后作为 `ImageAsset` 显示。离屏遮罩回读和图像上传是示例的开销。

DX12 和 Vulkan 共用 `shadows.hlsl`，Metal 使用 `shadows.metal`。修改 HLSL 后，用支持 SPIR-V 的 [DirectX Shader Compiler v1.8.2505.1](https://github.com/microsoft/DirectXShaderCompiler/releases/tag/v1.8.2505.1) 从着色器目录重新生成随 Gallery 打包的两个产物。

```sh
dxc -T cs_6_5 -E shadowMain -Fo shadows.dxil shadows.hlsl
dxc -T cs_6_5 -E shadowMain -spirv "-fspv-target-env=vulkan1.2" "-fspv-extension=SPV_KHR_ray_query" -Fo shadows.spv shadows.hlsl
```
