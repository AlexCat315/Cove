import CoveUICore

enum RenderCulling {
    static func isOutside(_ rect: UIRect, list: DrawList) -> Bool {
        if let clip = list.currentClip, !intersects(rect, clip) { return true }
        if let viewport = list.viewportBounds, !intersects(rect, viewport) { return true }
        return false
    }

    static func bounds(of list: DrawList) -> UIRect? {
        list.paintBounds
    }

    private static func intersects(_ a: UIRect, _ b: UIRect) -> Bool {
        a.minX < b.maxX && a.maxX > b.minX && a.minY < b.maxY && a.maxY > b.minY
    }
}
