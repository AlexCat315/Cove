internal import Foundation
import GuavaUIRuntime

/// Reference geometry drawn across or behind the series: rules, corridors,
/// pinned notes and column markers. They share one value so a chart's content
/// reads as data plus its annotations.
struct ChartGuides {
    var thresholds: [ChartThreshold] = []
    var bands: [ChartBand] = []
    var annotations: [ChartAnnotation] = []
    var markers: [ChartMarker] = []
}

/// Cartesian series or pie slices, with per-item visibility from the legend.
enum ChartContent {
    case cartesian(series: [ChartSeries], guides: ChartGuides, x: ChartXDomain)
    case pie([ChartSlice])

    var count: Int {
        switch self {
        case .cartesian(let series, _, _): series.count
        case .pie(let slices): slices.count
        }
    }

    func title(_ index: Int) -> String {
        switch self {
        case .cartesian(let series, _, _): series.indices.contains(index) ? series[index].title : ""
        case .pie(let slices): slices.indices.contains(index) ? slices[index].title : ""
        }
    }
}

/// Selection writes coming from input; the chart owns the binding.
struct ChartPlotInput {
    /// Interactive only when the caller supplied a selection binding.
    let isEnabled: Bool
    let select: (ChartPoint?) -> Void
}

/// A chart resolved against one node size: the frame the axis chrome leaves
/// behind, plus the marks drawn inside it.
struct ChartResolution {
    let frame: ChartFrame
    let cartesian: ChartGeometry?
    let pie: PieGeometry?
}

/// The drawing and input surface of `Chart`. Hover previews a point on
/// pointer surfaces; press/drag scrubs on touch; arrows step; Escape clears.
struct ChartPlot: _PrimitiveView {
    let content: ChartContent
    let visible: [Bool]
    let style: ChartStyle
    let axis: ChartConfiguration.Axis
    let pie: ChartConfiguration.Pie
    /// Formats a value for a data label and for a slice's share.
    let format: (Float) -> String
    let selection: ChartPoint?
    let summary: (label: String, value: String)
    /// The callout for any point, so the accessibility cursor can read a value
    /// the pointer never visited.
    var callout: (ChartPoint) -> ChartCallout? = { _ in nil }
    var accessibility = ChartConfiguration.Accessibility()
    let input: ChartPlotInput

    func _makeNode() -> Node {
        let node = Node(), session = ChartSession()
        node.attachments[ChartSession.key] = session
        node.addResource(session)
        return node
    }

    func _makeLayoutNode() -> LayoutNode? { LayoutNode() }

    func _updateLayout(_ layout: LayoutNode) {
        if (layout.height ?? 0) <= 0 { layout.height = 64 }
        layout.minHeight = 24
        layout.minWidth = 0
    }

