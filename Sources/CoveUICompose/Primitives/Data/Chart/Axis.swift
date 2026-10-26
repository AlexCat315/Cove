internal import Foundation
import CoveUIRuntime

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
///
/// Ticks are named for the axis they belong to, not the edge they sit on: a
/// horizontal chart draws `valueTicks` along the bottom and `categoryTicks`
/// down the left, and nothing else in the renderer has to care which is which.
struct ChartFrame {
    struct Tick {
        /// Screen coordinate along the axis this tick belongs to.
        let position: Float
        let label: String
    }

    /// The chart box after `contentInset`, before the axis gutters.
    let bounds: UIRect
    let plot: UIRect
    let valueTicks: [Tick]
    /// Ticks for the second value axis; empty when the chart has none.
    let secondaryTicks: [Tick]
    let categoryTicks: [Tick]
    let valueTitle: String
    let secondaryTitle: String
    let categoryTitle: String
    let orientation: ChartOrientation

    /// Space between a tick label and the plot edge.
    static let labelGap: Float = 6

    static func cartesian(axes: ChartValueAxes, xDomain: ChartXDomain, xScale: ChartScale?,
                          style: ChartStyle, axis: ChartConfiguration.Axis,
                          size: CGSize, origin: CGPoint, text: ChartText, window: ChartWindow? = nil) -> ChartFrame {
        let bounds = inset(size: size, origin: origin, style: style)
        let orientation = style.orientation
        let valueLabels = labels(axes.primaryScale, labeled: axis.y.isLabeled,
                                 format: axis.y.format, percent: axes.primary.isPercent)
        // The second value axis only exists when a series reads it.
        let hasSecondary = axes.usesSecondary && axis.secondary.isLabeled
        let secondaryLabels = hasSecondary
            ? labels(axes.scale(.right), labeled: true, format: axis.secondary.format,
                     percent: axes.secondary?.isPercent ?? false)
            : []
        let categoryTexts = axis.x.isLabeled
            ? categoryTexts(xDomain, count: axes.sampleCount, scale: xScale, axis: axis)
            : []
        let hasCategories = !categoryTexts.isEmpty

        let lineHeight = text.size("0").height
        let valueWidth = valueLabels.map { text.size($0).width }.max() ?? 0
        let secondaryWidth = secondaryLabels.map { text.size($0).width }.max() ?? 0
        let categoryWidth = categoryTexts.map { text.size($0).width }.max() ?? 0
        let valueTitleHeight = axis.y.title.isEmpty ? 0 : text.size(axis.y.title).height
        let secondaryTitleHeight = hasSecondary && !axis.secondary.title.isEmpty
            ? text.size(axis.secondary.title).height
            : 0
        let categoryTitleHeight = hasCategories && !axis.x.title.isEmpty
            ? text.size(axis.x.title).height
            : 0

        // Each gutter is reserved only where something is drawn in it. The
        // extreme label is centred on its grid line, so the box keeps half a
        // line beyond the plot for it; a title stacks on top of that.
        var left: Float = 0, right: Float = 0, top: Float = 0, bottom: Float = 0
        switch orientation {
        case .vertical:
            left = axis.y.isLabeled ? valueWidth + labelGap : 0
            right = hasSecondary ? secondaryWidth + labelGap : 0
            let pad = axis.y.isLabeled || hasSecondary ? lineHeight / 2 : 0
            let title = max(valueTitleHeight, secondaryTitleHeight)
            top = title > 0 ? title + pad + 2 : pad
            bottom = (hasCategories ? lineHeight + 4 : 0)
                + (categoryTitleHeight > 0 ? categoryTitleHeight + 2 : 0)
        case .horizontal:
            // Categories run down the left; values along the bottom, with the
            // second value axis and the category title sharing the top edge.
            left = hasCategories ? categoryWidth + labelGap : 0
            bottom = (axis.y.isLabeled ? lineHeight + 4 : 0)
                + (valueTitleHeight > 0 ? valueTitleHeight + 2 : 0)
                + (axis.y.isLabeled ? lineHeight / 2 : 0)
            let title = max(categoryTitleHeight, secondaryTitleHeight)
            top = (hasSecondary ? lineHeight / 2 + lineHeight + labelGap : 0)
                + (title > 0 ? title + 2 : 0)
        }
        let leftInset = min(left, bounds.width * 0.4)
        let rightInset = min(right, bounds.width * 0.3)
        let topInset = min(top, bounds.height * 0.35)
        let bottomInset = min(bottom, bounds.height * 0.4)

        let plot = UIRect(x: bounds.minX + leftInset,
                          y: bounds.minY + topInset,
                          width: max(0, bounds.width - leftInset - rightInset),
                          height: max(0, bounds.height - topInset - bottomInset))
        let valueTicks = zip(axes.primaryScale.ticks, valueLabels).map { value, label in
            Tick(position: orientation.along(axes.primaryScale, value, in: plot), label: label)
        }
        let secondaryTicks = axes.secondaryScale.map { scale in
            zip(scale.ticks, secondaryLabels).map { value, label in
                Tick(position: orientation.along(scale, value, in: plot), label: label)
            }
        } ?? []
        return ChartFrame(bounds: bounds, plot: plot, valueTicks: valueTicks,
                          secondaryTicks: secondaryTicks,
                          categoryTicks: hasCategories
                            ? ticks(categoryTexts, xDomain: xDomain, xScale: xScale,
                                    count: axes.sampleCount, hasBars: axes.hasBars,
                                    plot: plot, orientation: orientation, text: text, window: window)
                            : [],
                          valueTitle: axis.y.title,
                          secondaryTitle: hasSecondary ? axis.secondary.title : "",
                          categoryTitle: hasCategories ? axis.x.title : "",
                          orientation: orientation)
    }

