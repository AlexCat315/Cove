internal import Foundation
import CoveUIRuntime

extension ChartConfiguration {
    public struct Viewport {
        /// Visible interval on the independent axis; nil fits all data.
        /// Numeric charts use x values; categories use zero-based indices.
        /// Enables wheel zoom, horizontal wheel pan, drag pan (Alt-drag when
        /// brushing or selecting), +/- keys and 0/double-click to reset.
        public var range: Binding<ClosedRange<Float>?>?
        /// Smallest visible share of the full axis, including logarithmic axes.
        public var minimumFraction: Float = 0.01

        public init() {}
    }
}

/// One independent-axis transform used by ticks, marks, brushing and navigation.
struct ChartWindow: Equatable {
    let full: ClosedRange<Float>
    let range: ClosedRange<Float>
    let kind: ChartScaleKind
    let minimumFraction: Float
    let isCategorical: Bool

    init(full: ClosedRange<Float>, range: ClosedRange<Float>? = nil,
         kind: ChartScaleKind = .linear, minimumFraction: Float = 0.01, isCategorical: Bool = false) {
        let minimum = minimumFraction.isFinite ? min(1, max(0.0001, minimumFraction)) : 0.01
        self = Self(full: full, validatedRange: full, kind: kind, minimumFraction: minimum,
                    isCategorical: isCategorical).replacing(range)
    }

    private init(full: ClosedRange<Float>, validatedRange range: ClosedRange<Float>, kind: ChartScaleKind,
                 minimumFraction: Float, isCategorical: Bool) {
        self.full = full; self.range = range; self.kind = kind; self.minimumFraction = minimumFraction
        self.isCategorical = isCategorical
    }

    func replacing(_ candidate: ClosedRange<Float>?) -> Self {
        guard let candidate, candidate.lowerBound.isFinite, candidate.upperBound.isFinite,
              kind != .log || candidate.lowerBound > 0 else {
            return Self(full: full, validatedRange: full, kind: kind, minimumFraction: minimumFraction, isCategorical: isCategorical)
        }
        let lo = space(full.lowerBound), hi = space(full.upperBound)
        let width = hi - lo
        let span = min(width, max(width * Double(minimumFraction), space(candidate.upperBound) - space(candidate.lowerBound)))
        let start = max(lo, min(space(candidate.lowerBound), hi - span))
        return Self(full: full, validatedRange: value(start)...value(start + span), kind: kind,
                    minimumFraction: minimumFraction, isCategorical: isCategorical)
    }

    func fraction(_ value: Float) -> Float {
        Float((space(value) - space(range.lowerBound)) / (space(range.upperBound) - space(range.lowerBound)))
    }

    func value(at fraction: Float) -> Float {
        value(space(range.lowerBound) + Double(fraction) * (space(range.upperBound) - space(range.lowerBound)))
    }

    func zoom(factor: Float, anchor: Float) -> Self {
        guard factor.isFinite, factor > 0, anchor.isFinite else { return self }
        let a = Double(min(1, max(0, anchor)))
        let start = space(range.lowerBound), span = space(range.upperBound) - start
        let next = min(space(full.upperBound) - space(full.lowerBound),
                       max((space(full.upperBound) - space(full.lowerBound)) * Double(minimumFraction), span * Double(factor)))
        let lower = start + (span - next) * a
        return replacing(value(lower)...value(lower + next))
    }

    func pan(fraction: Float) -> Self {
        guard fraction.isFinite else { return self }
        let delta = Double(fraction) * (space(range.upperBound) - space(range.lowerBound))
        return replacing(value(space(range.lowerBound) + delta)...value(space(range.upperBound) + delta))
    }

    func position(_ value: Float, plot: UIRect, orientation: ChartOrientation) -> Float {
        let ratio = fraction(value)
        // Numeric horizontal axes run upwards, matching their tick order.
        if orientation == .vertical { return plot.minX + ratio * plot.width }
        return isCategorical ? plot.minY + ratio * plot.height : plot.maxY - ratio * plot.height
    }

    func fraction(x: Float, y: Float, plot: UIRect, orientation: ChartOrientation) -> Float {
        if orientation == .vertical { return (x - plot.minX) / max(1, plot.width) }
        return isCategorical ? (y - plot.minY) / max(1, plot.height) : (plot.maxY - y) / max(1, plot.height)
    }

    private func space(_ value: Float) -> Double { kind == .log ? log(Double(value)) : Double(value) }
    private func value(_ value: Double) -> Float { Float(kind == .log ? exp(value) : value) }
}