    func _updateNode(_ node: Node) {
        let session = node.attachments[ChartSession.key] as! ChartSession
        let snapshot = self
        // The session holds the live selection: several events can arrive
        // before the chart recomposes with the binding's new value.
        session.current = selection
        // A bound selection *is* the reading cursor. Without a binding the
        // cursor only moves under assistive control, so an unrelated
        // recomposition must not wipe it back to the summary.
        if input.isEnabled { session.reading = selection }
        let choose: (ChartPoint?) -> Void = { next in
            session.current = next
            snapshot.input.select(next)
        }
        node.isHitTestable = input.isEnabled
        node.isFocusable = input.isEnabled
        node.cursor = input.isEnabled ? .crosshair : .arrow
        publishAccessibility(node: node, session: session)
        let theme = node.theme
        let identity = ChartPaintIdentity(plot: self, theme: theme)
        // Only the data and the text metrics move a mark, so a pointer move —
        // which changes nothing but the selection — keeps the cached geometry.
        if session.signature != identity.geometry {
            session.signature = identity.geometry
            session.cache.invalidate()
        }
        node.updateDraw(identity: identity) { [weak node] list, origin in
            guard let node else { return }
            snapshot.draw(node: node, origin: origin, list: list)
        }
        guard input.isEnabled, let registry = InteractionRegistryHolder.current else {
            InteractionRegistryHolder.current?.remove(node)
            return
        }
        registry.setHover(node) { [weak node] phase in
            guard let node, phase == .leave, !session.isPinned, !session.isPressed, node.supportsControlHover else { return }
            choose(nil)
        }
        registry.setMotion(node) { [weak node] event, phase in
            guard let node, phase == .target else { return .ignored }
            guard session.isPressed || (node.supportsControlHover && !session.isPinned) else { return .ignored }
            let hit = snapshot.hit(x: event.x, y: event.y, node: node)
            // A drag scrubs; leaving the plot keeps the last value.
            if hit != nil || !session.isPressed, hit != session.current { choose(hit) }
            return session.isPressed ? .handled : .ignored
        }
        registry.setPointer(node) { [weak node] event, phase, route in
            guard let node, event.button == .left, route == .target else { return .ignored }
            switch phase {
            case .down:
                let hit = snapshot.hit(x: event.x, y: event.y, node: node)
                // Pressing the pinned point again releases it.
                session.releasesPin = session.isPinned && hit != nil && hit == session.current
                session.press(node: node)
                choose(session.releasesPin ? nil : hit)
            case .up:
                session.release(node: node)
                session.isPinned = !session.releasesPin && session.current != nil
                session.releasesPin = false
            }
            return .handled
        }
        registry.setKey(node) { event, phase in
            guard phase == .target, event.modifiers.isDisjoint(with: [.ctrl, .gui, .alt]) else { return .ignored }
            let current = session.current
            switch event.scancode {
            case Scancode.arrowRight: choose(snapshot.stepped(index: 1, from: current))
            case Scancode.arrowLeft: choose(snapshot.stepped(index: -1, from: current))
            case Scancode.arrowDown: choose(snapshot.stepped(series: 1, from: current))
            case Scancode.arrowUp: choose(snapshot.stepped(series: -1, from: current))
            case Scancode.home: choose(snapshot.jumped(toEnd: false, from: current))
            case Scancode.end: choose(snapshot.jumped(toEnd: true, from: current))
            case Scancode.escape:
                guard current != nil else { return .ignored }
                session.isPinned = false
                session.reading = nil
                choose(nil)
                return .handled
            default: return .ignored
            }
            // A keyboard selection stays until Escape or a pointer moves it.
            session.isPinned = session.current != nil
            return .handled
        }
    }

    /// Text drawn over the plot: the callout, guide labels, pinned notes, value
    /// labels and pie labels. Text is content, so it goes in the view tree
    /// rather than the draw list — the overlay only carries placement.
    ///
    /// Children are built before layout runs, so this only decides *what* the
    /// labels say. `ChartOverlayLayout` resolves where they go, once the plot
    /// rect exists.
    func _children(for node: Node) -> [any View] {
        let environment = TextEnvironmentHolder.current
        let text = ChartText(theme: node.theme, environment: environment)
        let cache = session(node)?.cache
        let slots = overlaySlots(in: nil, text: text, cache: cache)
        guard !slots.isEmpty else { return [] }
        return [
            ChartOverlayLayout(plot: self, theme: node.theme, environment: environment, cache: cache) {
                for slot in slots {
                    ChartOverlayLabel(item: slot.item)
                }
            }
            .frame(width: .percent(100), height: .percent(100))
            .absolutePosition(left: 0, top: 0)
            .allowsHitTesting(false)
            .accessibilityHidden()
        ]
    }

