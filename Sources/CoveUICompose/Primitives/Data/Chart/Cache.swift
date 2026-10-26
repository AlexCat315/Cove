internal import Foundation
import CoveUIRuntime

/// Reuses one resolved chart across the passes that need it.
///
/// Drawing, hit-testing and the overlay each resolve the same plot, which would
/// otherwise rebuild every stack, every scale and every mark four times per
/// frame. Two slots cover the two spaces a chart lives in: drawing and
/// hit-testing work in absolute coordinates, while the overlay lays out in the
/// node's own space.
///
/// The cache never decides *whether* the data changed — `ChartPlot` compares its
/// geometry signature and calls `invalidate()`. Selection and colors stay out of
/// that signature, so moving the pointer repaints without moving a mark.
final class ChartResolutionCache {
    private struct Slot {
        let size: CGSize
        let origin: CGPoint
        let text: TextStyleToken
        let resolution: ChartResolution
    }

    private var slots: [Slot] = []
    var samples: ChartSamples?
    /// How many resolutions have been built. A repeat resolve leaves it alone,
    /// which is the whole point of the cache.
    private(set) var builds = 0

    /// Forgets both slots, after the data or the text metrics change.
    func invalidate() { slots.removeAll(keepingCapacity: true) }

    func resolution(size: CGSize, origin: CGPoint, text: TextStyleToken,
                    build: () -> ChartResolution) -> ChartResolution {
        if let hit = slots.first(where: { $0.size == size && $0.origin == origin && $0.text == text }) {
            return hit.resolution
        }
        let value = build()
        builds += 1
        slots.append(Slot(size: size, origin: origin, text: text, resolution: value))
        // A resize drag walks through sizes one at a time; two slots keep the
        // two spaces warm without growing.
        if slots.count > 2 { slots.removeFirst() }
        return value
    }
}
