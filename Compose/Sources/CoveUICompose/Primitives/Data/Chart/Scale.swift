internal import Foundation
import CoveUIScene
import CoveUIText
import CoveUIImaging

/// The value axis of a cartesian chart: the domain the plot maps and the round
/// values ticked across it.
///
/// Fitting runs before layout. The domain decides the ticks, the ticks decide
/// the label widths, and the label widths decide how much room the plot keeps.
struct ChartScale {
    let domain: ClosedRange<Float>
    /// Ascending tick values inside `domain`, both bounds included. Empty when
    /// the axis asks for no ticks.
    let ticks: [Float]
    /// Decimal places that render the tick step exactly.
    let decimals: Int
    let kind: ChartScaleKind

    /// Fits the plotted values to round bounds at about `tickCount` intervals.
    ///
    /// Reference rules and bands widen the domain with the data, so a threshold
    /// or corridor is always visible. An explicit `minValue`/`maxValue` is
    /// honoured as-is; the ticks are then generated inside it instead of
    /// widening it. A non-positive `tickCount` still fits the domain but draws
    /// no ticks. A percent stack always fits linearly: shares are not a
    /// magnitude that a log axis would help read.
    static func fit(stack: ChartStack, thresholds: [ChartThreshold] = [], bands: [ChartBand] = [],
                    style: ChartStyle, tickCount: Int, kind: ChartScaleKind = .linear) -> ChartScale {
        let guides = thresholds.map(\.value) + bands.flatMap { [$0.from, $0.to] }
        return fit(extent: stack.extent + guides,
                   lower: style.minValue, upper: style.maxValue, tickCount: tickCount,
                   kind: stack.isPercent ? .linear : kind)
    }

    /// Fits raw values; the shared path for the value axis and a numeric x axis.
    static func fit(extent: [Float], lower explicitMin: Float?, upper explicitMax: Float?,
                    tickCount: Int, kind: ChartScaleKind = .linear) -> ChartScale {
        switch kind {
        case .linear: return fitLinear(extent: extent, lower: explicitMin, upper: explicitMax,
                                       tickCount: tickCount)
        case .log: return fitLog(extent: extent, lower: explicitMin, upper: explicitMax,
                                 tickCount: tickCount)
        }
    }

    private static func fitLinear(extent: [Float], lower explicitMin: Float?, upper explicitMax: Float?,
                                  tickCount: Int) -> ChartScale {
        let values = extent.filter(\.isFinite)
        let explicitMin = explicitMin.flatMap { $0.isFinite ? $0 : nil }
        let explicitMax = explicitMax.flatMap { $0.isFinite ? $0 : nil }
        var lower = explicitMin ?? values.min() ?? 0
        var upper = explicitMax ?? values.max() ?? 1
        if lower > upper { swap(&lower, &upper) }
        if lower == upper {
            let pad = max(abs(lower) * 0.1, 1)
            lower -= pad
            upper += pad
        }
        let step = niceStep((upper - lower) / Float(max(1, tickCount)))
        if explicitMin == nil { lower = (lower / step).rounded(.down) * step }
        if explicitMax == nil { upper = (upper / step).rounded(.up) * step }
        return ChartScale(domain: lower...upper,
                          ticks: tickCount > 0 ? ticks(from: lower, to: upper, step: step) : [],
                          decimals: decimals(for: step), kind: .linear)
    }

    /// Fits in log space and snaps to whole decades, so ticks read 1, 10, 100
    /// rather than 3.7. Non-positive values have no place on the axis and are
    /// ignored, as is a non-positive bound.
    private static func fitLog(extent: [Float], lower explicitMin: Float?, upper explicitMax: Float?,
                               tickCount: Int) -> ChartScale {
        let values = extent.filter { $0.isFinite && $0 > 0 }
        let explicitMin = explicitMin.flatMap { $0.isFinite && $0 > 0 ? $0 : nil }
        let explicitMax = explicitMax.flatMap { $0.isFinite && $0 > 0 ? $0 : nil }
        var lower = explicitMin ?? values.min() ?? 1
        var upper = explicitMax ?? values.max() ?? 10
        if lower > upper { swap(&lower, &upper) }
        if explicitMin == nil { lower = decade(floorf(log10f(lower))) }
        if explicitMax == nil { upper = decade(ceilf(log10f(upper))) }
        // A single value (or one exact decade) still needs a readable range.
        if upper <= lower { lower /= 10; upper *= 10 }
        let ticks = tickCount > 0 ? logTicks(from: lower, to: upper) : []
        return ChartScale(domain: lower...upper, ticks: ticks,
                          decimals: decimals(forLog: ticks), kind: .log)
    }