    /// Every overlay label for one size, in back-to-front order.
    ///
    /// A nil `size` means the plot rect is not known yet: the labels are
    /// returned in the same order, all without placement. The order is stable
    /// across both passes, so slot *i* always belongs to the same label.
    func overlaySlots(in size: CGSize?, text: ChartText,
                      cache: ChartResolutionCache? = nil) -> [ChartOverlaySlot] {
        let resolution = size.map { resolve(in: $0, text: text, cache: cache) }
        var slots: [ChartOverlaySlot] = []
        var taken: [CGRect] = []

        /// Records one label. `reserving` labels drop out when they would
        /// overlap an earlier one, so a dense series stays readable instead of
        /// piling text on itself.
        func add(_ item: ChartOverlayItem, _ placement: ChartOverlayPlacement?, reserving: Bool = true) {
            guard let placement else {
                slots.append(ChartOverlaySlot(item: item, placement: nil))
                return
            }
            if reserving {
                let measured = text.size(item.title.isEmpty ? item.subtitle : item.title)
                let box = placement.box(width: CGFloat(measured.width), height: CGFloat(measured.height))
                guard !taken.contains(where: { $0.intersects(box.insetBy(dx: -2, dy: -2)) }) else {
                    slots.append(ChartOverlaySlot(item: item, placement: nil))
                    return
                }
                taken.append(box)
            }
            slots.append(ChartOverlaySlot(item: item, placement: placement))
        }

        if case .cartesian(let series, let guides, _) = content {
            let geometry = resolution?.cartesian
            for band in guides.bands where !band.label.isEmpty && band.from.isFinite && band.to.isFinite {
                add(ChartOverlayItem(kind: .band, title: band.label),
                    geometry.map { g in
                        ChartOverlayPlacement(point(g.plot.minX + 4, g.y(band.to, axis: band.axis)),
                                              horizontal: 0, vertical: 0, gap: 2)
                    })
            }
            for threshold in guides.thresholds where !threshold.label.isEmpty && threshold.value.isFinite {
                add(ChartOverlayItem(kind: .threshold, title: threshold.label),
                    geometry.map { g in
                        ChartOverlayPlacement(point(g.plot.maxX - 2, g.y(threshold.value, axis: threshold.axis)),
                                              horizontal: 1, vertical: 1, gap: 2)
                    })
            }
            for note in guides.annotations {
                // The note exists as soon as its value does; where it sits
                // waits for the plot rect.
                guard series.indices.contains(note.series),
                      series[note.series].data.indices.contains(note.index),
                      series[note.series].data[note.index].y.isFinite else { continue }
                var placement: ChartOverlayPlacement?
                if let mark = geometry?.mark(ChartPoint(series: note.series, index: note.index)),
                   mark.y.isFinite {
                    placement = ChartOverlayPlacement(point(mark.x, mark.bar.map(\.minY) ?? mark.y),
                                                      horizontal: 0.5, vertical: 1, gap: 5, flips: true)
                }
                add(ChartOverlayItem(kind: .annotation, title: note.text, tint: note.color), placement)
            }
            for index in series.indices where series[index].style.showsValues {
                for i in series[index].data.indices {
                    let datum = series[index].data[i]
                    guard datum.y.isFinite else { continue }
                    var placement: ChartOverlayPlacement?
                    if let mark = geometry?.mark(ChartPoint(series: index, index: i)), mark.y.isFinite {
                        // A bar below the baseline is labelled underneath.
                        let below = mark.y > mark.baseY
                        let top = mark.bar.map { below ? $0.maxY : $0.minY } ?? mark.y
                        placement = ChartOverlayPlacement(point(mark.x, top), horizontal: 0.5,
                                                          vertical: below ? 0 : 1, gap: 3, flips: true)
                    }
                    add(ChartOverlayItem(kind: .value, title: format(datum.y)), placement)
                }
            }
        }

        if case .pie(let slices) = content {
            let pie = resolution?.pie
            if let center = pieConfig?.center {
                add(ChartOverlayItem(kind: .center, title: center.value, subtitle: center.label),
                    pie.flatMap { g in
                        // A full pie has no hole to write in.
                        g.innerRadius > 4
                            ? ChartOverlayPlacement(point(g.center.x, g.center.y), horizontal: 0.5, vertical: 0.5)
                            : nil
                    },
                    reserving: false)
            }
            if pieConfig?.showsLabels == true {
                let values = slices.indices.map { index -> Float in
                    guard visible.indices.contains(index), visible[index], slices[index].value.isFinite else { return 0 }
                    return max(0, slices[index].value)
                }
                let total = values.reduce(0, +)
                for index in slices.indices where !slices[index].title.isEmpty && total > 0 && values[index] > 0 {
                    let share = Int((values[index] / total * 100).rounded())
                    add(ChartOverlayItem(kind: .slice, title: slices[index].title, subtitle: "\(share)%"),
                        pie.flatMap { g -> ChartOverlayPlacement? in
                            guard let wedge = g.wedges.first(where: { $0.index == index }) else { return nil }
                            let anchor = g.labelAnchor(of: wedge)
                            // Labels grow away from the ring on their own side.
                            return ChartOverlayPlacement(point(anchor.x, anchor.y),
                                                         horizontal: anchor.x >= g.center.x ? 0 : 1,
                                                         vertical: 0.5)
                        })
                }
            }
        }

        if let selection, let callout = callout(selection) {
            var placement: ChartOverlayPlacement?
            if let mark = resolution?.cartesian?.mark(callout.point) {
                placement = ChartOverlayPlacement(point(mark.x, mark.bar.map(\.minY) ?? mark.y),
                                                  horizontal: 0.5, vertical: 1, gap: 10, flips: true)
            } else if let pie = resolution?.pie,
                      let wedge = pie.wedges.first(where: { $0.index == callout.point.index }) {
                let anchor = pie.anchor(of: wedge)
                placement = ChartOverlayPlacement(point(anchor.x, anchor.y),
                                                  horizontal: 0.5, vertical: 1, gap: 10, flips: true)
            }
            add(ChartOverlayItem(kind: .callout, title: callout.title, rows: callout.rows),
                placement, reserving: false)
        }
        return slots
    }

