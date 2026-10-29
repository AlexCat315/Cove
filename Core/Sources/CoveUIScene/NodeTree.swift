import CoveUICore
/// Owns the root node and drives depth-first traversal.
///
/// Usage:
/// ```swift
/// let tree = NodeTree()
/// tree.root = Node()
/// tree.markDirty(someChild)
/// tree.flush()   // resets layout + render invalidation flags after a frame
/// ```
public final class NodeTree: @unchecked Sendable {

    /// The single root of the scene graph.
    public var root: Node?
    public let timeline = PerformanceTimeline()

    // MARK: - Node count (memoised)

    /// Cache key for `nodeCount`: the root's identity plus the structural
    /// version that every membership change bumps on it.
    ///
    /// `Node.addChild` / `removeChild` / `reorderChildren` are the only
    /// membership mutators (`children` is `private(set)`), and each routes
    /// through `bumpSubtreeStructureVersion()`, which walks to the root. A
    /// changed root version is therefore an exact "the reachable node set may
    /// have changed" signal, so a steady frame answers in O(1) instead of
    /// walking the whole tree — the host asks twice per frame to publish one
    /// diagnostic number.
    private var cachedNodeCount = 0
    private var cachedNodeCountVersion: UInt64 = 0
    private var cachedNodeCountRoot: ObjectIdentifier?
    /// Reused by each recomputation so even a structural frame allocates nothing.
    private var nodeCountStack: [Node] = []
    private var flushStack: [Node] = []

    public init() {}

    public var hasRenderUpdates: Bool {
        root?.renderDirty ?? false
    }

    public var nodeCount: Int {
        guard let root else {
            cachedNodeCountRoot = nil
            return 0
        }
        let identity = ObjectIdentifier(root)
        if cachedNodeCountRoot == identity,
           cachedNodeCountVersion == root.subtreeStructureVersion {
            return cachedNodeCount
        }
        nodeCountStack.removeAll(keepingCapacity: true)
        nodeCountStack.append(root)
        var count = 0
        while let node = nodeCountStack.popLast() {
            count += 1
            nodeCountStack.append(contentsOf: node.children)
        }
        cachedNodeCountRoot = identity
        cachedNodeCountVersion = root.subtreeStructureVersion
        cachedNodeCount = count
        return count
    }

    // MARK: - Dirty management

    /// Convenience wrapper around `Node.markDirty()`.
    public func markDirty(_ node: Node) {
        node.markDirty()
    }

    public func markRenderDirty(_ node: Node) {
        node.markRenderDirty()
    }

    // MARK: - Flush

    /// Depth-first traversal that resets layout and render invalidation flags
    /// on every reachable node.
    ///
    /// Layout (`LayoutTree`/Yoga) and draw (`LayerAwareNodeRenderer`) are
    /// driven directly by `ViewGraph` and `EngineHost.tick`; this method
    /// only clears the per-`Node` dirty flags after each frame so the next
    /// recompose can detect new mutations.
    public func flush() {
        guard let root else { return }
        // Instrumented at the single funnel so every host (desktop, Android,
        // browser) reports this pass: it is the one place the tree walk itself
        // shows up as frame time, and it sits outside the layout / draw spans
        // the trace already records.
        let trace = timeline.begin()
        flushStack.append(root)
        while let node = flushStack.popLast() {
            let shouldVisitChildren = node.isDirty || node.renderDirty
            node.isDirty = false
            node.renderDirty = false
            guard shouldVisitChildren else { continue }
            for child in node.children.reversed() where child.isDirty || child.renderDirty {
                flushStack.append(child)
            }
        }
        timeline.end(trace, phase: "frame", name: "Flush dirty flags")
    }
}
