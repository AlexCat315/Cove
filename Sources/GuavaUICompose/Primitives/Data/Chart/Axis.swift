internal import Foundation
import GuavaUIRuntime

/// Caption text the axis chrome is measured with and drawn in.
struct ChartText {
    let token: TextStyleToken
    let environment: TextEnvironment?

    init(theme: Theme, environment: TextEnvironment?) {
        token = theme.typography.caption
        self.environment = environment
    }

    func size(_ text: String) -> (width: Float, height: Float) {
        guard !text.isEmpty else { return (0, 0) }
        if let layout = layout(text) { return (layout.totalWidth, layout.totalHeight) }
        // No text environment (headless tests): a caption glyph averages ~0.55em.
        return (Float(text.count) * token.font.size * 0.55, token.lineHeight)
    }

    func layout(_ text: String) -> TextLayoutResult? {
        guard let environment, !text.isEmpty else { return nil }
        return environment.cachedLayout(text: text, font: token.font,
                                        lineHeight: token.lineHeight,
                                        letterSpacing: token.letterSpacing)
    }
}

/// Where the plot sits inside a chart and the tick labels around it.
///
/// The chart has to know the label widths before it can size the plot, so this
/// is resolved from the data-derived scale rather than from the final frame:
/// domain → ticks → label widths → plot.
struct ChartFrame {
    struct Tick {
        /// y for the value axis, x for the category axis.
        let position: Float
        let label: String
    }

    /// The chart box after `contentInset`, before the axis gutters.
    let bounds: UIRect
    let plot: UIRect
    let yTicks: [Tick]
    /// Ticks for the right-hand axis; empty when the chart has none.
    let secondaryTicks: [Tick]
    let xTicks: [Tick]
    let yTitle: String
    let secondaryTitle: String
    let xTitle: String

    /// Space between a tick label and the plot edge.
    static let labelGap: Float = 6

    static func cartesian(axes: ChartValueAxes, xDomain: ChartXDomain, xScale: ChartScale?,
                          style: ChartStyle, axis: ChartConfiguration.Axis,
                          size: CGSize, origin: CGPoint, text: ChartText) -> ChartFrame {
        let bounds = inset(size: size, origin: origin, style: style)
        let primaryLabels = labels(axes.primaryScale, labeled: axis.y.isLabeled,
                                   format: axis.y.format, percent: axes.primary.isPercent)
        let left = axis.y.isLabeled
            ? (primaryLabels.map { text.size($0).width }.max() ?? 0) + labelGap
            : 0
        // The right gutter only exists when a series reads that axis.
        let hasSecondary = axes.usesSecondary && axis.secondary.isLabeled
        let secondaryLabels = hasSecondary
            ? labels(axes.scale(.right), labeled: true, format: axis.secondary.format,
                     percent: axes.secondary?.isPercent ?? false)
            : []
        let right = hasSecondary
            ? (secondaryLabels.map { text.size($0).width }.max() ?? 0) + labelGap
            : 0
        let lineHeight = text.size("0").height
        let primaryTitleHeight = axis.y.title.isEmpty ? 0 : text.size(axis.y.title).height
        let secondaryTitleHeight = hasSecondary && !axis.secondary.title.isEmpty
            ? text.size(axis.secondary.title).height
            : 0
        let titleHeight = max(primaryTitleHeight, secondaryTitleHeight)
        // The topmost label is centred on its grid line, so the box keeps half a
        // line above the plot for it; a title stacks on top of that.
        let labelPad = axis.y.isLabeled || hasSecondary ? lineHeight / 2 : 0
        let top = titleHeight > 0 ? titleHeight + labelPad + 2 : labelPad

        let hasXLabels = axis.x.isLabeled && xLabelsExist(xDomain, count: axes.sampleCount, scale: xScale)
        let bottom = (hasXLabels ? lineHeight + 4 : 0)
            + (axis.x.title.isEmpty ? 0 : text.size(axis.x.title).height + 2)
        let leftInset = min(left, bounds.width * 0.4)
        let rightInset = min(right, bounds.width * 0.3)
        let topInset = min(top, bounds.height * 0.3)

        let plot = UIRect(x: bounds.minX + leftInset,
                          y: bounds.minY + topInset,
                          width: max(0, bounds.width - leftInset - rightInset),
                          height: max(0, bounds.height - topInset - min(bottom, bounds.height * 0.4)))
        let yTicks = zip(axes.primaryScale.ticks, primaryLabels).map { value, label in
            Tick(position: axes.primaryScale.y(value, in: plot), label: label)
        }
        let secondaryTicks = axes.secondaryScale.map { scale in
            zip(scale.ticks, secondaryLabels).map { value, label in
                Tick(position: scale.y(value, in: plot), label: label)
            }
        } ?? []
        var xTicks: [Tick] = []
        if axis.x.isLabeled {
            switch xDomain {
            case .categorical(let labels):
                xTicks = labelIndices(labels, count: axes.sampleCount, width: plot.width, text: text).map { index in
                    Tick(position: ChartGeometry.x(index: index, count: axes.sampleCount,
                                                   hasBars: axes.hasBars, plot: plot),
                         label: labels[index])
                }
            case .numeric:
                if let xScale {
                    xTicks = xScale.ticks.map { value in
                        Tick(position: xScale.x(value, in: plot), label: axis.x.format?(value) ?? xScale.label(value))
                    }
                }
            }
        }
        return ChartFrame(bounds: bounds, plot: plot, yTicks: yTicks, secondaryTicks: secondaryTicks,
                          xTicks: xTicks, yTitle: axis.y.title,
                          secondaryTitle: hasSecondary ? axis.secondary.title : "", xTitle: axis.x.title)
    }

