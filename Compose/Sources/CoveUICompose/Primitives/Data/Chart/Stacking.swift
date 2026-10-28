internal import Foundation
import CoveUIScene
import CoveUIText
import CoveUIImaging

/// Per-series baselines and tops after stacking, plus what the scale needs to
/// know about them.
///
/// The scale fit and the mark geometry both read this, so the domain always
/// matches what is actually drawn instead of being recomputed from raw values.
/// A non-finite value stays non-finite here: it is a gap, not a zero.
///
/// Stacking runs away from zero in both directions, so a negative value sits
/// under the baseline instead of cancelling the surplus above it: one column
/// can carry a gain and a loss at once.
struct ChartStack {
    /// Series indices on this axis that take part, in order. Hidden series and
    /// series on the other value axis have no marks here.
    let shown: [Int]
    let bases: [[Float]]
    let tops: [[Float]]
    /// The outermost nonzero bar at an index in each direction. Only that
    /// segment's end away from zero stays rounded.
    let outermostUp: [Int?]
    let outermostDown: [Int?]
    let hasBars: Bool
    let isStacked: Bool
    let isPercent: Bool
    let sampleCount: Int
    /// Values the domain must contain: every finite top, plus zero for bars and areas.
    let extent: [Float]

    init(series: [ChartSeries], visible: [Bool], style: ChartStyle,
         axis: ChartValueAxis = .left) {
        let shown = series.indices.filter {
            visible.indices.contains($0) && visible[$0] && series[$0].axis == axis
        }
        self.shown = shown
        hasBars = shown.contains { series[$0].mark == .bar }
        let sampleCount = shown.map { series[$0].data.count }.max() ?? 0
        self.sampleCount = sampleCount
        let stacking = style.stacking
        isStacked = stacking != .none
        isPercent = stacking == .percent

        var up = [Float](repeating: 0, count: sampleCount)
        var down = [Float](repeating: 0, count: sampleCount)
        var tops = [[Float]](repeating: [], count: series.count)
        var bases = [[Float]](repeating: [], count: series.count)
        for index in shown {
            let data = series[index].data
            // Percent mode takes every series: a chart of shares has no room for
            // one series reading raw values off the same axis.
            let stacks = isPercent
                || (isStacked && (series[index].mark == .bar || series[index].mark == .area))
            bases[index] = data.indices.map { i in
                guard data[i].y.isFinite else { return .nan }
                guard stacks else { return 0 }
                return data[i].y >= 0 ? up[i] : down[i]
            }
            tops[index] = data.indices.map { i in
                guard data[i].y.isFinite else { return .nan }
                return bases[index][i] + data[i].y
            }
            // A gap contributes nothing to the stack above it.
            if stacks {
                for (i, top) in tops[index].enumerated() where top.isFinite {
                    if data[i].y >= 0 { up[i] = top } else { down[i] = top }
                }
            }
        }
        if isPercent {
            // Scale each index by its taller side, so one side always reads 100
            // and the axis stays fixed at 0…100 (−100…100 with negatives).
            var factor = [Float](repeating: 1, count: sampleCount)
            for i in 0..<sampleCount {
                let span = max(up[i], -down[i])
                if span > 0 { factor[i] = 100 / span }
            }
            for index in shown {
                for i in bases[index].indices where bases[index][i].isFinite {
                    bases[index][i] *= factor[i]
                    tops[index][i] *= factor[i]
                }
            }
        }
        var outermostUp = [Int?](repeating: nil, count: sampleCount)
        var outermostDown = [Int?](repeating: nil, count: sampleCount)
        if isStacked {
            for index in shown where series[index].mark == .bar {
                for (i, datum) in series[index].data.enumerated() where datum.y.isFinite && datum.y != 0 {
                    if datum.y > 0 { outermostUp[i] = index } else { outermostDown[i] = index }
                }
            }
        }
        self.bases = bases
        self.tops = tops
        self.outermostUp = outermostUp
        self.outermostDown = outermostDown

        var extent = shown.flatMap { tops[$0] }.filter(\.isFinite)
        if shown.contains(where: { [.bar, .area].contains(series[$0].mark) }) { extent.append(0) }
        self.extent = extent
    }

    /// Whether a bar's far end is exposed, so it can take a rounded corner.
    func isOuterEnd(series index: Int, at position: Int, value: Float) -> Bool {
        guard isStacked else { return true }
        let outermost = value >= 0 ? outermostUp : outermostDown
        return outermost.indices.contains(position) && outermost[position] == index
    }
}