    /// The pie configuration, when this plot is a pie.
    private var pieConfig: ChartConfiguration.Pie? {
        if case .pie = content { return pie }
        return nil
    }

    /// Plot space is `Float`; overlay placement is `CGFloat`.
    private func point(_ x: Float, _ y: Float) -> CGPoint {
        CGPoint(x: CGFloat(x), y: CGFloat(y))
    }

    // MARK: Geometry

    /// Resolves the stack, the scale, the axis frame and the marks for one size.
    ///
    /// Drawing, hit-testing and the callout all go through here so they agree on
    /// where the plot is. `cache` reuses an earlier resolution when the space and
    /// the text metrics are unchanged; the caller drops it when the data moves.
    func resolve(in size: CGSize, origin: CGPoint = .zero, text: ChartText,
                 cache: ChartResolutionCache? = nil) -> ChartResolution {
        guard let cache else { return buildResolution(in: size, origin: origin, text: text) }
        return cache.resolution(size: size, origin: origin, text: text.token) {
            buildResolution(in: size, origin: origin, text: text)
        }
    }

    private func buildResolution(in size: CGSize, origin: CGPoint, text: ChartText) -> ChartResolution {
        switch content {
        case .cartesian(let series, let guides, let xDomain):
            let axes = ChartValueAxes.resolve(series: series, visible: visible, style: style,
                                              guides: guides, axis: axis)
            let xScale = xDomain.isNumeric
                ? ChartScale.fit(extent: axes.shown.flatMap { series[$0].data.map(\.x) },
                                 lower: nil, upper: nil, tickCount: axis.x.tickCount,
                                 kind: axis.x.kind)
                : nil
            let frame = ChartFrame.cartesian(axes: axes, xDomain: xDomain, xScale: xScale,
                                             style: style, axis: axis,
                                             size: size, origin: origin, text: text)
            let geometry = ChartGeometry(series: series, axes: axes, xDomain: xDomain, xScale: xScale,
                                         barSpacing: style.barSpacing, plot: frame.plot)
            return ChartResolution(frame: frame, cartesian: geometry, pie: nil)
        case .pie(let slices):
            let frame = ChartFrame.pie(style: style, size: size, origin: origin)
            let geometry = PieGeometry(slices: slices, visible: visible,
                                       innerRatio: style.innerRadiusRatio,
                                       startAngle: pie.startAngle, sortsByValue: pie.sortsByValue,
                                       plot: frame.plot)
            return ChartResolution(frame: frame, cartesian: nil, pie: geometry)
        }
    }

