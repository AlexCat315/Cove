import Foundation

public enum WindowRevealContext {
    private static let activationKey = "CoveUI.control.activation-origin"

    public static func recordActivation(_ node: Node) {
        let frame = node.absoluteFrame
        root(of: node).attachments[activationKey] = CGPoint(x: frame.midX, y: frame.midY)
    }

    public static func activationOrigin(for node: Node) -> CGPoint? {
        root(of: node).attachments[activationKey] as? CGPoint
    }

    public static func clearActivation(in node: Node?) {
        node?.attachments.removeValue(forKey: activationKey)
    }

    public static func bounds(for node: Node) -> CGRect { root(of: node).frame }

    private static func root(of node: Node) -> Node {
        var root = node
        while let parent = root.parent { root = parent }
        return root
    }
}
