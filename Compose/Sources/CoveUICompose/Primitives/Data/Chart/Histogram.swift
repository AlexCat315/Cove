internal import Foundation
import CoveUIScene
import CoveUIText
import CoveUIImaging

/// Counts samples into even-width bins, for a distribution chart.
///
/// ```swift
/// let histogram = ChartHistogram(binCount: 8).build(latencies, title: "Latency")
/// Chart([histogram.series], x: .categorical(histogram.labels),
///       style: ChartStyle(showsGrid: false, barSpacing: 0)) {
///     $0.axis.y.title = "Requests"
/// }
/// ```
///
/// Binning is a data concern, not a drawing one, so it happens here and the
/// chart just draws the bars. Each bin is labelled with its edges; samples
/// outside an explicit `range` are ignored rather than folded into the end bins,
/// which would misreport the tails.
public struct ChartHistogram: Sendable {
    public struct Result: Sendable {
        /// Bars, one per bin, titled `title`.
        public let series: ChartSeries
        /// Each bin's edges, e.g. "0–120".
        public let labels: [String]
        /// How many samples landed in each bin.
        public let counts: [Int]
    }

    /// Number of bins, clamped to 1...64.
    public var binCount: Int
    /// The span to bin over. `nil` fits the samples, so nothing is dropped.
    public var range: ClosedRange<Float>?

    public init(binCount: Int = 10, range: ClosedRange<Float>? = nil) {
        self.binCount = min(max(binCount, 1), 64)
        self.range = range.flatMap {
            $0.lowerBound.isFinite && $0.upperBound.isFinite && $0.lowerBound < $0.upperBound ? $0 : nil
        }
    }

    /// Bins the finite samples. Non-finite values never reach a bin.
    public func build(_ values: [Float], color: SemanticColorRef = .accent,
                      title: String = "") -> Result {
        let samples = values.filter(\.isFinite)
        let domain = range ?? Self.fitted(samples)
        let count = max(1, binCount)
        let width = (domain.upperBound - domain.lowerBound) / Float(count)
        var counts = [Int](repeating: 0, count: count)
        for value in samples where value >= domain.lowerBound && value <= domain.upperBound {
            let position = (value - domain.lowerBound) / width
            let index = min(count - 1, Int(position.rounded(.down)))
            counts[max(0, index)] += 1
        }
        let decimals = Self.decimals(width)
        let labels = (0..<count).map { index -> String in
            let lower = domain.lowerBound + Float(index) * width
            return "\(ChartScale.fixed(lower, decimals: decimals))–\(ChartScale.fixed(lower + width, decimals: decimals))"
        }
        return Result(series: ChartSeries(counts.map { Float($0) }, color: color, mark: .bar, title: title),
                      labels: labels, counts: counts)
    }

    /// The samples' own span. Identical samples still need a width to divide.
    private static func fitted(_ samples: [Float]) -> ClosedRange<Float> {
        guard let low = samples.min(), let high = samples.max() else { return 0...1 }
        guard high > low else { return (low - 0.5)...(low + 0.5) }
        return low...high
    }

    /// The first precision in 0...3 that states the bin width without rounding.
    private static func decimals(_ width: Float) -> Int {
        guard width.isFinite, width > 0 else { return 0 }
        for places in 0...3 {
            let scaled = width * powf(10, Float(places))
            if abs(scaled - scaled.rounded()) < 0.001 * max(1, abs(scaled)) { return places }
        }
        return 3
    }
}
