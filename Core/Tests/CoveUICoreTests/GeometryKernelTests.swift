import Foundation
import Testing
@testable import CoveUICore

@Suite("Bulk geometry kernels")
struct GeometryKernelTests {
    @Test("Translated copies and rebased indices preserve COW, attributes, clips and uneven worker tails",
           arguments: [0, 1, 15, 16_383, 16_384, 32_771, 131_079])
    func translatedCopy(count: Int) {
        let points = (0..<count).map { index in
            UIVertex(posX: Float(index) * 0.25, posY: -Float(index), u: Float(index % 7),
                     v: Float(index % 11), color: UInt32(truncatingIfNeeded: index) &* 71)
        }
        let triangles = (0..<count).map(UInt32.init)
        let batches = count == 0 ? [] : [DrawBatch(indexOffset: 0, indexCount: UInt32(count), textureID: 42,
                                                  scissor: UIRect(x: 1, y: 2, width: 3, height: 4))]
        let source = DrawList(); source.load(vertices: points, indices: triangles, batches: batches)
        let destination = DrawList()
        // The first append shares the buffers; the translated append must detach.
        destination.append(source)
        destination.append(source, vertexTranslationX: 3.5, vertexTranslationY: -2.25)
        #expect(source.vertices.map(\.posX) == points.map(\.posX))
        #expect(source.indices == triangles)
        #expect(destination.vertices.count == count * 2)
        #expect(destination.indices == triangles + triangles.map { $0 + UInt32(count) })
        #expect(zip(destination.vertices.dropFirst(count), points).allSatisfy { actual, original in
            actual.posX == original.posX + 3.5 && actual.posY == original.posY - 2.25 &&
            actual.u == original.u && actual.v == original.v && actual.color == original.color
        })
        #expect(destination.batches.allSatisfy { $0.scissor == batches.first?.scissor && $0.textureID == 42 })
        if count > 0 { #expect(destination.batches.last?.indexOffset == UInt32(count)) }
    }

    @Test("Parallel opacity matches scalar rounding and leaves the prefix and RGB unchanged",
           arguments: [Float(0), 0.1, 0.5, 0.9999, 1, -1, 2])
    func opacity(alpha: Float) {
        let points = (0..<32_771).map { index in
            UIVertex(posX: Float(index), posY: 1, u: 0, v: 0,
                     color: (UInt32(index % 256) << 24) | 0x00123456)
        }
        let list = DrawList(); list.load(vertices: points, indices: [], batches: [])
        list.multiplyOpacity(fromVertexIndex: 17, by: alpha)
        let multiplier = max(0, min(1, alpha))
        #expect(zip(list.vertices.indices, list.vertices).allSatisfy { index, vertex in
            let expected = index < 17 ? points[index].color :
                0x00123456 | (UInt32((Float(points[index].color >> 24) * multiplier).rounded()) << 24)
            return vertex.color == expected && vertex.posX == points[index].posX
        })
        #expect(points[17].color >> 24 == 17)
    }

    @Test("Bounds include overflow geometry and handle empty lists")
    func bounds() {
        let list = DrawList()
        #expect(list.paintBounds == nil)
        list.addRect(UIRect(x: -20, y: -8, width: 30, height: 60), color: .white)
        #expect(list.paintBounds == UIRect(x: -20, y: -8, width: 30, height: 60))
    }
}
