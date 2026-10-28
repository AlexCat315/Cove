import Foundation

/// Coverage geometry is emitted before clipping. Shared strips never overlap,
/// so translucent strokes do not darken at segment joins.
public extension DrawList {
    func addPathStroke(_ points: [SIMD2<Float>], width: Float, color: Color, closed: Bool = false) {
        guard width.isFinite, width > 0, color.a > 0 else { return }
        var path: [SIMD2<Float>] = []
        for point in points {
            guard point.x.isFinite, point.y.isFinite else { return }
            if path.last != point { path.append(point) }
        }
        if closed, path.count > 1, path.first == path.last { path.removeLast() }
        guard path.count >= (closed ? 3 : 2) else { return }
        let feather = 0.5 / max(1, pixelScale), half = width * 0.5
        let peak = min(1, width / (half + feather))
        let opaque = color.multipliedAlpha(peak).rgba8, clear = opaque & 0x00ff_ffff
        var output: [UIVertex] = [], triangles: [UInt32] = []
        func unit(_ value: SIMD2<Float>) -> SIMD2<Float> {
            value / max(0.00001, (value.x * value.x + value.y * value.y).squareRoot())
        }
        func normal(_ direction: SIMD2<Float>) -> SIMD2<Float> { SIMD2(-direction.y, direction.x) }
        for index in path.indices {
            let incoming = unit(path[index] - path[index == 0 ? (closed ? path.count - 1 : 0) : index - 1])
            let outgoing = unit(path[index == path.count - 1 ? (closed ? 0 : index) : index + 1] - path[index])
            let direction = !closed && index == 0 ? outgoing : !closed && index == path.count - 1 ? incoming : unit(incoming + outgoing)
            var miter = normal(direction)
            let edge = normal(index == path.count - 1 && !closed ? incoming : outgoing)
            let divisor = max(0.5, abs(miter.x * edge.x + miter.y * edge.y))
            miter /= divisor
            var position = path[index]
            if !closed && index == 0 { position += outgoing * min(feather, half) }
            if !closed && index == path.count - 1 { position -= incoming * min(feather, half) }
            let distances: [Float] = [half + feather, max(0, half - feather), -max(0, half - feather), -half - feather]
            for (band, distance) in distances.enumerated() {
                let point = position + miter * distance
                output.append(UIVertex(posX: point.x, posY: point.y, u: -1, v: 0,
                    color: band == 0 || band == 3 ? clear : opaque))
            }
        }
        func connect(_ a: UInt32, _ b: UInt32) {
            for band: UInt32 in 0..<3 {
                triangles.append(contentsOf: [a + band, b + band, a + band + 1,
                    a + band + 1, b + band, b + band + 1])
            }
        }
        for index in 0..<(closed ? path.count : path.count - 1) {
            connect(UInt32(index * 4), UInt32(((index + 1) % path.count) * 4))
        }
        if !closed {
            for (index, sign) in [(0, Float(-1)), (path.count - 1, Float(1))] {
                let direction = unit(index == 0 ? path[1] - path[0] : path[index] - path[index - 1])
                let base = UInt32(output.count)
                for band in 0..<4 {
                    var vertex = output[index * 4 + band]
                    vertex.posX += direction.x * sign * min(2 * feather, width)
                    vertex.posY += direction.y * sign * min(2 * feather, width)
                    vertex.color = clear
                    output.append(vertex)
                }
                connect(UInt32(index * 4), base)
            }
        }
        appendCoverageGeometry(output, triangles)
    }

    func addArcStroke(center: SIMD2<Float>, radius: Float, start: Float, sweep: Float,
                      width: Float, color: Color) {
        guard radius.isFinite, radius > 0, start.isFinite, sweep.isFinite, abs(sweep) > 0.0001 else { return }
        let sweep = max(-2 * Float.pi, min(2 * Float.pi, sweep))
        let estimate = (abs(sweep) * radius * max(1, pixelScale)).rounded(.up)
        let count = Int(max(12, min(512, estimate)))
        let closed = abs(sweep) >= 2 * .pi - 0.0001
        let points = (0..<(closed ? count : count + 1)).map { index in
            let angle = Double(start + Float(index) / Float(count) * sweep)
            return center + SIMD2(Float(cos(angle)), Float(sin(angle))) * radius
        }
        addPathStroke(points, width: width, color: color, closed: closed)
    }
}
