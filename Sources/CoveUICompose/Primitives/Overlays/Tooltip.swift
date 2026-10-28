import Foundation
import CoveUIRuntime

public struct TooltipOptions {
    public var delay: Double = 0.45
    public var maxWidth: Float = 320
    public var gap: Float = 6
    public var isEnabled = true
    /// Labels that are fully visible repeat what the user already reads. Set
    /// by `Text.truncated()` so the description appears only when the node
    /// actually clipped its text.
    public var onlyWhenTruncated = false
    public init() {}
    mutating func validate() {
        delay = delay.isFinite ? max(0, delay) : 0.45
        maxWidth = maxWidth.isFinite ? max(80, maxWidth) : 320
        gap = gap.isFinite ? max(0, gap) : 6
    }
}

/// Window coordinates match TextFieldHoverAnchor and overlay placement.
public enum TooltipAnchor: Equatable {
    case point(CGPoint)
    case range(CGRect)
    var rect: CGRect {
        switch self { case .point(let point): CGRect(origin: point, size: .zero); case .range(let rect): rect }
    }
}

/// An arbitrary target and noninteractive description, presented outside clipping.
public struct Tooltip<Target: View, Content: View>: View {
    private let target: Target
    private let content: Content
    private var coordinateAnchor: TooltipAnchor?
    private var isPresented = true
    public var options = TooltipOptions()
    public init(configure: (inout TooltipOptions) -> Void = { _ in }, @ViewBuilder target: () -> Target,
                @ViewBuilder content: () -> Content) {
        self.target = target(); self.content = content(); configure(&options); options.validate()
    }
    /// Explicitly anchored tooltips are already debounced by their producer
    /// (for example LSP hover), so presentation follows isPresented directly.
    public init(anchor: TooltipAnchor, isPresented: Bool = true,
                configure: (inout TooltipOptions) -> Void = { _ in },
                @ViewBuilder content: () -> Content) where Target == EmptyView {
        self.target = EmptyView(); self.content = content()
        coordinateAnchor = anchor; self.isPresented = isPresented
        configure(&options); options.validate()
    }
    public var body: AnyView {
        if let coordinateAnchor {
            return AnyView(CoordinateTooltipHost(anchor: coordinateAnchor, content: AnyView(content),
                                                  options: options, isPresented: isPresented))
        }
        return AnyView(TooltipHost(target: target, content: AnyView(content), options: options,
                    focused: FocusChainHolder.current?.currentFocus,
                    focusVisible: FocusChainHolder.current?.isFocusVisible ?? false))
    }
}

private struct CoordinateTooltipHost: _PrimitiveView {
    private static var availabilityKey: String { "CoveUI.tooltip.coordinate.available" }
    let anchor: TooltipAnchor
    let content: AnyView
    let options: TooltipOptions
    let isPresented: Bool
    func _makeNode() -> Node {
        let node = Node(); node.isHitTestable = false; node.addResource(PortalResource())
        node.attachments["tooltip.coordinate"] = true
        return node
    }
    func _makeLayoutNode() -> LayoutNode? { nil }
    func _updateNode(_ node: Node) {
        node.attachments[Self.availabilityKey] = node.acceptsSubtreeInput
        node.layoutDidUpdate = { node in
            let available = node.acceptsSubtreeInput
            guard node.attachments[Self.availabilityKey] as? Bool != available else { return }
            node.attachments[Self.availabilityKey] = available
            updatePresentation(node)
        }
        updatePresentation(node)
    }
    private func updatePresentation(_ node: Node) {
        guard let resource = node.firstResource(PortalResource.self) else { return }
        guard node.acceptsSubtreeInput, isPresented, options.isEnabled else { resource.unmount(node: node); return }
        let store = node.compositionValue(of: PortalStoreEnvironment.key) ?? PortalStoreHolder.current
        resource.present(in: store, position: CGPoint(x: anchor.rect.minX, y: anchor.rect.maxY + CGFloat(options.gap)),
                         width: nil, content: AnyView(content.frame(maxWidth: options.maxWidth).theme(node.theme).allowsHitTesting(false)))
        resource.setDismissal(anchor: { anchor.rect }, dismiss: {})
    }
}

public extension View {
    func tooltip(_ description: String, configure: (inout TooltipOptions) -> Void = { _ in }) -> some View {
        tooltip(description, onlyWhenTruncated: false, configure: configure)
    }

    /// A description that is redundant unless the target had to clip text.
    func tooltip(_ description: String, onlyWhenTruncated: Bool,
                 configure: (inout TooltipOptions) -> Void = { _ in }) -> some View {
        Tooltip(configure: { options in
            options.onlyWhenTruncated = onlyWhenTruncated
            configure(&options)
        }, target: { self }) {
            tooltipDescription(description)
        }
    }
}

func tooltipDescription(_ description: String) -> AnyView {
    // Inverted: the primary text colour becomes the fill and the window
    // background colour becomes the label, so it reads the same on both
    // themes — a dark chip in light mode, a light chip in dark mode.
    AnyView(Text(description)
        .font(Font.system(size: 11))
        .foregroundColor(.background)
        .padding(horizontal: 8, vertical: 5)
        .background(.onSurface)
        .cornerRadius(5)
        .shadow(color: Color(r: 0, g: 0, b: 0, a: 0.2), offsetX: 0, offsetY: 2, blur: 8))
}

