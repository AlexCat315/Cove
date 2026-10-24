internal import Foundation
import GuavaUIRuntime

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

    /// Fits the plotted values to round bounds at about `tickCount` intervals.
    ///
    /// Reference rules and bands widen the domain with the data, so a threshold
    /// or corridor is always visible. An explicit `minValue`/`maxValue` is
    /// honoured as-is; the ticks are then generated inside it instead of
    /// widening it. A non-positive `tickCount` still fits the domain but draws
    /// no ticks.
    static func fit(stack: ChartStack, thresholds: [ChartThreshold] = [], bands: [ChartBand] = [],
                    style: ChartStyle, tickCount: Int) -> ChartScale {
        let guides = thresholds.map(\.value) + bands.flatMap { [$0.from, $0.to] }
        return fit(extent: stack.extent + guides,
                   lower: style.minValue, upper: style.maxValue, tickCount: tickCount)
    }

    /// Fits raw values; the shared path for the value axis and a numeric x axis.
    static func fit(extent: [Float], lower explicitMin: Float?, upper explicitMax: Float?,
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
                          decimals: decimals(for: step))
    }

    /// The y for a value, clamped to the plot.
    func y(_ value: Float, in plot: UIRect) -> Float {
        let span = max(domain.upperBound - domain.lowerBound, 0.001)
        let ratio = max(0, min((value - domain.lowerBound) / span, 1))
        return plot.maxY - ratio * plot.height
    }

    /// The x for a value, clamped to the plot.
    func x(_ value: Float, in plot: UIRect) -> Float {
        let span = max(domain.upperBound - domain.lowerBound, 0.001)
        let ratio = max(0, min((value - domain.lowerBound) / span, 1))
        return plot.minX + ratio * plot.width
    }

    /// A tick label with the step's precision and no trailing zeros.
    func label(_ value: Float) -> String {
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

    /// The first decimal count in 0...4 that represents `step` without rounding.
    private static func decimals(for step: Float) -> Int {
        guard step.isFinite, step > 0 else { return 0 }
        for places in 0...4 {
            let scaled = step * powf(10, Float(places))
            if abs(scaled - scaled.rounded()) < 0.0001 * max(1, abs(scaled)) { return places }
        }
        return 4
    }
}
