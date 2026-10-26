private import CoveUIRuntime

/// Read-only scrolling content needs a keyboard entrance. Interactive content
/// already provides one; an extra ancestor stop would steal modal/menu focus.
enum ScrollKeyboardFocus {
    private struct Cache {
        let focusRevision: UInt64
        let structureVersion: UInt64
        let canScroll: Bool
        let hasViewport: Bool
        let focusable: Bool
    }
    private static let cacheKey = "__scroll.keyboardFocus"

    static func update(_ node: Node, canScroll: Bool) {
        let hasViewport = canScroll && node.frame.width > 0.5 && node.frame.height > 0.5
        let focusRevision = FocusRevision.current
        let structureVersion = node.subtreeStructureVersion
        let cache = node.attachments[cacheKey] as? Cache
        if let cache, cache.focusRevision == focusRevision,
           cache.structureVersion == structureVersion,
           cache.canScroll == canScroll, cache.hasViewport == hasViewport {
            apply(cache.focusable, to: node)
            return
        }
        var hasControl = false
        if hasViewport {
            var pending = node.children
            while let child = pending.popLast() {
                guard child.acceptsSubtreeInput else { continue }
                if child.isFocusable && child.isTabStop { hasControl = true; break }
                pending.append(contentsOf: child.children)
            }
        }
        let focusable = hasViewport && !hasControl
        node.attachments[cacheKey] = Cache(focusRevision: focusRevision, structureVersion: structureVersion,
                                           canScroll: canScroll, hasViewport: hasViewport, focusable: focusable)
        apply(focusable, to: node)
    }

    private static func apply(_ focusable: Bool, to node: Node) {
        node.isFocusable = focusable
        if !focusable, FocusChainHolder.current?.focused === node {
            FocusChainHolder.current?.clear()
        }
    }
}
