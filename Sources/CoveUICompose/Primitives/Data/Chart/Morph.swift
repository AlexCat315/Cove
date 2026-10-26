import CoveUIRuntime

/// Only sampled values animate. Authored data and session state stay separate.
struct ChartSamples: Interpolatable, Equatable {
    var series: [[ChartDatum]] = []
    var slices: [Float] = []

    init(_ content: ChartContent) {
        switch content {
        case .cartesian(let series, _, _): self.series = series.map(\.data)
        case .pie(let slices): self.slices = slices.map(\.value)
        }
    }

    func applying(to content: ChartContent) -> ChartContent {
        switch content {
        case .cartesian(var items, let guides, let x):
            for i in items.indices where series.indices.contains(i) { items[i].data = series[i] }
            return .cartesian(series: items, guides: guides, x: x)
        case .pie(var items):
            for i in items.indices where slices.indices.contains(i) { items[i].value = slices[i] }
            return .pie(items)
        }
    }

    static func interpolate(_ from: Self, _ to: Self, t progress: Float) -> Self {
        var result = to
        for s in to.series.indices {
            for i in to.series[s].indices {
                let target = to.series[s][i]
                let source = from.series.indices.contains(s) && from.series[s].indices.contains(i)
                    ? from.series[s][i] : ChartDatum(x: target.x, y: 0)
                result.series[s][i] = ChartDatum(x: mix(source.x, target.x, progress),
                                                y: mix(source.y, target.y, progress))
            }
        }
        for i in to.slices.indices {
            result.slices[i] = mix(from.slices.indices.contains(i) ? from.slices[i] : 0, to.slices[i], progress)
        }
        return result
    }

    private static func mix(_ a: Float, _ b: Float, _ t: Float) -> Float {
        guard a.isFinite, b.isFinite else { return b }
        return a + (b - a) * min(1, max(0, t))
    }

    static func == (lhs: Self, rhs: Self) -> Bool {
        func same(_ a: Float, _ b: Float) -> Bool { a == b || (a.isNaN && b.isNaN) }
        return lhs.slices.elementsEqual(rhs.slices, by: same)
            && lhs.series.elementsEqual(rhs.series) { a, b in
                a.elementsEqual(b) { same($0.x, $1.x) && same($0.y, $1.y) }
            }
    }
}

final class ChartTransition {
    private var target: ChartSamples?

    func update(content: ChartContent, animation: Animation?, node: Node, cache: ChartResolutionCache) {
        let next = ChartSamples(content)
        guard target != next || node.prefersReducedMotion else { return }
        let previous = cache.samples ?? next
        target = next
        let apply = {
            node.animatableSet(propertyKey: "chart.samples", current: previous, to: next) { [weak node, weak cache] value in
                guard let cache else { return }
                cache.samples = value
                cache.invalidate()
                node?.markDirty()
            }
        }
        if let animation { withAnimation(animation, apply) } else { apply() }
    }
}
