import Foundation
import Testing
@testable import CoveUIScene

@Suite("Render pipeline performance", .serialized)
@MainActor
struct RenderPipelinePerformanceTests {
    @Test(.enabled(if: ProcessInfo.processInfo.environment["COVE_RENDER_BENCHMARK"] == "1"), arguments: [Float(0), 16])
    func scrolling(cornerRadius: Float) {
        let root = Node()
        root.frame = CGRect(x: 0, y: 0, width: 1200, height: 800)
        root.clipsToBounds = true
        root.cornerRadius = cornerRadius
        for row in 0..<1200 {
            let container = Node()
            container.frame = CGRect(x: 0, y: row * 24, width: 1200, height: 24)
            root.addChild(container)
            let leaf = Node()
            leaf.frame = CGRect(x: 4, y: 2, width: 1160, height: 20)
            leaf.draw = { list, origin in
                for column in 0..<8 {
                    list.addRect(UIRect(x: Float(origin.x) + Float(column * 100), y: Float(origin.y),
                                        width: 90, height: 18), color: .white)
                }
            }
            container.addChild(leaf)
        }
        let tree = RenderTree(); tree.install(rootNode: root)
        let renderer = LayerAwareNodeRenderer(), list = DrawList()
        var samples: [Double] = []
        for frame in 0..<90 {
            root.contentOffset.y = CGFloat(frame * 7)
            list.reset()
            let start = ContinuousClock.now
            renderer.render(tree: tree, into: list)
            let elapsed = start.duration(to: .now).components
            if frame >= 30 { samples.append(Double(elapsed.seconds) * 1_000 + Double(elapsed.attoseconds) / 1e15) }
        }
        samples.sort()
        print("COVE_RENDER_BENCHMARK scrolling radius=\(cornerRadius) p50=\(samples[samples.count / 2]) ms p95=\(samples[Int(Double(samples.count) * 0.95)]) ms")
        #expect(!list.indices.isEmpty)
    }
}
