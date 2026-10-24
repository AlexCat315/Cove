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
    let callout: ChartCallout?
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
        let choose: (ChartPoint?) -> Void = { next in
            session.current = next
            snapshot.input.select(next)
        }
        node.isHitTestable = input.isEnabled
        node.isFocusable = input.isEnabled
        node.cursor = input.isEnabled ? .crosshair : .arrow
        node.accessibility = AccessibilitySemantics(.image) {
            $0.label = summary.label; $0.value = summary.value
            if input.isEnabled { $0.help = "Use Left and Right to move between values, Up and Down between series, Escape to clear" }
        }
        node.accessibilityActions = AccessibilityActions()
        if input.isEnabled {
            node.accessibilityActions.increment = { choose(snapshot.stepped(index: 1, from: session.current)) }
            node.accessibilityActions.decrement = { choose(snapshot.stepped(index: -1, from: session.current)) }
        }
        let theme = node.theme
        node.updateDraw(identity: ChartPaintIdentity(plot: self, theme: theme)) { [weak node] list, origin in
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
        let slots = overlaySlots(in: nil, text: text)
        guard !slots.isEmpty else { return [] }
        return [
            ChartOverlayLayout(plot: self, theme: node.theme, environment: environment) {
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
    func overlaySlots(in size: CGSize?, text: ChartText) -> [ChartOverlaySlot] {
        let resolution = size.map { resolve(in: $0, text: text) }
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
                        ChartOverlayPlacement(point(g.plot.minX + 4, g.y(band.to)),
                                              horizontal: 0, vertical: 0, gap: 2)
                    })
            }
            for threshold in guides.thresholds where !threshold.label.isEmpty && threshold.value.isFinite {
                add(ChartOverlayItem(kind: .threshold, title: threshold.label),
                    geometry.map { g in
                        ChartOverlayPlacement(point(g.plot.maxX - 2, g.y(threshold.value)),
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

        if let callout {
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
    /// where the plot is.
    func resolve(in size: CGSize, origin: CGPoint = .zero, text: ChartText) -> ChartResolution {
        switch content {
        case .cartesian(let series, let guides, let xDomain):
            let stack = ChartStack(series: series, visible: visible, style: style)
            let yScale = ChartScale.fit(stack: stack, thresholds: guides.thresholds, bands: guides.bands,
                                        style: style, tickCount: axis.y.tickCount)
            let xScale = xDomain.isNumeric
                ? ChartScale.fit(extent: stack.shown.flatMap { series[$0].data.map(\.x) },
                                 lower: nil, upper: nil, tickCount: axis.x.tickCount)
                : nil
            let frame = ChartFrame.cartesian(stack: stack, xDomain: xDomain, xScale: xScale,
                                             yScale: yScale, style: style, axis: axis,
                                             size: size, origin: origin, text: text)
            let geometry = ChartGeometry(series: series, stack: stack, xDomain: xDomain, xScale: xScale,
                                         yScale: yScale, barSpacing: style.barSpacing, plot: frame.plot)
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
        let local = (x - Float(node.absoluteOrigin.x), y - Float(node.absoluteOrigin.y))
        let text = ChartText(theme: node.theme, environment: TextEnvironmentHolder.current)
        let resolution = resolve(in: node.frame.size, text: text)
        if let geometry = resolution.cartesian {
            return geometry.nearest(x: local.0, y: local.1).map { ChartPoint(series: $0.series, index: $0.index) }
        }
        return resolution.pie?.wedge(atX: local.0, y: local.1).map { ChartPoint(series: 0, index: $0.index) }
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

    // MARK: Drawing

    private func draw(node: Node, origin: CGPoint, list: DrawList) {
        let size = node.frame.size
        guard size.width > 0, size.height > 0 else { return }
        let text = ChartText(theme: node.theme, environment: TextEnvironmentHolder.current)
        let resolution = resolve(in: size, origin: origin, text: text)
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

/// Repaint identity: values, resolved colors, per-series styling and selection.
private struct ChartPaintIdentity: Equatable {
    let values: [[Float]]
    let xs: [[Float]]
    var colors: [Color]
    let marks: [String]
    let styles: [ChartSeriesStyle]
    let xDomain: ChartXDomain?
    let visible: [Bool]
    let selection: ChartPoint?
    let thresholds: [Float]
    let bands: [[Float]]
    let markers: [Int]
    let style: [Float]
    /// Axis chrome that changes what is drawn: titles and tick counts.
    let axis: [String]
    /// Pie order and start angle, which decide every wedge.
    let pie: [Float]

    init(plot: ChartPlot, theme: Theme) {
        switch plot.content {
        case .cartesian(let series, let guides, let x):
            values = series.map { $0.data.map(\.y) }
            xs = series.map { $0.data.map(\.x) }
            colors = series.map { $0.color.resolve(theme) }
                + guides.thresholds.map { $0.color.resolve(theme) }
                + guides.bands.map { $0.color.resolve(theme) }
                + guides.markers.map { $0.color.resolve(theme) }
            marks = series.map { "\($0.mark)" }
            styles = series.map(\.style)
            xDomain = x
            thresholds = guides.thresholds.map(\.value)
            bands = guides.bands.map { [$0.from, $0.to] }
            markers = guides.markers.map(\.index)
        case .pie(let slices):
            values = [slices.map(\.value)]
            xs = []
            colors = slices.indices.map { slices[$0].resolvedColor(at: $0).resolve(theme) }
            marks = ["pie"]
            styles = []
            xDomain = nil
            thresholds = []; bands = []; markers = []
        }
        visible = plot.visible
        selection = plot.selection
        let style = plot.style
        self.style = [style.minValue ?? .nan, style.maxValue ?? .nan, style.showsGrid ? 1 : 0, style.lineWidth,
                      style.barSpacing, style.contentInset, style.innerRadiusRatio, style.stacking == .stacked ? 1 : 0]
        let axis = plot.axis
        self.axis = [axis.y.title, axis.x.title, "\(axis.y.tickCount)", "\(axis.x.tickCount)",
                     axis.y.isLabeled ? "y" : "", axis.x.isLabeled ? "x" : ""]
        let pie = plot.pie
        self.pie = [pie.startAngle, pie.sortsByValue ? 1 : 0, pie.showsLabels ? 1 : 0]

        colors += [style.background?.resolve(theme) ?? .clear, style.gridColor.resolve(theme)]
    }

    static func == (lhs: Self, rhs: Self) -> Bool {
        lhs.values == rhs.values && lhs.xs == rhs.xs && lhs.colors == rhs.colors && lhs.marks == rhs.marks
            && lhs.styles == rhs.styles && lhs.xDomain == rhs.xDomain && lhs.visible == rhs.visible
            && lhs.selection == rhs.selection && lhs.thresholds == rhs.thresholds && lhs.markers == rhs.markers
            && lhs.bands == rhs.bands && lhs.axis == rhs.axis && lhs.pie == rhs.pie
            && lhs.style.elementsEqual(rhs.style) { $0 == $1 || ($0.isNaN && $1.isNaN) }
    }
}