    private func hit(x: Float, y: Float, node: Node) -> ChartPoint? {
        let text = ChartText(theme: node.theme, environment: TextEnvironmentHolder.current)
        // Pointer coordinates are absolute, so the plot resolves in the same
        // space the draw pass used — one cache entry serves both.
        let resolution = resolve(in: node.frame.size, origin: node.absoluteOrigin,
                                 text: text, cache: session(node)?.cache)
        if let geometry = resolution.cartesian {
            return geometry.nearest(x: x, y: y).map { ChartPoint(series: $0.series, index: $0.index) }
        }
        return resolution.pie?.wedge(atX: x, y: y).map { ChartPoint(series: 0, index: $0.index) }
    }

    /// The input state and resolution cache for a node, when it has one.
    private func session(_ node: Node) -> ChartSession? {
        node.attachments[ChartSession.key] as? ChartSession
    }

    /// Visible (series, count) pairs in order; a pie is one series of slices.
    private var tracks: [(series: Int, count: Int)] {
        switch content {
        case .cartesian(let series, _, _):
            series.indices.filter { visible[$0] && !series[$0].data.isEmpty }
                .map { ($0, series[$0].data.count) }
        case .pie(let slices):
            slices.isEmpty ? [] : [(0, slices.count)]
        }
    }

    /// Keyboard stepping skips gaps, which hold no value to read out.
    private func isVisible(_ point: ChartPoint) -> Bool {
        switch content {
        case .cartesian(let series, _, _):
            guard series.indices.contains(point.series),
                  series[point.series].data.indices.contains(point.index) else { return false }
            return series[point.series].data[point.index].y.isFinite
        case .pie(let slices):
            guard slices.indices.contains(point.index) else { return false }
            return visible[point.index] && slices[point.index].value > 0
        }
    }

    func stepped(index delta: Int, from current: ChartPoint?) -> ChartPoint? {
        guard let first = tracks.first else { return current }
        let start = current ?? ChartPoint(series: first.series, index: delta > 0 ? -1 : first.count)
        guard let track = tracks.first(where: { $0.series == start.series }) ?? tracks.first else { return current }
        var next = ChartPoint(series: track.series, index: start.index)
        repeat {
            next.index = max(0, min(track.count - 1, next.index + delta))
        } while !isVisible(next) && next.index > 0 && next.index < track.count - 1
        return isVisible(next) ? next : current
    }

    func stepped(series delta: Int, from current: ChartPoint?) -> ChartPoint? {
        guard case .cartesian = content, !tracks.isEmpty else { return current }
        let position = tracks.firstIndex { $0.series == current?.series } ?? (delta > 0 ? -1 : tracks.count)
        let track = tracks[max(0, min(tracks.count - 1, position + delta))]
        return ChartPoint(series: track.series, index: min(current?.index ?? 0, track.count - 1))
    }

    func jumped(toEnd: Bool, from current: ChartPoint?) -> ChartPoint? {
        guard let track = tracks.first(where: { $0.series == current?.series }) ?? tracks.first else { return current }
        return ChartPoint(series: track.series, index: toEnd ? track.count - 1 : 0)
    }

    // MARK: Accessibility

