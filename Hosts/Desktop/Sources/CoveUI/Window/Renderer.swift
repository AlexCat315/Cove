import CoveUIRuntime
import PlatformShell

/// Presentation ownership is separate from layout and input. All calls occur
/// on the UI thread; the native implementation also coordinates scene frames.
@MainActor
protocol AppWindowRenderer: AnyObject {
    var isConfigured: Bool { get }
    var memorySnapshot: AppWindowMemorySnapshot { get }
    func configure(native: NativeRenderSurface, size: SIMD2<UInt32>, vsync: Bool) throws
    func resize(size: SIMD2<UInt32>, vsync: Bool) throws
    func draw(list: DrawList, logical: SIMD2<Float>) throws -> Bool
    func setPresentationActive(_ active: Bool) throws
    func close() throws
}
