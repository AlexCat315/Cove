import Foundation
import CoveUIScene
import CoveUIText
import CoveUIImaging
#if canImport(CoreGraphics)
import CoreGraphics
#endif

/// A button that opens a menu anchored to itself. Uses the regular `Popover`
/// trigger contract, so the menu escapes clipping, closes on Escape/outside
/// click and exposes expanded state to accessibility.
public struct MenuButton<Label: View>: View {
    public let entries: [MenuEntry]
    public let isEnabled: Bool
    public let width: Float?
    public let placement: PopoverPlacement
    public let label: Label
    public let accessibilityLabel: String?
    public let tooltip: String?

    @State private var presented = false

    public init(entries: [MenuEntry],
                isEnabled: Bool = true,
                width: Float? = nil,
                placement: PopoverPlacement = .bottomLeading,
                accessibilityLabel: String? = nil,
                tooltip: String? = nil,
                @ViewBuilder label: () -> Label) {
        self.entries = entries
        self.isEnabled = isEnabled
        self.width = width
        self.placement = placement
        self.accessibilityLabel = accessibilityLabel
        self.tooltip = tooltip
        self.label = label()
    }

    public var body: some View {
        SurfaceReader { surface in
            if NativeMenuPresenter.present != nil, !TouchPresentation.usesSheet(surface) {
                nativeButton
            } else {
                popoverButton
            }
        }
    }

    /// A button that opens the system menu anchored at its own corner.
    @ViewBuilder private var nativeButton: some View {
        let frame = FrameBox()
        let button = Button(role: .normal, isEnabled: isEnabled, tooltip: tooltip, action: {
            guard let rect = frame.current else { return }
            let built = NativeMenuBuilder.build(entries)
            if let chosen = NativeMenuPresenter.present?(built.snapshot, Self.anchor(rect, placement: placement)) {
                built.run(id: chosen)
            }
        }) { label }
            .buttonStyle(AnyButtonStyle(SecondaryButtonStyle()))
            .captureFrame(frame)
        if let accessibilityLabel {
            button.compositionLocal(ActionControlAccessibility.label, accessibilityLabel)
        } else {
            button
        }
    }

    private var popoverButton: some View {
        var popover = Popover(isPresented: $presented,
                              isEnabled: isEnabled,
                              width: width,
                              placement: placement,
                              surface: .content, overflow: .flip, triggerStyle: AnyButtonStyle(SecondaryButtonStyle()),
                              label: { label }) {
            Menu(entries, width: width, onItemActivated: { presented = false })
        }
        if let accessibilityLabel {
            popover.triggerDescription = PopoverTriggerDescription(label: accessibilityLabel, help: tooltip)
        }
        return popover
    }

    /// The anchor point for a menu at a button's corner, matching the reference
    /// framework: bottom-left (or bottom-right in RTL), top corners for top
    /// placements.
    private static func anchor(_ frame: CGRect, placement: PopoverPlacement) -> CGPoint {
        switch placement {
        case .topLeading, .top, .leadingTop:
            return CGPoint(x: frame.minX, y: frame.minY)
        case .topTrailing, .trailingTop:
            return CGPoint(x: frame.maxX, y: frame.minY)
        case .leading, .leadingBottom, .bottomLeading, .bottom:
            return CGPoint(x: frame.minX, y: frame.maxY)
        case .trailing, .trailingBottom, .bottomTrailing:
            return CGPoint(x: frame.maxX, y: frame.maxY)
        }
    }
}

public extension MenuButton where Label == AnyView {
    init(_ title: String,
         entries: [MenuEntry],
         isEnabled: Bool = true,
         width: Float? = nil,
         placement: PopoverPlacement = .bottomLeading,
         tooltip: String? = nil) {
        self.init(entries: entries, isEnabled: isEnabled, width: width, placement: placement,
                  accessibilityLabel: title, tooltip: tooltip) {
            // mygo's menu button draws its label and the dropdown arrow together,
            // so a text-only MenuButton still reads as a menu trigger.
            AnyView(Row(alignment: .center, spacing: 6) {
                Text(title).truncatingTail()
                Icon(UIIcons.Navigation.dropdownChevronDown, size: .small)
            })
        }
    }
}
