internal import Foundation
import CoveUIScene
import CoveUIText
import CoveUIImaging

public struct ListRowOptions {
    public var subtitle: String?
    public var role: ButtonRole = .normal
    public var showsDisclosure = false
    public init() {}
}

/// Title and fixed slots. Touch rows stay within their proposal; desktop rows
/// retain horizontal overflow for dense, independently authored accessory slots.
public struct ListRow<Leading: View, Trailing: View>: View {
    private let title: String
    private let leading: Leading
    private let trailing: Trailing
    private var options = ListRowOptions()

    public init(_ title: String, configure: (inout ListRowOptions) -> Void = { _ in },
                @ViewBuilder leading: () -> Leading, @ViewBuilder trailing: () -> Trailing) {
        self.title = title; self.leading = leading(); self.trailing = trailing(); configure(&options)
    }

    public var body: some View {
        SurfaceReader { surface in
            TextScalerReader { scaler in
                let touch = surface.input.contains(.touch)
                var scroll = ScrollView(.horizontal) {
                    contents(minimumTitleWidth: CGFloat(scaler.scale(24)), allowsOverflow: !touch)
                }
                scroll.combinesAccessibility = true
                scroll.isScrollEnabled = !touch
                // A touch row is a finger-sized target that spans its container.
                return scroll.frame(width: touch ? .percent(100) : .auto, minWidth: 0)
            }
        }
    }

    private var trailingContent: AnyView {
        if options.showsDisclosure {
            return AnyView(Row(alignment: .center, spacing: 8) {
                trailing.flex(0, shrink: 0)
                ListDisclosure().accessibilityHidden()
            })
        }
        return AnyView(Box(direction: .row, alignItems: .center, spacing: 0) { trailing })
    }

    private var titleContent: AnyView {
        if let subtitle = options.subtitle {
            return AnyView(Column(alignment: .leading, spacing: 4) {
                Text(title).truncated()
                Text(subtitle).truncatingTail().font(.caption).foregroundColor(.onSurfaceMuted)
            }.foregroundColor(options.role == .destructive ? .error : .onSurface))
        }
        return AnyView(Text(title).truncated().foregroundColor(options.role == .destructive ? .error : .onSurface))
    }

    private func contents(minimumTitleWidth: CGFloat, allowsOverflow: Bool) -> some View {
        ListRowLayout(minimumTitleWidth: minimumTitleWidth, allowsOverflow: allowsOverflow) {
            Box(direction: .row, alignItems: .center, spacing: 0) { leading }
            titleContent
            trailingContent
        }.frame(minHeight: allowsOverflow ? 0 : 44).accessibility { $0.combinesChildren = true }
    }
}

public extension ListRow where Leading == EmptyView {
    init(_ title: String, configure: (inout ListRowOptions) -> Void = { _ in }, @ViewBuilder trailing: () -> Trailing) {
        self.init(title, configure: configure, leading: { EmptyView() }, trailing: trailing)
    }
}

public extension ListRow where Leading == EmptyView, Trailing == EmptyView {
    init(_ title: String, configure: (inout ListRowOptions) -> Void = { _ in }) {
        self.init(title, configure: configure, leading: { EmptyView() }, trailing: { EmptyView() })
    }
}

private struct ListDisclosure: View {
    var body: some View {
        ResolvedLayoutDirection { direction in
            Icon(direction == .rtl ? UIIcons.Navigation.chevronLeft : UIIcons.Navigation.chevronRight,
                 size: .small, color: .onSurfaceMuted)
        }
    }
}

private struct ListRowLayout: Layout {
    let minimumTitleWidth: CGFloat
    let allowsOverflow: Bool
    private let spacing: CGFloat = 8

    func sizeThatFits(proposal: ProposedViewSize, subviews: [LayoutSubview]) -> CGSize {
        plan(subviews, width: proposal.width).size
    }
    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: [LayoutSubview]) {
        guard subviews.count == 3 else { return }
        let plan = plan(subviews, width: bounds.width), rtl = subviews[0].layoutDirection == .rtl
        var x = rtl ? bounds.maxX : bounds.minX
        for index in subviews.indices {
            let size = plan.slots[index]
            // Fixed slots keep the intrinsic proposal used for measurement.
            // Reapplying Yoga's rounded width as an at-most constraint can
            // wrap a badge's last glyph and change its height during placement.
            let slotProposal: ProposedViewSize = index == 1 || plan.constrainsAccessories ? .init(width: size.width, height: size.height) : .unspecified
            subviews[index].place(at: .init(x: rtl ? x - size.width : x, y: bounds.minY + (bounds.height - size.height) / 2),
                                 proposal: slotProposal)
            let gap = index == 0 ? (size.width > 0 ? spacing : 0) : (index == 1 && plan.slots[2].width > 0 ? spacing : 0)
            x += (rtl ? -1 : 1) * (size.width + gap)
        }
    }
    /// `width` is the viewport; nil (unbounded) gives the title its natural width.
    private func plan(_ subviews: [LayoutSubview], width: CGFloat?) -> Plan {
        guard subviews.count == 3 else { return Plan(slots: [], size: .zero, constrainsAccessories: false) }
        var leading = subviews[0].sizeThatFits(.unspecified)
        var trailing = subviews[2].sizeThatFits(.unspecified)
        let gaps = (leading.width > 0 ? spacing : 0) + (trailing.width > 0 ? spacing : 0)
        let natural = leading.width + trailing.width + gaps + subviews[1].sizeThatFits(.unspecified).width
        let proposed = width.flatMap { $0.isFinite ? max(0, $0) : nil } ?? natural
        let rowWidth: CGFloat
        var constrainsAccessories = false
        if allowsOverflow { rowWidth = max(proposed, leading.width + trailing.width + gaps + minimumTitleWidth) }
        else {
            rowWidth = proposed
            let fixedBudget = max(0, rowWidth - min(minimumTitleWidth, rowWidth) - gaps)
            let total = leading.width + trailing.width
            if total > fixedBudget, total > 0 {
                constrainsAccessories = true
                let leadingBudget = fixedBudget * leading.width / total
                let trailingBudget = fixedBudget - leadingBudget
                leading = subviews[0].sizeThatFits(.init(width: leadingBudget))
                trailing = subviews[2].sizeThatFits(.init(width: trailingBudget))
                leading.width = min(leading.width, leadingBudget)
                trailing.width = min(trailing.width, trailingBudget)
            }
        }
        let titleWidth = max(0, rowWidth - leading.width - trailing.width - gaps)
        let title = subviews[1].sizeThatFits(.init(width: titleWidth))
        return Plan(slots: [leading, CGSize(width: titleWidth, height: title.height), trailing],
                    size: CGSize(width: rowWidth, height: max(leading.height, title.height, trailing.height)),
                    constrainsAccessories: constrainsAccessories)
    }

    private struct Plan {
        let slots: [CGSize]
        let size: CGSize
        let constrainsAccessories: Bool
    }
}
