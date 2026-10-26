internal import Foundation
import CoveUIRuntime

/// The text of one item drawn over the plot: what it says, not where it goes.
///
/// Content is deliberately independent of the node's size, so the label set can
/// be materialised before layout runs. Where each label lands is a
/// `ChartOverlayPlacement`, resolved during layout when the plot rect is known.
struct ChartOverlayItem {
    enum Kind {
        /// The multi-row tooltip for the selected point.
        case callout
        /// A pinned note on one value.
        case annotation
        /// The label at the end of a reference rule.
        case threshold
        /// The label at the top-left of a shaded corridor.
        case band
        /// A value written next to its mark.
        case value
        /// A slice's title and share, outside the ring.
        case slice
        /// Text in the donut hole.
        case center
    }

    let kind: Kind
    let title: String
    let rows: [ChartCallout.Row]
    let tint: SemanticColorRef?
    /// A second, muted line under the title.
    let subtitle: String

    init(kind: Kind, title: String, rows: [ChartCallout.Row] = [], tint: SemanticColorRef? = nil,
         subtitle: String = "") {
        self.kind = kind
        self.title = title
        self.rows = rows
        self.tint = tint
        self.subtitle = subtitle
    }

    /// A callout reads as a card; everything else is caption text.
    var isCard: Bool { kind == .callout }
}

/// Where one overlay item attaches: the anchor point, plus which fraction of
/// the item's own size sits before it. `vertical: 1` puts its bottom edge on the
/// anchor, `0` its top edge, `0.5` its middle.
struct ChartOverlayPlacement {
    let anchor: CGPoint
    let horizontal: CGFloat
    let vertical: CGFloat
    /// Space kept from the anchor.
    let gap: CGFloat
    /// Moves to the other side of the anchor when there is no room.
    let flips: Bool

    init(_ anchor: CGPoint, horizontal: CGFloat, vertical: CGFloat, gap: CGFloat = 0, flips: Bool = false) {
        self.anchor = anchor
        self.horizontal = horizontal
        self.vertical = vertical
        self.gap = gap
        self.flips = flips
    }

    /// The rect the item occupies at a measured size, before bounds clamping.
    func box(width: CGFloat, height: CGFloat) -> CGRect {
        CGRect(x: anchor.x - width * horizontal, y: originY(height: height, vertical: vertical),
               width: width, height: height)
    }

    /// The same box mirrored about the anchor.
    func flippedY(height: CGFloat) -> CGFloat {
        vertical >= 1 ? anchor.y + gap : anchor.y - gap - height
    }

    private func originY(height: CGFloat, vertical: CGFloat) -> CGFloat {
        if vertical >= 1 { return anchor.y - gap - height }
        if vertical <= 0 { return anchor.y + gap }
        return anchor.y - height / 2
    }
}

/// One overlay label and, once the size is known, where it goes. A missing
/// placement means the label is dropped: it would overlap one already placed.
struct ChartOverlaySlot {
    let item: ChartOverlayItem
    let placement: ChartOverlayPlacement?
}

/// Places every overlay item against its own anchor, keeps it inside the chart,
/// flips it to the other side of the anchor when it does not fit, and hides the
/// labels the plot dropped.
///
/// Placement is resolved here rather than at materialisation because only the
/// layout pass knows the plot rect: children are built before layout runs.
struct ChartOverlayLayout: Layout {
    let plot: ChartPlot
    let theme: Theme
    let environment: TextEnvironment?
    /// Shared with the draw pass, so placing labels reuses the geometry it
    /// already built instead of resolving the chart a third time.
    let cache: ChartResolutionCache?

    func sizeThatFits(proposal: ProposedViewSize, subviews: [LayoutSubview]) -> CGSize {
        CGSize(width: proposal.width ?? 0, height: proposal.height ?? 0)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: [LayoutSubview]) {
        let text = ChartText(theme: theme, environment: environment)
        let slots = plot.overlaySlots(in: bounds.size, text: text, cache: cache)
        for (index, subview) in subviews.enumerated() {
            guard slots.indices.contains(index), let placement = slots[index].placement else {
                // Retained but not painted, hit-tested or announced.
                subview.place(at: bounds.origin, proposal: .init(width: 0, height: 0), isVisible: false)
                continue
            }
            let size = subview.sizeThatFits(.unspecified)
            var rect = placement.box(width: size.width, height: size.height)
            rect.origin.x += bounds.minX
            rect.origin.y += bounds.minY
            if placement.flips, rect.minY < bounds.minY || rect.maxY > bounds.maxY {
                rect.origin.y = bounds.minY + placement.flippedY(height: size.height)
            }
            rect.origin.x = max(bounds.minX, min(rect.minX, bounds.maxX - size.width))
            rect.origin.y = max(bounds.minY, min(rect.minY, bounds.maxY - size.height))
            subview.place(at: rect.origin, proposal: .init(width: size.width, height: size.height))
        }
    }
}

/// The text of one overlay item. Every kind shares one structure — a title, an
/// optional subtitle and optional rows — so the overlay can lay them out
/// uniformly and only the styling differs.
struct ChartOverlayLabel: View {
    let item: ChartOverlayItem

    var body: some View {
        if item.isCard {
            callout
        } else if item.kind == .annotation {
            annotation
        } else if item.kind == .center {
            center
        } else {
            caption
        }
    }

    private var callout: some View {
        Box(direction: .column, alignItems: .flexStart, spacing: 2) {
            Text(item.title).lineLimit(1).font(.caption).foregroundColor(.onSurfaceMuted)
            ChartCalloutRows(rows: item.rows)
        }
        .padding(horizontal: 8, vertical: 5)
        .background(.surfaceFloating).cornerRadius(.inputRadius).border(.border, width: 1).surfaceFinish()
        .debugName("chart-callout")
    }

    private var annotation: some View {
        Text(item.title).lineLimit(1).font(.caption).foregroundColor(.onSurface)
            .padding(horizontal: 5, vertical: 2)
            .background(.surfaceFloating).cornerRadius(4)
            .border(item.tint ?? .accent, width: 1)
            .debugName("chart-annotation")
    }

    private var center: some View {
        Box(direction: .column, alignItems: .center, spacing: 0) {
            Text(item.title).lineLimit(1).font(.headline).foregroundColor(.onSurface)
            subtitle
        }
        .debugName("chart-center")
    }

    private var caption: some View {
        Box(direction: .column, alignItems: .center, spacing: 0) {
            Text(item.title).lineLimit(1).font(.caption).foregroundColor(.onSurfaceVariant)
            subtitle
        }
        .debugName("chart-label")
    }

    @ViewBuilder private var subtitle: some View {
        if !item.subtitle.isEmpty {
            Text(item.subtitle).lineLimit(1).font(.caption).foregroundColor(.onSurfaceMuted)
        }
    }
}

/// One row of a multi-series callout: the series swatch, its name and value.
private struct ChartCalloutRows: View {
    let rows: [ChartCallout.Row]

    var body: some View {
        Box(direction: .column, alignItems: .flexStart, spacing: 2) {
            for row in rows {
                ChartCalloutRow(row: row)
            }
        }
    }
}

private struct ChartCalloutRow: View {
    let row: ChartCallout.Row

    var body: some View {
        Row(alignment: .center, spacing: 5) {
            Box {}.frame(width: 6, height: 6)
                .background(row.color).cornerRadius(3).accessibilityHidden()
            Text(row.title).lineLimit(1).font(.caption).foregroundColor(.onSurfaceMuted)
            Text(row.value).lineLimit(1).font(.bodyStrong).foregroundColor(.onSurface)
        }
    }
}