    /// Every label the independent axis would show, before thinning.
    private static func categoryTexts(_ domain: ChartXDomain, count: Int, scale: ChartScale?,
                                      axis: ChartConfiguration.Axis) -> [String] {
        switch domain {
        case .categorical(let labels):
            return Array(labels.prefix(max(0, count)))
        case .numeric:
            guard let scale else { return [] }
            return scale.ticks.map { axis.x.format?($0) ?? scale.label($0) }
        }
    }

    /// Ticks on the independent axis, thinned to what the plot can hold apart.
    private static func ticks(_ texts: [String], xDomain: ChartXDomain, xScale: ChartScale?,
                              count: Int, hasBars: Bool, plot: UIRect,
                              orientation: ChartOrientation, text: ChartText, window: ChartWindow?) -> [Tick] {
        guard !texts.isEmpty else { return [] }
        if xDomain.isNumeric, let xScale {
            // A numeric axis ticks on round values, so it never needs thinning.
            return zip(texts, xScale.ticks).map { label, value in
                Tick(position: orientation.across(xScale, value, in: plot), label: label)
            }
        }
        let span = window.map { $0.range.upperBound - $0.range.lowerBound } ?? Float(max(1, count))
        let step = labelStep(texts, slot: orientation.breadth(plot) / max(1, span),
                             orientation: orientation, text: text)
        return stride(from: 0, to: texts.count, by: step).compactMap { index in
            let position = window?.position(Float(index), plot: plot, orientation: orientation)
                ?? ChartGeometry.across(index: index, count: count, hasBars: hasBars, plot: plot, orientation: orientation)
            guard position >= orientation.acrossOrigin(plot),
                  position <= orientation.acrossOrigin(plot) + orientation.breadth(plot) else { return nil }
            return Tick(position: position, label: texts[index])
        }
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

    /// A pie or donut has no axes: the plot fills the inset box.
    static func pie(style: ChartStyle, size: CGSize, origin: CGPoint) -> ChartFrame {
        let bounds = inset(size: size, origin: origin, style: style)
        return ChartFrame(bounds: bounds, plot: bounds, valueTicks: [], secondaryTicks: [],
                          categoryTicks: [], valueTitle: "", secondaryTitle: "", categoryTitle: "",
                          orientation: style.orientation)
    }

    /// The stride that keeps labels apart: side by side they need the widest
    /// label's width, stacked they need a line's height.
    private static func labelStep(_ texts: [String], slot: Float,
                                  orientation: ChartOrientation, text: ChartText) -> Int {
        guard slot > 0 else { return max(1, texts.count) }
        let needed = orientation == .vertical
            ? (texts.map { text.size($0).width }.max() ?? 0) + 8
            : text.size("0").height + 2
        return max(1, Int((needed / slot).rounded(.up)))
    }

    private static func inset(size: CGSize, origin: CGPoint, style: ChartStyle) -> UIRect {
        let width = Float(size.width), height = Float(size.height)
        let inset = min(style.contentInset, min(width, height) * 0.35)
        return UIRect(x: Float(origin.x) + inset, y: Float(origin.y) + inset,
                      width: max(0, width - inset * 2), height: max(0, height - inset * 2))
    }
}