private struct TooltipHost<Target: View>: _PrimitiveView {
    let target: Target
    let content: AnyView
    let options: TooltipOptions
    let focused: Node?
    let focusVisible: Bool
    func _makeNode() -> Node {
        let node = Node(); node.isHitTestable = true; node.addResource(PortalResource()); node.addResource(TooltipSession())
        node.attachments["tooltip.host"] = true
        return node
    }
    func _makeLayoutNode() -> LayoutNode? { let layout = LayoutNode(); layout.alignItems = .stretch; return layout }
    func _updateNode(_ node: Node) {
        let session = node.firstResource(TooltipSession.self)!
        session.configure(content: content, options: options,
                          focused: focusVisible && focused.map { FocusChainHolder.current?.contains($0, in: node) == true } == true)
        if !node.supportsControlHover { session.setHovered(false) }
        InteractionRegistryHolder.current?.setHover(node) { phase in
            session.setHovered(phase == .enter && node.supportsControlHover)
        }
        InteractionRegistryHolder.current?.setPointer(node) { _, phase, _ in
            if phase == .down { session.dismiss() }; return .ignored
        }
        InteractionRegistryHolder.current?.setKey(node) { event, _ in
            if event.scancode == Scancode.escape, session.dismiss() { return .handled }; return .ignored
        }
        node.layoutDidUpdate = { _ in session.updatePosition() }
    }
    var _children: [any View] { [target] }
}

final class TooltipSession: NodeResource, AnyAnimationController {
    private weak var node: Node?
    private var content: AnyView?
    private var options = TooltipOptions()
    private var hovered = false
    private var focused = false
    private var elapsed: Double = 0
    private var presented = false
    private var dismissedUntilLeave = false
    /// Re-evaluated before the tip appears, not just when the pointer enters:
    /// hover can begin while a control's label is still being laid out, and a
    /// description that repeats text drawn a frame later must not survive.
    private var canShow: () -> Bool = { true }
    var isFinished = true
    /// Whether a tip is on screen; exposed for tooltip regression tests.
    var isPresented: Bool { presented }
    func mount(node: Node) { self.node = node }
    func unmount(node: Node) { hide(); self.node = nil }
    func configure(content: AnyView, options: TooltipOptions, focused: Bool,
                   canShow: @escaping () -> Bool = { true }) {
        self.content = content; self.options = options; self.focused = focused
        self.canShow = canShow
        refresh()
    }
    func setHovered(_ hovered: Bool) { self.hovered = hovered; refresh() }
    func setFocused(_ focused: Bool) { self.focused = focused; refresh() }
    private func refresh() {
        let showsText = !options.onlyWhenTruncated || showsTruncatedText
        guard options.isEnabled && (hovered || focused) && showsText, canShow() else { dismissedUntilLeave = false; hide(); return }
        guard !dismissedUntilLeave else { return }
        if presented { show(); return }
        guard isFinished else { return }
        elapsed = 0
        if options.delay == 0 { show() }
        else { isFinished = false; AnimatorScheduler.current.register(self) }
    }

    /// A truncated label marks its node while drawing. Text that fits keeps
    /// the tooltip hidden, so hovering ordinary labels stays quiet.
    private var showsTruncatedText: Bool {
        guard let node else { return false }
        return node.containsTruncatedText
    }
    func tick(deltaTime: Double) {
        guard !isFinished else { return }
        elapsed += max(0, deltaTime)
        if elapsed >= options.delay {
            isFinished = true
            if canShow() { show() } else { hide() }
        }
    }
    private func show() {
        guard let node, node.acceptsSubtreeInput, let content, canShow() else { return }
        presented = true
        node.firstResource(PortalResource.self)?.present(in: node.compositionValue(of: PortalStoreEnvironment.key) ?? PortalStoreHolder.current,
            position: position(node), width: nil,
            content: AnyView(content.frame(maxWidth: options.maxWidth).theme(node.theme).allowsHitTesting(false)))
        node.firstResource(PortalResource.self)?.setDismissal(anchor: { [weak node] in node?.absoluteFrame ?? .zero }, dismiss: { [weak self] in self?.dismiss() })
    }
    private func position(_ node: Node) -> CGPoint {
        CGPoint(x: node.absoluteFrame.minX, y: node.absoluteFrame.maxY + CGFloat(options.gap))
    }
    func updatePosition() {
        if let node, !node.acceptsSubtreeInput { dismiss(); return }
        guard presented, let node, let resource = node.firstResource(PortalResource.self), let id = resource.entryID else { return }
        let store = node.compositionValue(of: PortalStoreEnvironment.key) ?? PortalStoreHolder.current
        store.updatePosition(id, position: position(node))
    }
    @discardableResult
    func dismiss() -> Bool { let wasPresented = presented; dismissedUntilLeave = true; hide(); return wasPresented }
    func hide() {
        isFinished = true; presented = false
        if let node { node.firstResource(PortalResource.self)?.unmount(node: node) }
    }
    func cancel() { hide() }
    func finishImmediately() { hide() }
}
