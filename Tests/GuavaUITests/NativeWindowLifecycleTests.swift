#if os(macOS)
import AppKit
import QuartzCore
import NativeRHI
import GuavaUIRuntime
import Testing
@testable import GuavaUI

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
