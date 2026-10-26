import Foundation
import CoveUIRuntime

/// Session-owned page stack. The caller owns `path`; pushing appends a route,
/// popping removes the last one, and a non-empty initial path restores a deep
/// link on the first frame.
///
/// The root and every route page stay mounted while they are in the path, so a
/// push keeps the root's drafts, scroll and local state. Only the top page is
/// interactive and painted; the incoming page slides from the trailing edge
/// with a fade, and the outgoing page slides to the leading edge. Popping
/// reveals the retained previous page with the reverse motion. Reduced motion
/// is handled by `AnimatedVisibility`.
///
/// On touch surfaces a drag from the leading edge pulls the top page aside over
/// the previous one and pops it when released past a third of the width or
/// flicked. Android back is a host concern: the host should remove the last
/// route. The component deliberately does not consume Escape; overlays keep
/// priority over page navigation.
public struct NavigationStack<Route: Hashable, Root: View, Destination: View>: View {
    public let path: Binding<[Route]>
    private let root: () -> Root
    private let destination: (Route) -> Destination
    /// Horizontal travel of the page transition in logical points.
    public let transitionDistance: Float

    @State private var lastCount: Int
    @State private var lastRoutes: [Route]
    @State private var isAnimating = false
    @State private var swipe: Float = 0
    @State private var isSwiping = false

    public init(path: Binding<[Route]>,
                transitionDistance: Float = 32,
                @ViewBuilder root: @escaping () -> Root,
                @ViewBuilder destination: @escaping (Route) -> Destination) {
        self.path = path
        self.transitionDistance = transitionDistance.isFinite ? max(0, transitionDistance) : 32
        self.root = root
        self.destination = destination
        _lastCount = State(wrappedValue: path.wrappedValue.count)
        _lastRoutes = State(wrappedValue: path.wrappedValue)
    }

    private var routes: [Route] { path.wrappedValue }

    public var body: some View {
        ContainerSizeReader { size in
            ThemeReader { theme in
                ResolvedLayoutDirection { direction in
                    stack(size: size, theme: theme, sign: direction == .rtl ? -1 : 1)
                }
            }
        }
    }

    private func stack(size: CGSize, theme: Theme, sign: Float) -> some View {
        let routes = self.routes
        let forward = routes.count >= lastCount
        let width = Float(size.width), height = Float(size.height)
        return NavigationEdgeSwipe(isEnabled: !routes.isEmpty, sign: sign, width: width,
                                   onChange: { distance in
            if !isSwiping { isSwiping = true }
            swipe = min(width, distance)
        }, onEnd: { commit in
            if commit, !path.wrappedValue.isEmpty {
                // The revealed page is already in place, so the pop has no
                // transition to wait for.
                var next = path.wrappedValue; next.removeLast()
                lastRoutes = next; lastCount = next.count
                swipe = 0; isSwiping = false
                path.wrappedValue = next
            } else {
                withAnimation(.semantic(.fast, in: theme)) { swipe = 0 }
                isSwiping = false
            }
        }) {
            ZStack(alignment: .topLeading) {
                _NavigationPathWatcher(count: routes.count) { count, animates in
                    guard count != lastCount else { return }
                    if isAnimating {
                        if path.wrappedValue != lastRoutes {
                            path.wrappedValue = lastRoutes
                        }
                        return
                    }
                    lastRoutes = path.wrappedValue
                    lastCount = count
                    isAnimating = animates
                }
                AnyView(page(route: nil, index: 0, depth: routes.count, sign: sign,
                             forward: forward, theme: theme, width: width, height: height))
                for (offset, route) in routes.enumerated() {
                    AnyView(page(route: route, index: offset + 1, depth: routes.count - offset - 1, sign: sign,
                                 forward: forward, theme: theme, width: width, height: height))
                }
            }
        }
    }

