import CoveUIScene
import CoveUIText
import CoveUIImaging
import Foundation
#if canImport(CoreGraphics)
import CoreGraphics
#endif

/// A thread-safe box holding the latest absolute frame of a node.
final class FrameBox: @unchecked Sendable {
    private let lock = NSLock()
    private var frame: CGRect?
    public init() {}
    var current: CGRect? { lock.withLock { frame } }
    func update(_ frame: CGRect) { lock.withLock { self.frame = frame } }
}

/// Records the node's absolute frame after every layout pass, chaining any
/// existing `layoutDidUpdate` hook so other primitives keep working.
final class FrameCaptureResource: NodeResource {
    private let box: FrameBox
    private weak var node: Node?
    private var previousLayout: ((Node) -> Void)?

    init(box: FrameBox) { self.box = box }

    func mount(node: Node) {
        self.node = node
        previousLayout = node.layoutDidUpdate
        let previous = previousLayout
        node.layoutDidUpdate = { [weak self, box] n in
            previous?(n)
            box.update(n.absoluteFrame)
        }
        box.update(node.absoluteFrame)
    }

    func unmount(node: Node) {
        node.layoutDidUpdate = previousLayout
        self.node = nil
    }
}

/// Captures the modified node's absolute frame into `box` after each layout.
struct FrameCaptureModifier: ViewModifier {
    let box: FrameBox
    init(_ box: FrameBox) { self.box = box }
    func apply(node: Node) {
        if node.firstResource(FrameCaptureResource.self) == nil {
            node.addResource(FrameCaptureResource(box: box))
        }
    }
}

extension View {
    func captureFrame(_ box: FrameBox) -> some View {
        modifier(FrameCaptureModifier(box))
    }
}