    /// Publishes the chart's meaning: a name, the value under the reading
    /// cursor — or the range when there is none — and the stepping actions.
    ///
    /// Stepping is offered even without a selection binding. A chart that
    /// cannot be clicked can still be read value by value; the only difference
    /// is where the cursor lives, in the session rather than in the caller's
    /// binding.
    private func publishAccessibility(node: Node, session: ChartSession) {
        let cursor = session.reading
        let steps = canStep
        node.accessibility = AccessibilitySemantics(.image) {
            $0.label = summary.label
            $0.value = cursor.flatMap { readout(for: $0) } ?? summary.value
            if input.isEnabled {
                $0.help = "Use Increase and Decrease to read each value, or Left and Right to move, Up and Down between series, Escape to clear"
            } else if steps {
                $0.help = "Use Increase and Decrease to read each value"
            }
        }
        node.accessibilityActions = AccessibilityActions()
        guard steps else { return }
        node.accessibilityActions.increment = { [weak node] in step(1, node: node) }
        node.accessibilityActions.decrement = { [weak node] in step(-1, node: node) }
    }

    /// Whether there is anything to step through.
    private var canStep: Bool {
        input.isEnabled || (accessibility.exposesValues && tracks.contains { $0.count > 0 })
    }

    /// Moves the reading cursor and republishes, so an assistive client hears
    /// the new value without the caller's view state changing.
    private func step(_ delta: Int, node: Node?) {
        guard let node, let session = session(node) else { return }
        let next = stepped(index: delta, from: session.reading)
        session.reading = next
        if input.isEnabled, next != session.current {
            session.current = next
            input.select(next)
        }
        publishAccessibility(node: node, session: session)
    }

    /// One accessible sentence for a point: what it is, every visible series at
    /// it, and where it sits in the series.
    func readout(for point: ChartPoint) -> String? {
        guard let callout = callout(point), !callout.rows.isEmpty else { return nil }
        let values = callout.rows
            .map { $0.title.isEmpty ? $0.value : "\($0.title) \($0.value)" }
            .joined(separator: ", ")
        // A middle dot is silent when spoken; the callout's separator becomes
        // a comma so a value never runs into its share.
        let head = (callout.title.isEmpty ? values : "\(callout.title): \(values)")
            .replacingOccurrences(of: " · ", with: ", ")
        let count = trackCount(at: point)
        guard count > 1, !namesPositionInTitle(point) else { return head }
        return "\(head). \(positionNoun) \(point.index + 1) of \(count)"
    }

    /// True when the point's own title already says where it sits. An
    /// unlabelled category axis and an untitled slice both fall back to their
    /// position, so repeating it would read "Point 3: … Point 3 of 12".
    private func namesPositionInTitle(_ point: ChartPoint) -> Bool {
        switch content {
        case .cartesian(_, _, .categorical(let labels)):
            return labels.isEmpty
        case .pie(let slices):
            return slices.indices.contains(point.index) && slices[point.index].title.isEmpty
        case .cartesian(_, _, .numeric):
            return false
        }
    }

    /// How many positions the cursor's own track holds. A gap or a hidden
    /// slice keeps its position, so the count is the track's full length.
    private func trackCount(at point: ChartPoint) -> Int {
        switch content {
        case .cartesian(let series, _, _):
            return series.indices.contains(point.series) ? series[point.series].data.count : 1
        case .pie(let slices):
            return slices.count
        }
    }

    private var positionNoun: String {
        if case .pie = content { return "Slice" }
        return "Point"
    }

    // MARK: Drawing

