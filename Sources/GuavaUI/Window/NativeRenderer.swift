import GuavaUIRuntime
import NativeRHI
import PlatformShell

@MainActor
final class NativeAppWindowRenderer: AppWindowRenderer {
    private let device: Device
    private let renderer: NativeDrawListRenderer
    private let settings: AppWindowRenderSettings
    private var native: NativeRenderSurface?
    private var swapchain: SwapchainResource?
    private var size = SIMD2<UInt32>(repeating: 0)
    private var multisample: TextureResource?
    /// Owned copy of the presented frame, so a panel can blur it.
    private var framebuffer: TextureResource?
    private var presentationActive = true
    private var vsync = true
    private var state = AppWindowRendererState.unconfigured

    var isConfigured: Bool { native != nil }

    var memorySnapshot: AppWindowMemorySnapshot {
        let bytes = [multisample, framebuffer].compactMap { $0 }.reduce(0) {
            $0 + $1.descriptor.width * $1.descriptor.height * 4 * $1.descriptor.sampleCount
        }
        return AppWindowMemorySnapshot(state: state, samples: settings.samples,
            hasSwapchain: swapchain != nil, estimatedTargetBytes: bytes + renderer.estimatedPresentationTextureBytes)
    }

    init(device: Device, renderer: NativeDrawListRenderer, settings: AppWindowRenderSettings) {
        self.device = device; self.renderer = renderer; self.settings = settings
    }

    func configure(native: NativeRenderSurface, size: SIMD2<UInt32>, vsync: Bool) throws {
        try device.withFrameSession {
            guard self.native == nil else { throw RHIError.invalidArgument("window is already configured") }
            let resource = try SwapchainResource(device: device,
                descriptor: SurfaceFactory.nativeDescriptor(native: native, size: size, vsync: vsync))
            self.native = native; self.swapchain = resource; self.size = size
            self.vsync = vsync
            presentationActive = true
            state = .active
        }
    }

    func resize(size: SIMD2<UInt32>, vsync: Bool) throws {
        try device.withFrameSession {
            self.size = size
            self.vsync = vsync
            guard let native, let swapchain else { return }
            // Native swapchains require a positive extent. The host suspends
            // zero-sized windows; keep their desired size for the next activation.
            guard size.x > 0 && size.y > 0 else { return }
            try device.configureSwapchain(swapchain.swapchain,
                descriptor: SurfaceFactory.nativeDescriptor(native: native, size: size, vsync: vsync))
        }
    }

    func draw(list: DrawList, logical: SIMD2<Float>) throws -> Bool {
        try device.withFrameSession {
            guard presentationActive, let native, size.x > 0 && size.y > 0 else { return false }
            if swapchain == nil {
                swapchain = try SwapchainResource(device: device,
                    descriptor: SurfaceFactory.nativeDescriptor(native: native, size: size, vsync: vsync))
            }
            guard let swapchain else { return false }
            try renderer.configure(format: .bgra8UnormSRGB, sampleCount: Int(settings.samples))
            try device.beginFrame()
            defer { device.endFrame() }
            let image = try device.acquireSwapchainImage(swapchain.swapchain)
            let pixels = SIMD2(image.width, image.height)
            if settings.samples > 1 && (multisample?.descriptor.width != pixels.x || multisample?.descriptor.height != pixels.y) {
                var descriptor = TextureDescriptor(width: pixels.x, height: pixels.y,
                    format: .bgra8UnormSRGB, usage: [.colorTarget])
                descriptor.sampleCount = Int(settings.samples)
                multisample = try TextureResource(device: device, descriptor: descriptor)
            }
            let clear = settings.clearColor
            // A frosted panel or a reveal samples the frame already drawn, so
            // those frames live in an owned texture and reach the drawable in a
            // final blit. Ordinary frames draw straight into the drawable: the
            // blit is a full-screen pass and the owned texture is a full-size
            // allocation, neither of which pays for itself when no pass needs
            // to sample the result.
            let samplesFrame = renderer.supportsBackdrop
                && (list.reveal != nil || list.batches.contains { $0.backdrop != nil })
            if !samplesFrame { framebuffer = nil }
            else if framebuffer?.descriptor.width != pixels.x || framebuffer?.descriptor.height != pixels.y {
                framebuffer = try TextureResource(device: device, descriptor: TextureDescriptor(
                    width: pixels.x, height: pixels.y, format: .bgra8UnormSRGB,
                    usage: [.colorTarget, .sampled], label: "ui-framebuffer"))
            }
            let surface = samplesFrame ? framebuffer?.texture ?? image.texture : image.texture
            var target = RenderColorTarget(texture: multisample?.texture ?? surface,
                loadAction: .clear(clear))
            target.resolveTexture = multisample == nil ? nil : surface
            let commands = CommandBuffer()
            let frame = try renderer.record(list: list, into: commands, target: target,
                viewport: NativeUIViewport(pixels: pixels, logical: logical),
                backdropSource: samplesFrame ? framebuffer?.texture : nil)
            if samplesFrame, let source = framebuffer?.texture {
                let present = RenderColorTarget(texture: image.texture, loadAction: .dontCare, store: true)
                try renderer.recordBlit(source: source, into: commands, target: present, size: pixels)
            }
            try device.submit(commands)
            frame.didSubmit()
            try device.present(image)
            return true
        }
    }

    func setPresentationActive(_ active: Bool) throws {
        try device.withFrameSession {
            guard presentationActive != active else { return }
            presentationActive = active
            if native != nil { state = active ? .active : .suspended }
            if !active {
                // close waits for in-flight work before dropping attachment ownership.
                defer { swapchain = nil; multisample = nil; framebuffer = nil; renderer.trimPresentationResources() }
                try swapchain?.close()
            }
        }
    }

    func close() throws {
        try device.withFrameSession {
            defer {
                swapchain = nil; native = nil; multisample = nil; framebuffer = nil; size = .zero; state = .closed
                renderer.trimPresentationResources()
            }
            try swapchain?.close()
        }
    }
}
