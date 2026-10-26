internal import Foundation
import CoveUIRuntime

/// Gesture state is transient; only completed/live intervals go to bindings.
final class ChartNavigation {
    enum Drag {
        case brush(anchor: Float, window: ChartWindow)
        case pan(anchor: Float, window: ChartWindow)
    }
    var drag: Drag?
    var window: ChartWindow?
    func cancel() { drag = nil; window = nil }
}

extension ChartPlot {
    var isCartesian: Bool { if case .cartesian = content { return true }; return false }
    var isInteractive: Bool { input.isEnabled || (isCartesian && (viewport.range != nil || brush != nil)) }

    func installInput(node: Node, session: ChartSession) {
        guard isInteractive, let registry = InteractionRegistryHolder.current else {
            session.release(node: node)
            session.navigation.cancel()
            InteractionRegistryHolder.current?.remove(node)
            return
        }
        let choose: (ChartPoint?) -> Void = { next in
            session.current = next
            input.select(next)
        }
        registry.setHover(node) { [weak node] phase in
            guard let node, phase == .leave, !session.isPinned, !session.isPressed, node.supportsControlHover else { return }
            if input.isEnabled { choose(nil) }
        }
        registry.setMotion(node) { [weak node] event, phase in
            guard let node, phase == .target else { return .ignored }
            if session.isPressed, moveNavigation(x: event.x, y: event.y, node: node, session: session) { return .handled }
            guard input.isEnabled, session.isPressed || (node.supportsControlHover && !session.isPinned) else { return .ignored }
            let hit = hit(x: event.x, y: event.y, node: node)
            if hit != nil || !session.isPressed, hit != session.current { choose(hit) }
            return session.isPressed ? .handled : .ignored
        }
        registry.setPointer(node) { [weak node] event, phase, route in
            guard let node, event.button == .left, route == .target else { return .ignored }
            switch phase {
            case .down:
                if isCartesian, event.clicks == 2, viewport.range != nil {
                    viewport.range?.wrappedValue = nil
                    session.navigation.cancel()
                    return .handled
                }
                if beginNavigation(event: event, node: node, session: session) {
                    session.press(node: node)
                    return .handled
                }
                guard input.isEnabled else { return .ignored }
                let hit = hit(x: event.x, y: event.y, node: node)
                session.releasesPin = session.isPinned && hit != nil && hit == session.current
                session.press(node: node)
                choose(session.releasesPin ? nil : hit)
            case .up:
                guard session.isPressed else { return .ignored }
                let navigated = moveNavigation(x: event.x, y: event.y, node: node, session: session)
                session.release(node: node)
                session.navigation.drag = nil
                if !navigated {
                    session.isPinned = !session.releasesPin && session.current != nil
                    session.releasesPin = false
                }
            }
            return .handled
        }
        registry.setWheel(node) { [weak node] event, phase in
            guard let node, phase == .target, isCartesian, viewport.range != nil,
                  let geometry = geometry(node, session: session),
                  let window = session.navigation.window ?? geometry.window else { return .ignored }
            let x = event.mouseX ?? geometry.plot.minX + geometry.plot.width / 2
            let y = event.mouseY ?? geometry.plot.minY + geometry.plot.height / 2
            guard contains(x: x, y: y, plot: geometry.plot) else { return .ignored }
            let delta = event.displacement(lineStep: 24, pageWidth: geometry.plot.width, pageHeight: geometry.plot.height)
            guard delta.x.isFinite, delta.y.isFinite else { return .ignored }
            let next: ChartWindow
            if abs(delta.x) > abs(delta.y) {
                next = window.pan(fraction: -delta.x / max(1, geometry.plot.width))
            } else {
                let anchor = window.fraction(x: x, y: y, plot: geometry.plot, orientation: style.orientation)
                next = window.zoom(factor: Float(exp(Double(min(500, max(-500, -delta.y))) * 0.005)), anchor: anchor)
            }
            guard next.range != window.range else { return .ignored }
            setWindow(next, session: session)
            return .handled
        }
        registry.setKey(node) { [weak node] event, phase in
            guard let node, phase == .target, event.modifiers.isDisjoint(with: [.ctrl, .gui, .alt]) else { return .ignored }
            if isCartesian, viewport.range != nil {
                switch event.scancode {
                case Scancode.digit0:
                    viewport.range?.wrappedValue = nil; session.navigation.window = nil
                    return .handled
                case Scancode.equals, Scancode.minus:
                    guard let window = session.navigation.window ?? geometry(node, session: session)?.window else { return .ignored }
                    setWindow(window.zoom(factor: event.scancode == Scancode.equals ? 0.8 : 1.25, anchor: 0.5), session: session)
                    return .handled
                default: break
                }
            }
            if event.scancode == Scancode.escape, brush?.wrappedValue != nil {
                brush?.wrappedValue = nil
                session.navigation.cancel(); session.release(node: node)
                return .handled
            }
            guard input.isEnabled else { return .ignored }
            let current = session.current
            switch event.scancode {
            case Scancode.arrowRight: choose(stepped(index: 1, from: current))
            case Scancode.arrowLeft: choose(stepped(index: -1, from: current))
            case Scancode.arrowDown: choose(stepped(series: 1, from: current))
            case Scancode.arrowUp: choose(stepped(series: -1, from: current))
            case Scancode.home: choose(jumped(toEnd: false, from: current))
            case Scancode.end: choose(jumped(toEnd: true, from: current))
            case Scancode.escape:
                guard current != nil else { return .ignored }
                session.isPinned = false; session.reading = nil; choose(nil)
                return .handled
            default: return .ignored
            }
            session.isPinned = session.current != nil
            return .handled
        }
    }

