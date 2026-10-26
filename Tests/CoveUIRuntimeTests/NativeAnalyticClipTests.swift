#if os(macOS)
import Foundation
import Testing
@testable import CoveUIRuntime

@Suite("Analytic coverage", .serialized)
struct NativeAnalyticClipTests {
    @Test func translucentMaskAndStrokeDoNotAccumulateAlpha() throws {
        let context = try NativeUIDrawTestContext()
        for stroke in [false, true] {
            let list = DrawList()
            list.pixelScale = 1
            list.geometryAntialiasing = .analytic
            let rect = UIRect(x: 12, y: 12, width: 40, height: 40)
            if stroke {
                list.addRoundedRectStroke(rect, radius: 10, width: 3, color: Color(r: 1, g: 1, b: 1, a: 0.5))
            } else {
                list.pushMask(UIRoundedRect(rect, radius: 10))
                list.addRect(UIRect(x: 0, y: 0, width: 64, height: 64), color: Color(r: 1, g: 1, b: 1, a: 0.5))
                list.popMask()
            }
            let image = try context.nativeImage(list: list, format: NativeUITestFormat.cases[0], samples: 1,
                viewport: NativeUIViewport(pixels: SIMD2(64, 64), logical: SIMD2(64, 64))).0
            let alpha = stride(from: 3, to: image.count, by: 4).map { image[$0] }
            #expect(alpha.allSatisfy { $0 <= 193 })
            #expect(alpha.contains { $0 > 130 && $0 < 188 })
        }
    }
}
#endif