    /// The y for a value, clamped to the plot.
    func y(_ value: Float, in plot: UIRect) -> Float {
        plot.maxY - ratio(value) * plot.height
    }

    /// The x for a value, clamped to the plot.
    func x(_ value: Float, in plot: UIRect) -> Float {
        plot.minX + ratio(value) * plot.width
    }

    /// Where a value sits along the axis, 0 at the lower bound and 1 at the
    /// upper. A value off the axis clamps instead of leaving the plot; on a log
    /// axis anything at or below zero clamps to the bottom.
    private func ratio(_ value: Float) -> Float {
        guard value.isFinite else { return 0 }
        switch kind {
        case .linear:
            let span = max(domain.upperBound - domain.lowerBound, 0.001)
            return max(0, min((value - domain.lowerBound) / span, 1))
        case .log:
            guard value > 0, domain.lowerBound > 0 else { return 0 }
            let low = log10f(domain.lowerBound), high = log10f(domain.upperBound)
            let span = max(high - low, 0.0001)
            return max(0, min((log10f(value) - low) / span, 1))
        }
    }

    /// A tick label with the step's precision and no trailing zeros.
    func label(_ value: Float) -> String { Self.fixed(value, decimals: decimals) }

    /// A label at a fixed precision, with trailing zeros and a bare "-0" trimmed.
    static func fixed(_ value: Float, decimals: Int) -> String {
        guard value.isFinite else { return "" }
        var text = String(format: "%.\(decimals)f", value)
        if decimals > 0 {
            while text.hasSuffix("0") { text.removeLast() }
            if text.hasSuffix(".") { text.removeLast() }
        }
        return text == "-0" ? "0" : text
    }

    /// The smallest step of 1, 2, 2.5 or 5 × 10ⁿ that is at least `raw`.
    static func niceStep(_ raw: Float) -> Float {
        guard raw.isFinite, raw > 0 else { return 1 }
        let magnitude = powf(10, floorf(log10f(raw)))
        let fraction = raw / magnitude
        let nice: Float
        switch fraction {
        case ..<1.5: nice = 1
        case ..<2.25: nice = 2
        case ..<3.5: nice = 2.5
        case ..<7.5: nice = 5
        default: nice = 10
        }
        return nice * magnitude
    }

    /// Ticks on multiples of `step` that fall inside the domain.
    private static func ticks(from lower: Float, to upper: Float, step: Float) -> [Float] {
        let first = (lower / step).rounded(.up) * step
        let last = (upper / step).rounded(.down) * step
        guard last >= first else { return [lower, upper] }
        let count = Int(((last - first) / step).rounded())
        guard count >= 0, count <= 64 else { return [lower, upper] }
        return (0...count).map { first + Float($0) * step }
    }

    /// Ticks at 1, 2 and 5 × 10ⁿ inside the domain, thinned to what a reader can
    /// count: all of them when there are few, decades only when there are many.
    private static func logTicks(from lower: Float, to upper: Float) -> [Float] {
        let low = Int(max(-38, min(38, floorf(log10f(lower)))))
        let high = Int(max(-38, min(38, ceilf(log10f(upper)))))
        var ticks: [Float] = []
        for exponent in low...high {
            for multiplier: Float in [1, 2, 5] {
                let value = multiplier * decade(Float(exponent))
                if value >= lower * 0.9999 && value <= upper * 1.0001 { ticks.append(value) }
            }
        }
        guard ticks.count > 1 else { return [lower, upper] }
        var chosen = ticks.count <= 9 ? ticks : ticks.filter(isDecade)
        if chosen.count > 9 {
            let stride = Int((Float(chosen.count) / 9).rounded(.up))
            chosen = chosen.enumerated().compactMap { $0.offset % stride == 0 ? $0.element : nil }
        }
        return chosen
    }

    private static func isDecade(_ value: Float) -> Bool {
        let exponent = log10f(value)
        return abs(exponent - exponent.rounded()) < 0.0001
    }

    private static func decade(_ exponent: Float) -> Float {
        powf(10, max(-38, min(38, exponent)))
    }

    /// The first decimal count in 0...4 that represents `step` without rounding.
    private static func decimals(for step: Float) -> Int {
        guard step.isFinite, step > 0 else { return 0 }
        for places in 0...4 {
            let scaled = step * powf(10, Float(places))
            if abs(scaled - scaled.rounded()) < 0.0001 * max(1, abs(scaled)) { return places }
        }
        return 4
    }

    /// Log ticks span decades, so the precision is set by the smallest one.
    private static func decimals(forLog ticks: [Float]) -> Int {
        var places = 0
        for tick in ticks where tick > 0 && tick < 1 {
            places = max(places, Int(ceilf(-log10f(tick))))
        }
        return min(places, 4)
    }
}