    private func geometry(_ node: Node, session: ChartSession) -> ChartGeometry? {
        resolve(in: node.frame.size, origin: node.absoluteOrigin,
                text: ChartText(theme: node.theme, environment: TextEnvironmentHolder.current),
                cache: session.cache).cartesian
    }

    private func contains(x: Float, y: Float, plot: UIRect) -> Bool {
        x >= plot.minX && x <= plot.maxX && y >= plot.minY && y <= plot.maxY
    }

    private func beginNavigation(event: MouseButtonEvent, node: Node, session: ChartSession) -> Bool {
        guard isCartesian, let geometry = geometry(node, session: session), let window = geometry.window,
              contains(x: event.x, y: event.y, plot: geometry.plot) else { return false }
        let fraction = window.fraction(x: event.x, y: event.y, plot: geometry.plot, orientation: style.orientation)
        if viewport.range != nil, event.modifiers.hasAlt || (brush == nil && !input.isEnabled) {
            session.navigation.drag = .pan(anchor: fraction, window: window)
        } else if brush != nil {
            let value = window.value(at: min(1, max(0, fraction)))
            session.navigation.drag = .brush(anchor: value, window: window)
            brush?.wrappedValue = value...value
        } else { return false }
        return true
    }

    private func moveNavigation(x: Float, y: Float, node: Node, session: ChartSession) -> Bool {
        guard let drag = session.navigation.drag, let geometry = geometry(node, session: session) else { return false }
        switch drag {
        case .brush(let anchor, let window):
            let fraction = window.fraction(x: x, y: y, plot: geometry.plot, orientation: style.orientation)
            let value = window.value(at: min(1, max(0, fraction)))
            brush?.wrappedValue = min(anchor, value)...max(anchor, value)
        case .pan(let anchor, let window):
            let fraction = window.fraction(x: x, y: y, plot: geometry.plot, orientation: style.orientation)
            setWindow(window.pan(fraction: anchor - fraction), session: session)
        }
        return true
    }

    private func setWindow(_ window: ChartWindow, session: ChartSession) {
        session.navigation.window = window
        viewport.range?.wrappedValue = window.range == window.full ? nil : window.range
    }
}