    /// Tick labels for one axis. A percent axis reads as shares even though the
    /// plotted values are the normalised ones.
    private static func labels(_ scale: ChartScale, labeled: Bool, format: ((Float) -> String)?,
                               percent: Bool) -> [String] {
        guard labeled else { return scale.ticks.map { _ in "" } }
        return scale.ticks.map { value in
            if let format { return format(value) }
            let text = scale.label(value)
            return percent ? text + "%" : text
        }
    }

    /// Whether an x axis has anything to label, and so needs the bottom gutter.
    private static func xLabelsExist(_ domain: ChartXDomain, count: Int, scale: ChartScale?) -> Bool {
        switch domain {
        case .categorical(let labels): return !labels.isEmpty && count > 0
        case .numeric: return !(scale?.ticks.isEmpty ?? true)
        }
    }

    /// A pie or donut has no axes: the plot fills the inset box.
    static func pie(style: ChartStyle, size: CGSize, origin: CGPoint) -> ChartFrame {
        let bounds = inset(size: size, origin: origin, style: style)
        return ChartFrame(bounds: bounds, plot: bounds, yTicks: [], secondaryTicks: [], xTicks: [],
                          yTitle: "", secondaryTitle: "", xTitle: "")
    }

    /// Indices whose labels fit side by side; every other one is dropped so
    /// dense category axes stay readable.
    private static func labelIndices(_ labels: [String], count: Int, width: Float,
                                     text: ChartText) -> [Int] {
        let usable = min(labels.count, count)
        guard usable > 0, width > 0 else { return [] }
        let slot = width / Float(max(1, count))
        let widest = (0..<usable).map { text.size(labels[$0]).width }.max() ?? 0
        let step = max(1, Int(((widest + 8) / max(slot, 1)).rounded(.up)))
        return Array(stride(from: 0, to: usable, by: step))
    }

    private static func inset(size: CGSize, origin: CGPoint, style: ChartStyle) -> UIRect {
        let width = Float(size.width), height = Float(size.height)
        let inset = min(style.contentInset, min(width, height) * 0.35)
        return UIRect(x: Float(origin.x) + inset, y: Float(origin.y) + inset,
                      width: max(0, width - inset * 2), height: max(0, height - inset * 2))
    }
}
