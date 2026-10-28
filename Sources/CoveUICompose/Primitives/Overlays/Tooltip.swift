import Foundation
import CoveUIRuntime

public struct TooltipOptions {
    /// mygo's rest time before a tip appears; moving along a toolbar skips it.
    public var delay: Double = 0.6
    public var maxWidth: Float = 320
    public var gap: Float = 6
    public var isEnabled = true
    /// Labels that are fully visible repeat what the user already reads. Set
    /// by `Text.truncated()` so the description appears only when the node
    /// actually clipped its text.
    public var onlyWhenTruncated = false
    public init() {}
    mutating func validate() {
        delay = delay.isFinite ? max(0, delay) : 0.6
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

/// Window-wide tooltip rules, matching mygo's engine: one tip at a time, the
/// innermost target wins, and a tip that follows another within the warm
/// window skips the rest delay so the pointer can travel along a toolbar.
///
/// Owned by the window's `PlatformInputContext`, like the portal store (规则 3):
/// two windows must not take tips from each other, and a fresh context starts
/// with none on screen. The holder is empty until a window registers one, so a
/// bare tree (previews, tests that do not care) simply has no arbitration and
/// every tip times out on its own.
///
/// Main-thread-only, like the portal store: tooltips are presented and
/// dismissed on the UI thread.
public final class TooltipArbitration {
    private final class WeakSession {
        weak var session: TooltipSession?
        init(_ session: TooltipSession) { self.session = session }
    }

    /// A tip that appeared this soon before the next target shows at once.
    static let warmWindow: TimeInterval = 0.4
    private var sessions: [WeakSession] = []
    private var lastShown: TimeInterval = -.greatestFiniteMagnitude
    private var isNotifying = false

    public init() {}

    func register(_ session: TooltipSession) {
        prune()
        if !sessions.contains(where: { $0.session === session }) {
            sessions.append(WeakSession(session))
        }
    }

    func unregister(_ session: TooltipSession) {
        sessions.removeAll { $0.session == nil || $0.session === session }
    }

    /// A tip went up within the warm window, so the next one needs no rest.
    func isWarm() -> Bool {
        prune()
        return Date.timeIntervalSinceReferenceDate - lastShown <= Self.warmWindow
    }

    func noteShown() { lastShown = Date.timeIntervalSinceReferenceDate }

    /// An enabled target inside this session's element is waiting for its own
    /// tip, so this one must stay quiet: over nested targets the inner wins.
    func hasInnerActive(than session: TooltipSession) -> Bool {
        prune()
        guard let node = session.anchorNode else { return false }
        return sessions.contains { entry in
            guard let other = entry.session, other !== session, other.isActive,
                  let otherNode = other.anchorNode else { return false }
            return Self.isDescendant(otherNode, of: node)
        }
    }

    /// A new tip takes the screen from any other that is still up.
    func hideOthers(besides session: TooltipSession) {
        for entry in sessions {
            guard let other = entry.session, other !== session, other.isPresented else { continue }
            other.hide()
        }
    }

    /// After a tip goes, a target that is still hovered may now own the screen.
    func reconsider(besides session: TooltipSession) {
        guard !isNotifying else { return }
        isNotifying = true
        defer { isNotifying = false }
        for entry in sessions where entry.session !== session {
            entry.session?.reconsider()
        }
    }

    func reset() {
        sessions.removeAll()
        lastShown = -.greatestFiniteMagnitude
        isNotifying = false
    }

    private func prune() {
        sessions.removeAll { $0.session == nil }
    }

    private static func isDescendant(_ node: Node, of ancestor: Node) -> Bool {
        var parent = node.parent
        while let candidate = parent {
            if candidate === ancestor { return true }
            parent = candidate.parent
        }
        return false
    }
}

/// The active window's tooltip rules; `nil` until a window registers one.
public enum TooltipArbitrationHolder {
    nonisolated(unsafe) public static var current: TooltipArbitration?
}

/// Registers a window's tooltip rules for as long as its input context is
/// current, mirroring `PortalStoreAmbient`.
public final class TooltipArbitrationAmbient: ScopedAmbient {
    private let arbitration: TooltipArbitration
    public init(_ arbitration: TooltipArbitration = TooltipArbitration()) {
        self.arbitration = arbitration
    }
    public func activate() -> () -> Void {
        let previous = TooltipArbitrationHolder.current
        TooltipArbitrationHolder.current = arbitration
        return { TooltipArbitrationHolder.current = previous }
    }
}

final class TooltipSession: NodeResource, AnyAnimationController {
    private weak var node: Node?
    private var content: AnyView?
    private var options = TooltipOptions()
    private var hovered = false
    private var focused = false
    private var elapsed: Double = 0
    /// When the pointer arrived: a target that waited behind a nested one
    /// still measures its rest from here, so it opens as soon as it wins.
    private var hoverSince: TimeInterval = 0
    private var presented = false
    private var dismissedUntilLeave = false
    /// The window that mounted this tip. `unmount` must leave *that* window's
    /// list even if the ambient has already moved on to another one.
    private weak var owner: TooltipArbitration?
    private var arbitration: TooltipArbitration? { owner ?? TooltipArbitrationHolder.current }
    /// Re-evaluated before the tip appears, not just when the pointer enters:
    /// hover can begin while a control's label is still being laid out, and a
    /// description that repeats text drawn a frame later must not survive.
    private var canShow: () -> Bool = { true }
    var isFinished = true
    /// Whether a tip is on screen; exposed for tooltip regression tests.
    var isPresented: Bool { presented }
    /// Target node; other tips arbitrate against its place in the tree.
    var anchorNode: Node? { node }
    /// Hovered (or keyboard-focused) with something worth showing.
    var isActive: Bool {
        options.isEnabled && (hovered || focused)
            && (!options.onlyWhenTruncated || showsTruncatedText) && canShow()
    }
    func mount(node: Node) {
        self.node = node
        owner = TooltipArbitrationHolder.current
        arbitration?.register(self)
    }
    func unmount(node: Node) {
        hide()
        arbitration?.unregister(self)
        owner = nil
        self.node = nil
    }
    func configure(content: AnyView, options: TooltipOptions, focused: Bool,
                   canShow: @escaping () -> Bool = { true }) {
        self.content = content; self.options = options; self.focused = focused
        self.canShow = canShow
        refresh()
    }
    func setHovered(_ hovered: Bool) {
        if hovered, !self.hovered { hoverSince = Date.timeIntervalSinceReferenceDate }
        self.hovered = hovered
        refresh()
    }
    func setFocused(_ focused: Bool) { self.focused = focused; refresh() }
    private func refresh() {
        let showsText = !options.onlyWhenTruncated || showsTruncatedText
        guard options.isEnabled && (hovered || focused) && showsText, canShow() else { dismissedUntilLeave = false; hide(); return }
        guard !dismissedUntilLeave else { return }
        // Over elements inside one another only the inner tip shows.
        guard arbitration?.hasInnerActive(than: self) != true else { hide(); return }
        if presented { show(); return }
        guard isFinished else { return }
        elapsed = hovered ? max(0, Date.timeIntervalSinceReferenceDate - hoverSince) : 0
        // A tip that follows another within the warm window skips the rest, so
        // a run along a toolbar does not wait again. Keyboard focus does *not*
        // skip it: focus lands on a trigger the instant a menu closes, and a
        // chip covering the bar the user just used reads as an accident.
        if options.delay == 0 || arbitration?.isWarm() == true || elapsed >= options.delay {
            show()
        } else {
            isFinished = false
            AnimatorScheduler.current.register(self)
        }
    }

    /// A tip that went may let a still-hovered target show without moving the
    /// pointer onto it again (the inner tip of nested targets just closed).
    func reconsider() {
        guard !presented else { return }
        refresh()
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
        guard arbitration?.hasInnerActive(than: self) != true else { return }
        presented = true
        // One tip at a time: anything already on screen gives way to this one.
        arbitration?.hideOthers(besides: self)
        arbitration?.noteShown()
        let anchor = node.absoluteFrame
        let gap = CGFloat(options.gap)
        let window = portalWindowBounds(node)
        node.firstResource(PortalResource.self)?.present(in: node.compositionValue(of: PortalStoreEnvironment.key) ?? PortalStoreHolder.current,
            position: Self.restingSpot(size: .zero, anchor: anchor, gap: gap, window: window), width: nil,
            content: AnyView(content.frame(maxWidth: options.maxWidth).theme(node.theme).allowsHitTesting(false)),
            placement: .fixed,
            positionForSize: { size in Self.restingSpot(size: size, anchor: anchor, gap: gap, window: window) })
        node.firstResource(PortalResource.self)?.setDismissal(anchor: { [weak node] in node?.absoluteFrame ?? .zero }, dismiss: { [weak self] in self?.dismiss() })
    }
    /// Centered above the anchor, like mygo. The chip's own size decides where
    /// it rests, and it only drops below when the top of the window would clip.
    private static func restingSpot(size: CGSize, anchor: CGRect, gap: CGFloat, window: CGRect) -> CGPoint {
        let above = anchor.minY - gap - size.height
        let below = anchor.maxY + gap
        let y = above >= window.minY ? above : (below + size.height <= window.maxY ? below : above)
        return CGPoint(x: anchor.midX - size.width / 2, y: y)
    }
    private func position(_ node: Node) -> CGPoint {
        Self.restingSpot(size: .zero, anchor: node.absoluteFrame, gap: CGFloat(options.gap), window: portalWindowBounds(node))
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
        let wasPresented = presented
        isFinished = true; presented = false
        if let node { node.firstResource(PortalResource.self)?.unmount(node: node) }
        // A target the tip was covering (a nested one that just closed) may now
        // own the screen without the pointer moving onto it again.
        if wasPresented { arbitration?.reconsider(besides: self) }
    }
    func cancel() { hide() }
    func finishImmediately() { hide() }
}