    /// `depth` counts the pages above this one. The page right under the top
    /// shows, parallaxed, while an edge swipe pulls the top page aside.
    private func page(route: Route?, index: Int, depth: Int, sign: Float, forward: Bool, theme: Theme,
                      width: Float, height: Float) -> some View {
        let isTop = depth == 0
        let revealed = isSwiping && depth == 1
        let transition = revealed ? Transition.identity
            : Transition.navigation(distance: transitionDistance, forward: forward)
        let shift = isTop ? swipe : revealed ? (swipe - width) * 0.3 : 0
        return AnimatedVisibility(isVisible: isTop || revealed, transition: transition,
                                  animateOnMount: index > 0, retainsContent: true,
                                  onVisibilitySettled: { visible in
            if visible, isAnimating { isAnimating = false }
        }) {
            _NavigationPageFocusHost(isTop: isTop,
                content: pageContent(route: route, theme: theme, width: width, height: height))
        }
        .offset(x: shift * sign)
        .id(route.map { AnyHashable($0) } ?? AnyHashable("navigation.root"))
    }

    private func pageContent(route: Route?, theme: Theme, width: Float, height: Float) -> some View {
        let content: AnyView
        if let route {
            content = AnyView(destination(route))
        } else {
            content = AnyView(root())
        }
        return content
            .frame(width: .points(max(1, width)), height: .points(max(1, height)))
            .background(theme.colors.background)
    }
}

/// Tracks a touch that starts at the leading edge. Down is only observed, so
/// taps still reach the page; the drag is claimed once it moves inward more
/// than it moves vertically.
private struct NavigationEdgeSwipe<Content: View>: _PrimitiveView {
    static var edgeWidth: CGFloat { 24 }
    let isEnabled: Bool
    let sign: Float
    let width: Float
    let onChange: (Float) -> Void
    let onEnd: (Bool) -> Void
    let content: Content

    init(isEnabled: Bool, sign: Float, width: Float, onChange: @escaping (Float) -> Void,
         onEnd: @escaping (Bool) -> Void, @ViewBuilder content: () -> Content) {
        self.isEnabled = isEnabled; self.sign = sign; self.width = width
        self.onChange = onChange; self.onEnd = onEnd; self.content = content()
    }

    final class Track {
        let start: CGPoint
        var sample: (x: CGFloat, time: Double)
        var velocity: CGFloat = 0
        var dragging = false
        init(_ start: CGPoint) { self.start = start; sample = (start.x, ProcessInfo.processInfo.systemUptime) }
        /// Inward speed over windows of at least 50 ms, so events delivered in
        /// one burst do not read as a flick, and a pause before release does.
        func record(_ x: CGFloat, sign: Float) {
            let now = ProcessInfo.processInfo.systemUptime, elapsed = now - sample.time
            if elapsed > 0.15 { velocity = 0; sample = (x, now) }
            else if elapsed >= 0.05 { velocity = (x - sample.x) * CGFloat(sign) / CGFloat(elapsed); sample = (x, now) }
        }
    }

    // Hit-testable so the edge also catches touches over page areas that are not.
    func _makeNode() -> Node { let node = Node(); node.isHitTestable = true; return node }
    func _makeLayoutNode() -> LayoutNode? { let layout = LayoutNode(); layout.alignItems = .stretch; return layout }
    func _updateLayout(_ layout: LayoutNode) { layout.flexDirection = .column; layout.alignItems = .stretch }

