import Foundation

public enum GeometryAntialiasing: Sendable, Equatable {
    case analytic
    case multisample
}

enum AnalyticClipper {
    static func inset(_ mask: UIRoundedRect, by amount: Float) -> UIRoundedRect {
        let rect = mask.rect
        return UIRoundedRect(UIRect(x: rect.x + amount, y: rect.y + amount,
            width: max(0, rect.width - 2 * amount), height: max(0, rect.height - 2 * amount)), radius: max(0, mask.radius - amount))
    }

    static func clip(_ triangle: [UIVertex], masks: [UIRoundedRect], scale: Float) -> [[UIVertex]] {
        let scale = max(1, scale), half = 0.5 / max(1, scale)
        var pieces = [triangle]
        for mask in masks {
            let segments = max(4, min(64, Int(ceil(mask.radius * scale * .pi / 4))))
            let inside = inset(mask, by: half).outline(segments: segments)
            let outside = inset(mask, by: -half).outline(segments: segments)
            var result: [[UIVertex]] = []
            for piece in pieces {
                if piece.allSatisfy({ distance($0, mask) <= -half }) { result.append(piece); continue }
                let core = RoundedClipper.clip(piece, against: [inside])
                if core.count >= 3 { result.append(core) }
                for index in inside.indices {
                    let next = (index + 1) % inside.count
                    // This strip is disjoint from the core and other strips.
                    let outline = [inside[index], outside[index], outside[next], inside[next]]
                    let area = zip(outline, Array(outline.dropFirst()) + [outline[0]]).reduce(Float(0)) {
                        $0 + $1.0.x * $1.1.y - $1.1.x * $1.0.y
                    }
                    guard abs(area) > 0.000001 else { continue }
                    let clockwise = area > 0 ? outline : Array(outline.reversed())
                    var feather = RoundedClipper.clip(piece, against: [clockwise])
                    guard feather.count >= 3 else { continue }
                    for vertex in feather.indices {
                        let coverage = max(0, min(1, 0.5 - distance(feather[vertex], mask) * scale))
                        let alpha = Float(feather[vertex].color >> 24) * coverage
                        feather[vertex].color = (feather[vertex].color & 0x00ff_ffff) | UInt32(alpha.rounded()) << 24
                    }
                    result.append(feather)
                }
            }
            pieces = result
            if pieces.isEmpty { break }
        }
        return pieces
    }

    private static func distance(_ vertex: UIVertex, _ mask: UIRoundedRect) -> Float {
        let rect = mask.rect, radius = mask.radius
        let x = abs(vertex.posX - rect.x - rect.width / 2) - rect.width / 2 + radius
        let y = abs(vertex.posY - rect.y - rect.height / 2) - rect.height / 2 + radius
        return sqrt(max(x, 0) * max(x, 0) + max(y, 0) * max(y, 0)) + min(max(x, y), 0) - radius
    }
}
