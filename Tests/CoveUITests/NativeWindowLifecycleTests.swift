#if os(macOS)
import AppKit
import QuartzCore
import NativeRHI
import CoveUIRuntime
import Testing
@testable import CoveUI

@Suite("Native window lifecycle", .serialized)
struct NativeWindowLifecycleTests {
    @Test @MainActor func suspendResumeReleasesTargetsAndUsesLatestSize() throws {
        _ = NSApplication.shared
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 64, height: 64),
            styleMask: [.borderless], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        let view = NSView(frame: window.contentLayoutRect)
        let layer = CAMetalLayer()
        view.wantsLayer = true
        view.layer = layer
        window.contentView = view
        window.orderFront(nil)
        CATransaction.flush()
        let device = try Device.make(DeviceConfig(preferredBackends: [.metal], enableValidation: true))
        let renderer = try NativeDrawListRenderer(device: device)
        let windowRenderer = NativeAppWindowRenderer(device: device, renderer: renderer,
            settings: AppWindowRenderSettings(config: ApplicationConfiguration()))
        defer { try? windowRenderer.close(); window.orderOut(nil) }
        try windowRenderer.configure(native: .metalLayer(Unmanaged.passUnretained(layer).toOpaque()),
            size: SIMD2(64, 64), vsync: false)
        let list = DrawList()
        list.addRect(UIRect(x: 0, y: 0, width: 64, height: 64), color: .white)
        for index in 0..<100 {
            #expect(try windowRenderer.draw(list: list, logical: SIMD2(64, 64)))
            #expect(windowRenderer.memorySnapshot.estimatedTargetBytes > 0)
            #if arch(arm64)
            #expect(windowRenderer.memorySnapshot.msaaMemoryless)
            #expect(windowRenderer.memorySnapshot.msaaAllocatedBytes == 0)
            #endif
            if index == 0, renderer.supportsBackdrop {
                let frosted = DrawList()
                frosted.addRect(UIRect(x: 0, y: 0, width: 64, height: 64), color: .white)
                frosted.beginBackdrop(UIBackdrop(rect: UIRect(x: 8, y: 8, width: 48, height: 48), blurRadius: 3))
                frosted.addRect(UIRect(x: 8, y: 8, width: 48, height: 48), color: Color(r: 0.2, g: 0.3, b: 0.4, a: 0.5))
                #expect(try windowRenderer.draw(list: frosted, logical: SIMD2(64, 64)))
                #expect(!windowRenderer.memorySnapshot.msaaMemoryless)
                #expect((windowRenderer.memorySnapshot.msaaAllocatedBytes ?? 0) > 0)
                #expect(try windowRenderer.draw(list: list, logical: SIMD2(64, 64)))
                #if arch(arm64)
                #expect(windowRenderer.memorySnapshot.msaaMemoryless)
                #expect(windowRenderer.memorySnapshot.msaaAllocatedBytes == 0)
                #endif
            }
            try windowRenderer.setPresentationActive(false)
            #expect(windowRenderer.memorySnapshot.state == .suspended)
            #expect(!windowRenderer.memorySnapshot.hasSwapchain)
            #expect(windowRenderer.memorySnapshot.estimatedTargetBytes == 0)
            let side = UInt32(index % 2 == 0 ? 80 : 64)
            try windowRenderer.resize(size: SIMD2(side, side), vsync: false)
            #expect(try !windowRenderer.draw(list: list, logical: SIMD2(64, 64)))
            try windowRenderer.setPresentationActive(true)
            #expect(!windowRenderer.memorySnapshot.hasSwapchain)
            #expect(try windowRenderer.draw(list: list, logical: SIMD2(64, 64)))
            #expect(windowRenderer.memorySnapshot.estimatedTargetBytes == Int(side * side * 4 * 4))
        }
        try windowRenderer.close()
        #expect(windowRenderer.memorySnapshot.state == .closed)
        #expect(windowRenderer.memorySnapshot.estimatedTargetBytes == 0)
    }
}
#endif
