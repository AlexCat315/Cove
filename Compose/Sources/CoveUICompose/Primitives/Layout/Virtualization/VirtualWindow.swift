import Foundation

/// Live scroll position is imperative; only crossing the retained window's
/// refill threshold needs a composition update.
final class VirtualWindow {
    var offset = CGPoint.zero
    private var range = 0..<0
    private var count = 0
    private var leadingGuard = 0
    private var trailingGuard = 0
    private var size = CGSize.zero

    static func preparation(offset: Double, viewport: Double, preload: Float) -> (offset: Double, height: Double) {
        let fraction = preload.isFinite ? max(0, min(4, preload)) : 0.3
        let inset = max(0, viewport) * Double(fraction) / 2
        let start = max(0, offset - inset)
        return (start, max(1, viewport) + offset - start + inset)
    }

    func record(range: Range<Int>, visible: Range<Int>, count: Int, size: CGSize) {
        self.range = range; self.count = count; self.size = size
        leadingGuard = max(0, visible.lowerBound - range.lowerBound) / 2
        trailingGuard = max(0, range.upperBound - visible.upperBound) / 2
    }

    func needsRefill(visible: Range<Int>, size: CGSize) -> Bool {
        guard self.size == size else { return true }
        guard count > 0, size.height > 0 else { return false }
        guard !range.isEmpty else { return true }
        let first = range.lowerBound + (range.lowerBound > 0 ? leadingGuard : 0)
        let last = range.upperBound - (range.upperBound < count ? trailingGuard : 0)
        return visible.lowerBound < first || visible.upperBound > last
    }
}

struct VirtualRowKey: Equatable {
    let generation: UUID
    let index: Int
    let extent: Float
    var width: Float = 0
}
