internal import Foundation
import CoveUIRuntime

/// The value axes a chart resolves: one stack and one scale per side.
///
/// Each side fits only its own series, so a rate and a count can share a plot
/// without either flattening the other. The left axis is always present; the
/// right one exists only when some visible series reads it.
struct ChartValueAxes {
    let primary: ChartStack
    let secondary: ChartStack?
    let primaryScale: ChartScale
    let secondaryScale: ChartScale?
    /// x slots: the widest series on either axis.
    let sampleCount: Int
    /// Any visible series draws bars, so every mark centers on a bar slot.
    let hasBars: Bool
    let isStacked: Bool

    /// Resolves both sides from the same data and style.
    ///
    /// Rules and bands widen the domain of the axis they belong to, so a
    /// threshold on the right axis never stretches the left one.
    static func resolve(series: [ChartSeries], visible: [Bool], style: ChartStyle,
                        guides: ChartGuides, axis: ChartConfiguration.Axis) -> ChartValueAxes {
        let primary = ChartStack(series: series, visible: visible, style: style, axis: .left)
        let usesSecondary = series.indices.contains {
            visible.indices.contains($0) && visible[$0] && series[$0].axis == .right
        }
        let secondary = usesSecondary
            ? ChartStack(series: series, visible: visible, style: style, axis: .right)
            : nil
        let primaryScale = ChartScale.fit(stack: primary,
                                          thresholds: guides.thresholds.filter { $0.axis == .left },
                                          bands: guides.bands.filter { $0.axis == .left },
                                          style: style, tickCount: axis.y.tickCount,
                                          kind: axis.y.kind)
        let secondaryScale = secondary.map { stack in
            ChartScale.fit(stack: stack,
                           thresholds: guides.thresholds.filter { $0.axis == .right },
                           bands: guides.bands.filter { $0.axis == .right },
                           style: style, tickCount: axis.secondary.tickCount,
                           kind: axis.secondary.kind)
        }
        return ChartValueAxes(primary: primary, secondary: secondary,
                              primaryScale: primaryScale, secondaryScale: secondaryScale,
                              sampleCount: max(primary.sampleCount, secondary?.sampleCount ?? 0),
                              hasBars: primary.hasBars || (secondary?.hasBars ?? false),
                              isStacked: primary.isStacked)
    }

    var usesSecondary: Bool { secondary != nil }

    /// Every visible series index on either axis, in series order.
    var shown: [Int] { (primary.shown + (secondary?.shown ?? [])).sorted() }

    /// The stack a series on `axis` belongs to. A right-axis request on a chart
    /// without one falls back to the left, so callers never need an optional.
    func stack(_ axis: ChartValueAxis) -> ChartStack {
        axis == .right ? (secondary ?? primary) : primary
    }

    func scale(_ axis: ChartValueAxis) -> ChartScale {
        axis == .right ? (secondaryScale ?? primaryScale) : primaryScale
    }
}