    private func draw(node: Node, origin: CGPoint, list: DrawList) {
        let size = node.frame.size
        guard size.width > 0, size.height > 0 else { return }
        let text = ChartText(theme: node.theme, environment: TextEnvironmentHolder.current)
        let resolution = resolve(in: size, origin: origin, text: text, cache: session(node)?.cache)
        let paint = ChartPaint(theme: node.theme, style: style, selection: selection,
                               frame: resolution.frame, text: text)
        paint.background(UIRect(x: Float(origin.x), y: Float(origin.y), width: Float(size.width), height: Float(size.height)), list: list)
        let marks: (series: [ChartSeries], guides: ChartGuides, geometry: ChartGeometry)? = {
            guard let geometry = resolution.cartesian, geometry.plot.width > 0, geometry.plot.height > 0,
                  case .cartesian(let series, let guides, _) = content else { return nil }
            return (series, guides, geometry)
        }()
        // Bands sit under the grid so the grid still reads as the top rule.
        if let marks { paint.bands(marks.guides.bands, geometry: marks.geometry, list: list) }
        paint.grid(list: list)
        if let marks {
            paint.thresholds(marks.guides.thresholds, geometry: marks.geometry, list: list)
            paint.series(marks.series, geometry: marks.geometry, list: list)
            paint.markers(marks.guides.markers, geometry: marks.geometry, list: list)
            paint.selection(marks.series, geometry: marks.geometry, list: list)
        }
        if let geometry = resolution.pie, case .pie(let slices) = content {
            if pie.showsLabels { paint.pieLeaders(slices, geometry: geometry, list: list) }
            paint.pie(slices, geometry: geometry, list: list)
        }
        paint.axisLabels(list: list)
        if FocusChainHolder.current?.focused === node, FocusChainHolder.current?.isFocusVisible == true {
            list.addRoundedRectStroke(UIRect(x: Float(origin.x), y: Float(origin.y), width: Float(size.width), height: Float(size.height)),
                                      radius: node.theme.radius.sm, width: 2, color: node.theme.colors.focusRing)
        }
    }
}

/// Input state that must survive recomposition: the press and the pin.
final class ChartSession: NodeResource {
    static let key = "chart.session"
    var isPressed = false
    /// A clicked, tapped or keyboard selection stays after the pointer leaves.
    var isPinned = false
    /// The press started on the pinned point, so its release clears the pin.
    var releasesPin = false
    /// The selection as of the latest event, ahead of the binding's recomposition.
    var current: ChartPoint?
    /// Where assistive stepping has read to. Independent of `current` so a
    /// chart without a binding still reads value by value.
    var reading: ChartPoint?
    /// The geometry the cached resolution was built from.
    fileprivate var signature: ChartGeometrySignature?
    let cache = ChartResolutionCache()
    private weak var capture: PointerCapture?
    private weak var focus: FocusChain?

    func mount(node: Node) { capture = PointerCaptureHolder.current; focus = FocusChainHolder.current }
    func unmount(node: Node) {
        release(node: node)
        if focus?.focused === node { focus?.clear() }
    }
    func press(node: Node) { isPressed = true; capture?.acquire(node) }
    func release(node: Node) {
        isPressed = false
        if capture?.target === node { capture?.release() }
    }
}

/// Repaint identity: what is painted, and how.
///
/// Geometry is split out because a pointer move repaints without moving a mark:
/// `ChartPlot` compares only `geometry` before deciding to drop the cached
/// resolution.
private struct ChartPaintIdentity: Equatable {
    let geometry: ChartGeometrySignature
    let colors: [Color]
    let selection: ChartPoint?

    init(plot: ChartPlot, theme: Theme) {
        geometry = ChartGeometrySignature(plot: plot, theme: theme)
        var colors: [Color]
        switch plot.content {
        case .cartesian(let series, let guides, _):
            colors = series.map { $0.color.resolve(theme) }
                + guides.thresholds.map { $0.color.resolve(theme) }
                + guides.bands.map { $0.color.resolve(theme) }
                + guides.markers.map { $0.color.resolve(theme) }
        case .pie(let slices):
            colors = slices.indices.map { slices[$0].resolvedColor(at: $0).resolve(theme) }
        }
        colors += [plot.style.background?.resolve(theme) ?? .clear, plot.style.gridColor.resolve(theme)]
        self.colors = colors
        selection = plot.selection
    }
}

