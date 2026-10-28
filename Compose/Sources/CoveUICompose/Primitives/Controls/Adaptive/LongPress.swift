internal import Foundation
import CoveUIScene
import CoveUIText
import CoveUIImaging

/// A hold observes the tap until it wins. Motion/wheel input cancels before
/// an ancestor scroll takes over; only recognition cancels the child press.
final class TouchLongPress: NodeResource, AnyAnimationController {
    private weak var node: Node?
    private var origin: CGPoint?
    private var elapsed = 0.0
    var onRecognized: (CGPoint) -> Void = { _ in }
    var isFinished = true

    func mount(node: Node) { self.node = node }
    func unmount(node: Node) { cancel(); self.node = nil }
    func begin(_ point: CGPoint) {
        cancel(); origin = point; elapsed = 0; isFinished = false
        AnimatorScheduler.current.register(self)
    }
    func move(_ point: CGPoint) {
        guard let origin else { return }
        if hypot(point.x - origin.x, point.y - origin.y) > 10 { cancel() }
    }
    func tick(deltaTime: Double) {
        guard !isFinished, let node, let origin else { return }
        guard node.acceptsSubtreeInput else { cancel(); return }
        if let captured = PointerCaptureHolder.current?.target {
            var candidate: Node? = captured
            while candidate != nil, candidate !== node { candidate = candidate?.parent }
            guard candidate === node else { cancel(); return }
        }
        elapsed += max(0, deltaTime)
        guard elapsed >= 0.5 else { return }
        isFinished = true; self.origin = nil
        PointerCaptureHolder.current?.release()
        onRecognized(origin)
    }
    func cancel() { isFinished = true; origin = nil }
    func finishImmediately() { cancel() }
}
