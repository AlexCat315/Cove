internal import Foundation
import GuavaUIRuntime

/// Per-series baselines and tops after stacking, plus what the scale needs to
/// know about them.
///
/// The scale fit and the mark geometry both read this, so the domain always
/// matches what is actually drawn instead of being recomputed from raw values.
/// A non-finite value stays non-finite here: it is a gap, not a zero.
struct ChartStack {
    /// Series indices that take part, in order. Hidden series have no marks.
    let shown: [Int]
    let bases: [[Float]]
    let tops: [[Float]]
    /// The last nonzero stacked bar at an index, whose top end stays rounded.
    let outermost: [Int?]
    let hasBars: Bool
    let isStacked: Bool
    let sampleCount: Int
    /// Values the domain must contain: every finite top, plus zero for bars and areas.
    let extent: [Float]

    init(series: [ChartSeries], visible: [Bool], style: ChartStyle) {
        let shown = series.indices.filter { visible.indices.contains($0) && visible[$0] }
        self.shown = shown
        hasBars = shown.contains { series[$0].mark == .bar }
        let sampleCount = shown.map { series[$0].data.count }.max() ?? 0
        self.sampleCount = sampleCount
        let stacked = style.stacking == .stacked
        isStacked = stacked

        var stack = [Float](repeating: 0, count: sampleCount)
        var tops = [[Float]](repeating: [], count: series.count)
        var bases = [[Float]](repeating: [], count: series.count)
        for index in shown {
            let data = series[index].data
            let stacks = stacked && (series[index].mark == .bar || series[index].mark == .area)
            bases[index] = data.indices.map { i in
                guard data[i].y.isFinite else { return .nan }
                return stacks ? stack[i] : 0
            }
            tops[index] = data.indices.map { i in
                guard data[i].y.isFinite else { return .nan }
                return bases[index][i] + data[i].y
            }
            // A gap contributes nothing to the stack above it.
            if stacks { for (i, top) in tops[index].enumerated() where top.isFinite { stack[i] = top } }
        }
        var outermost = [Int?](repeating: nil, count: sampleCount)
        if stacked {
            for index in shown where series[index].mark == .bar {
                for (i, datum) in series[index].data.enumerated() where datum.y.isFinite && datum.y != 0 {
                    outermost[i] = index
                }
            }
        }
        self.bases = bases
        self.tops = tops
        self.outermost = outermost

        var extent = shown.flatMap { tops[$0] }.filter(\.isFinite)
        if shown.contains(where: { [.bar, .area].contains(series[$0].mark) }) { extent.append(0) }
        self.extent = extent
    }
}