    func _updateNode(_ node: Node) {
        guard let registry = InteractionRegistryHolder.current else { return }
        let route = InputHandlerRoute(role: .scrollChrome, priority: .capture, debugName: "navigation.edge")
        let key = "navigation.edge.track"
        if !isEnabled, let track = node.attachments.removeValue(forKey: key) as? Track, track.dragging {
            if PointerCaptureHolder.current?.target === node { PointerCaptureHolder.current?.release() }
            onEnd(false)
        }
        registry.setPointer(node, route: route) { event, phase, eventPhase in
            guard event.button == .left else { return .ignored }
            switch phase {
            case .down:
                guard eventPhase == .capture else { return .ignored }
                node.attachments[key] = nil
                guard isEnabled, node.compositionValue(of: SurfaceEnvironment.key).input.contains(.touch) else { return .ignored }
                let local = node.convertFromWindow(CGPoint(x: CGFloat(event.x), y: CGFloat(event.y)))
                let inset = sign < 0 ? node.frame.width - local.x : local.x
                guard inset >= 0, inset <= Self.edgeWidth else { return .ignored }
                node.attachments[key] = Track(local)
                return .ignored
            case .up:
                guard let track = node.attachments.removeValue(forKey: key) as? Track, track.dragging else { return .ignored }
                if PointerCaptureHolder.current?.target === node { PointerCaptureHolder.current?.release() }
                let local = node.convertFromWindow(CGPoint(x: CGFloat(event.x), y: CGFloat(event.y)))
                let travel = (local.x - track.start.x) * CGFloat(sign)
                track.record(local.x, sign: sign)
                onEnd(travel > CGFloat(width) / 3 || (travel > 0 && track.velocity > 600))
                return .handled
            }
        }
        registry.setMotion(node, route: route) { event, _ in
            guard let track = node.attachments[key] as? Track else { return .ignored }
            let local = node.convertFromWindow(CGPoint(x: CGFloat(event.x), y: CGFloat(event.y)))
            let dx = (local.x - track.start.x) * CGFloat(sign), dy = local.y - track.start.y
            if !track.dragging {
                if abs(dy) > 10 && abs(dy) > dx { node.attachments[key] = nil; return .ignored }
                guard dx >= 8, dx > abs(dy) * 1.5 else { return .ignored }
                track.dragging = true
                PointerCaptureHolder.current?.acquire(node)
            }
            track.record(local.x, sign: sign)
            onChange(Float(max(0, dx)))
            return .handled
        }
    }

    var _children: [any View] { [content] }
}

/// Calls back once per path-count change, with whether a page transition will
/// run. Reduced motion settles pages without one, so nothing would end it. The
/// node has zero size and does not participate in hit testing; its attachment
/// stores the last observed count so the first mount is not reported as a
/// navigation.
private struct _NavigationPathWatcher: _PrimitiveView {
    let count: Int
    let onChange: (Int, Bool) -> Void

    func _makeNode() -> Node {
        let node = Node()
        node.isHitTestable = false
        return node
    }

    func _makeLayoutNode() -> LayoutNode? {
        let layout = LayoutNode()
        layout.width = 0
        layout.height = 0
        return layout
    }

    func _updateNode(_ node: Node) {
        let previous = node.attachments["navigation.path.count"] as? Int
        node.attachments["navigation.path.count"] = count
        if let previous, previous != count { onChange(count, !node.prefersReducedMotion) }
    }
}


/// Marks a page title as the focus target for a NavigationStack push/pop.
/// The stack focuses it without a visible keyboard ring once the new page has
/// laid out; ordinary keyboard traversal continues from there.
public extension View {
    func navigationTitle(_ title: String) -> some View {
        accessibility {
            $0.role = .heading
            $0.label = title
            $0.identifier = "navigation.title"
        }
    }
}

private struct _NavigationPageFocusHost<Content: View>: _PrimitiveView {
    let isTop: Bool
    let content: Content

    func _makeNode() -> Node {
        let node = Node()
        node.isHitTestable = false
        return node
    }

    func _makeLayoutNode() -> LayoutNode? {
        let layout = LayoutNode()
        layout.alignItems = .stretch
        return layout
    }

    func _updateNode(_ node: Node) {
        let wasTop = node.attachments["navigation.page.isTop"] as? Bool ?? false
        node.attachments["navigation.page.isTop"] = isTop
        if !isTop {
            node.attachments["navigation.page.titleFocused"] = false
            return
        }
        guard !wasTop else { return }
        node.layoutDidUpdate = { node in
            guard node.attachments["navigation.page.isTop"] as? Bool == true,
                  node.attachments["navigation.page.titleFocused"] as? Bool != true,
                  let title = findTitle(in: node),
                  let chain = FocusChainHolder.current else { return }
            node.attachments["navigation.page.titleFocused"] = true
            chain.focus(title, visible: false)
        }
    }

    private func findTitle(in node: Node) -> Node? {
        if node.accessibility?.identifier == "navigation.title" { return node }
        if node.accessibility?.role == .heading { return node }
        for child in node.children {
            if let found = findTitle(in: child) { return found }
        }
        return nil
    }

    var _children: [any View] { [content] }
}