/// Everything the resolved geometry depends on: the data, how each series is
/// drawn, which value axis it reads, the guides, the axis chrome, and the text
/// metrics the gutters are measured with.
///
/// Colors and selection are deliberately absent — they change what is painted,
/// never where a mark sits, so they must not throw the geometry away.
private struct ChartGeometrySignature: Equatable {
    let data: [[ChartDatum]]
    /// Slice values, for a pie. Empty on a cartesian chart.
    let pieValues: [Float]
    let marks: [String]
    let styles: [ChartSeriesStyle]
    /// Which value axis each series reads.
    let sides: [String]
    let visible: [Bool]
    let xDomain: ChartXDomain?
    let thresholds: [[Float]]
    let bands: [[Float]]
    let markers: [Int]
    let style: [Float]
    /// Axis chrome that changes what is drawn: titles, tick counts and kinds.
    let axis: [String]
    /// Pie order and start angle, which decide every wedge.
    let pie: [Float]
    /// The caption the gutters are measured in; a scale change moves the plot.
    let text: TextStyleToken

    init(plot: ChartPlot, theme: Theme) {
        visible = plot.visible
        text = ChartText(theme: theme, environment: nil).token
        let style = plot.style
        self.style = [style.minValue ?? .nan, style.maxValue ?? .nan, style.showsGrid ? 1 : 0, style.lineWidth,
                      style.barSpacing, style.contentInset, style.innerRadiusRatio,
                      Self.stackingCode(style.stacking)]
        let axis = plot.axis
        self.axis = [axis.y.title, axis.x.title, "\(axis.y.tickCount)", "\(axis.x.tickCount)",
                     axis.y.isLabeled ? "y" : "", axis.x.isLabeled ? "x" : "",
                     axis.secondary.title, "\(axis.secondary.tickCount)",
                     axis.secondary.isLabeled ? "s" : "",
                     axis.y.kind == .log ? "log" : "lin", axis.x.kind == .log ? "log" : "lin",
                     axis.secondary.kind == .log ? "log" : "lin"]
        let pie = plot.pie
        self.pie = [pie.startAngle, pie.sortsByValue ? 1 : 0, pie.showsLabels ? 1 : 0]
        switch plot.content {
        case .cartesian(let series, let guides, let x):
            // The series' own buffers, not copies: comparing them costs a walk,
            // but nothing here allocates per datum.
            data = series.map(\.data)
            pieValues = []
            marks = series.map { "\($0.mark)" }
            styles = series.map(\.style)
            sides = series.map { $0.axis == .right ? "r" : "l" }
            xDomain = x
            thresholds = guides.thresholds.map { [$0.value, $0.axis == .right ? 1 : 0] }
            bands = guides.bands.map { [$0.from, $0.to, $0.axis == .right ? 1 : 0] }
            markers = guides.markers.map(\.index)
        case .pie(let slices):
            data = []
            pieValues = slices.map(\.value)
            marks = ["pie"]
            styles = []
            sides = []
            xDomain = nil
            thresholds = []; bands = []; markers = []
        }
    }

    private static func stackingCode(_ stacking: ChartStacking) -> Float {
        switch stacking {
        case .none: 0
        case .stacked: 1
        case .percent: 2
        }
    }

    /// `style` carries NaN for an unset bound, and NaN never equals itself, so
    /// the elementwise comparison tolerates a matched pair.
    static func == (lhs: Self, rhs: Self) -> Bool {
        lhs.data == rhs.data && lhs.pieValues == rhs.pieValues && lhs.marks == rhs.marks
            && lhs.styles == rhs.styles && lhs.sides == rhs.sides && lhs.visible == rhs.visible
            && lhs.xDomain == rhs.xDomain && lhs.thresholds == rhs.thresholds
            && lhs.bands == rhs.bands && lhs.markers == rhs.markers
            && lhs.axis == rhs.axis && lhs.pie == rhs.pie && lhs.text == rhs.text
            && lhs.style.elementsEqual(rhs.style) { $0 == $1 || ($0.isNaN && $1.isNaN) }
    }
}
