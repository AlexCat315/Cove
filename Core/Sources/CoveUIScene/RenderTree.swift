import CoveUICore
import Foundation
#if canImport(CoreGraphics)
import CoreGraphics
#endif

/// Retained geometry and layer classification for one scene node.
public final class RenderObject {

    public weak var node: Node?
    public weak var parent: RenderObject?
    private weak var tree: RenderTree?
    public internal(set) var children: [RenderObject] = []

    /// Stable identity inherited from the paired Node. Survives reuse across
    /// recompose so cache lookups don't churn.
    public let elementID: ElementID

    /// True when this RenderObject begins a composition group whose subtree
    /// must record into its own DrawList. Computed from Node properties:
    /// clipping, non-opaque opacity, or any drop shadow forces a layer
    /// boundary because the GPU pipeline needs an isolated buffer to clip /
    /// blend / shadow over.
    public internal(set) var isLayerRoot: Bool = true

    /// Snapshot of this object's placement the last time the renderer recorded
    /// into it. Phase 4b uses this to detect "the layer only moved" vs "the
    /// layer's contents changed".
    ///
    /// The whole transform rather than just an origin, because a node can keep
    /// its origin and still need re-recording: a scale that changed leaves the
    /// recorded vertices the wrong size, and reusing them draws a stale image
    /// without reporting anything.
    ///
    /// Internal like the cache it guards: only `LayerAwareNodeRenderer` decides
    /// whether a recorded layer is still good.
    internal var lastPlacement: Affine2D?

    /// Set by Phase 4b's renderer when the cached DrawList for this layer
    /// is no longer trustworthy and must be re-recorded.
    public var cacheInvalid: Bool = true

    /// Phase 4b: cached DrawList recorded the last time this layer was
    /// composited. Holds vertices in absolute coordinates valid only for the
    /// `lastPlacement` snapshot. `LayerAwareNodeRenderer` reuses this
    /// list when `cacheInvalid == false` and the placement hasn't moved.
    /// Non-layer RenderObjects don't cache (this stays nil).
    public var cachedLayerList: DrawList?

    /// Bounds of actual cached leaf vertices, including overdraw and shadows.
    /// Node.frame alone is not a safe culling boundary for custom painters.
    var cachedPaintBounds: UIRect?
    var cachedLayerUsesLocalClips = false

    /// Parent clip stack snapshot at last record. Cache is only valid when
    /// the current parent clip stack equals this snapshot, otherwise scissor
    /// rects baked into `cachedLayerList.batches` are stale.
    public var lastClipStack: [UIClip] = []

    /// Memoised z-ordering of `children`. Nil means "recompute on next access"
    /// — the order only depends on `children` membership/order and each
    /// child's `node.zIndex`, both of which flow through `invalidateZOrder()`.
    /// Without this, `LayerAwareNodeRenderer` re-sorts every frame for any
    /// container that has ever held a non-zero `zIndex` (the
    /// `childrenMayNeedZSort` flag stays true after a zIndex is set).
    private var cachedZSortedChildren: [RenderObject]?

    /// Drop the memoised z-order. Called on membership/order change
    /// (`RenderTree.reconcileChildren` / `tearDown`) and whenever a child's
    /// `zIndex` changes (`Node.childZIndexDidChange`).
    func invalidateZOrder() {
        cachedZSortedChildren = nil
        tree?.invalidateFlatOrder()
    }

    /// Children ordered for paint/hit-test, sorted by `zIndex` (stable by
    /// sibling order) when any child uses a non-zero zIndex. Memoised: the
    /// sort runs once per structural or zIndex change, not every frame.
    func renderOrderedChildren() -> [RenderObject] {
        if let cached = cachedZSortedChildren { return cached }
        let sorted = children.enumerated()
            .sorted { lhs, rhs in
                let lhsZ = lhs.element.node?.zIndex ?? 0
                let rhsZ = rhs.element.node?.zIndex ?? 0
                if lhsZ == rhsZ { return lhs.offset < rhs.offset }
                return lhsZ < rhsZ
            }
            .map { $0.element }
        cachedZSortedChildren = sorted
        return sorted
    }

    init(node: Node, tree: RenderTree) {
        self.node = node
        self.tree = tree
        self.elementID = node.id
        node.renderObject = self
        refreshLayerClassification()
    }

    /// Walk parents and mark every layer-root cache on the path invalid.
    /// Nested layer DrawLists are appended into their enclosing layer's cached
    /// DrawList, so invalidating only the nearest layer would leave ancestors
    /// free to reuse stale composite output.
    public func invalidateLayerChain() {
        var node: RenderObject? = self
        while let cur = node {
            if cur.isLayerRoot {
                cur.cacheInvalid = true
            }
            node = cur.parent
        }
    }

    /// Re-evaluate `isLayerRoot` from the bound `Node`'s current properties.
    /// Called after style mutations that could change layer-root status.
    public func refreshLayerClassification() {
        guard let node else {
            isLayerRoot = true
            return
        }
        // Root has no parent — always its own layer.
        if parent == nil {
            isLayerRoot = true
            return
        }
        if node.clipsToBounds { isLayerRoot = true; return }
        if node.opacity < 1 { isLayerRoot = true; return }
        // A shadow spreads outside the node's box, so it needs its own layer to
        // composite from — either layer does it.
        if !node.shadow.isEmpty || !node.ambientShadow.isEmpty { isLayerRoot = true; return }
        // Custom painters are natural retained-rendering boundaries. A changing
        // chart/text/control can then re-record its own small DrawList while
        // unchanged sibling painters keep their cached geometry.
        if node.draw != nil || node.overlayDraw != nil { isLayerRoot = true; return }
        isLayerRoot = false
    }
}

/// Retained render records and a memoised linear paint plan.
public final class RenderTree {

    public private(set) var root: RenderObject?

    /// Node identity → RenderObject map. Lookups are O(1).
    private var byNode: [ObjectIdentifier: RenderObject] = [:]

    struct PaintPlan {
        var objects: [RenderObject] = []
        var parents: [Int] = []
        /// Exclusive subtree bounds let cached or culled branches skip in O(1).
        var subtreeEnds: [Int] = []
    }
    private var flatOrderCache: PaintPlan?

    fileprivate func invalidateFlatOrder() { flatOrderCache = nil }

    public init() {}

    deinit {
        if let root { tearDown(root) }
    }

    public func renderObject(for node: Node) -> RenderObject? {
        byNode[ObjectIdentifier(node)]
    }

    /// Replace the root and rebuild the entire RenderObject mirror from
    /// `node`. Called by `ViewGraph.install(root:)`.
    public func install(rootNode: Node) {
        if let root { tearDown(root) }
        byNode.removeAll(keepingCapacity: true)
        let obj = RenderObject(node: rootNode, tree: self)
        byNode[ObjectIdentifier(rootNode)] = obj
        root = obj
        rebuildSubtree(parent: obj)
        flatOrderCache = nil
    }

    /// Re-mirror the children of a `Node` after structural reconcile. New
    /// children get fresh RenderObjects; reused children keep their existing
    /// object so any cached state survives.
    public func reconcileChildren(of parentNode: Node) {
        guard let parentObject = byNode[ObjectIdentifier(parentNode)] else { return }
        var existing: [ObjectIdentifier: RenderObject] = [:]
        for child in parentObject.children {
            if let n = child.node { existing[ObjectIdentifier(n)] = child }
        }
        var rebuilt: [RenderObject] = []
        rebuilt.reserveCapacity(parentNode.children.count)
        var seen: Set<ObjectIdentifier> = []
        for childNode in parentNode.children {
            let key = ObjectIdentifier(childNode)
            seen.insert(key)
            if let kept = existing[key] {
                kept.parent = parentObject
                rebuilt.append(kept)
            } else {
                let made = RenderObject(node: childNode, tree: self)
                made.parent = parentObject
                byNode[key] = made
                rebuilt.append(made)
                rebuildSubtree(parent: made)
            }
        }
        // Drop torn-down children (and their subtrees) from the registry.
        for (key, obj) in existing where !seen.contains(key) {
            tearDown(obj)
            _ = key
        }
        parentObject.children = rebuilt
        parentObject.invalidateZOrder()
        flatOrderCache = nil
        for obj in rebuilt {
            obj.refreshLayerClassification()
            // Cache invalidates whenever children list changes.
            obj.cacheInvalid = true
        }
        parentObject.cacheInvalid = true
    }

    /// Drop a single Node's RenderObject (and its descendants).
    public func tearDown(node: Node) {
        guard let obj = byNode[ObjectIdentifier(node)] else { return }
        if let parent = obj.parent,
           let idx = parent.children.firstIndex(where: { $0 === obj }) {
            parent.children.remove(at: idx)
            parent.cacheInvalid = true
            parent.invalidateZOrder()
        }
        tearDown(obj)
        if root === obj { root = nil }
    }

    private func tearDown(_ obj: RenderObject) {
        flatOrderCache = nil
        var pending = [obj]
        while let object = pending.popLast() {
            pending.append(contentsOf: object.children)
            object.children.removeAll(keepingCapacity: false)
            if let node = object.node {
                byNode.removeValue(forKey: ObjectIdentifier(node))
                if node.renderObject === object { node.renderObject = nil }
            }
        }
    }

    private func rebuildSubtree(parent: RenderObject) {
        var pending = [parent]
        while let object = pending.popLast() {
            guard let node = object.node else { continue }
            object.children = node.children.map { child in
                let made = RenderObject(node: child, tree: self)
                made.parent = object
                byNode[ObjectIdentifier(child)] = made
                return made
            }
            object.refreshLayerClassification()
            pending.append(contentsOf: object.children.reversed())
        }
    }

    // MARK: - Diagnostics

    /// Discard derived geometry without destroying live scene/state ownership.
    public func trimCaches() {
        for object in byNode.values {
            object.cachedLayerList = nil
            object.cachedPaintBounds = nil
            object.cacheInvalid = true
        }
    }

    /// Snapshot of every layer-root RenderObject in pre-order.
    public func layerRoots() -> [RenderObject] {
        guard let root else { return [] }
        var out: [RenderObject] = []
        var pending = [root]
        while let object = pending.popLast() {
            if object.isLayerRoot { out.append(object) }
            pending.append(contentsOf: object.children.reversed())
        }
        return out
    }

    /// Count of RenderObjects in the tree (testing/diagnostic).
    public var objectCount: Int { byNode.count }

    /// Flat, depth-first, parent-before-child list of every `RenderObject` in
    /// final paint order. Sibling groups are emitted in z-order (via
    /// `renderOrderedChildren`) so the result is directly drawable without a
    /// per-frame sort. Memoised; rebuilt only when `invalidateFlatOrder` runs
    /// (structural change, child membership/order, or a `zIndex` change).
    ///
    /// This is the precomputed draw-order table the renderer consumes to turn its
    /// per-frame recursive walk into a single linear scan.
    public func flatDrawOrder() -> [RenderObject] {
        paintPlan().objects
    }

    func paintPlan() -> PaintPlan {
        if let cached = flatOrderCache { return cached }
        var plan = PaintPlan()
        guard let root else { flatOrderCache = plan; return plan }
        plan.objects.reserveCapacity(byNode.count)
        plan.parents.reserveCapacity(byNode.count)
        plan.subtreeEnds.reserveCapacity(byNode.count)
        var pending: [(object: RenderObject, parent: Int, exit: Int?)] = [(root, -1, nil)]
        while let step = pending.popLast() {
            if let index = step.exit {
                plan.subtreeEnds[index] = plan.objects.count
                continue
            }
            let index = plan.objects.count
            plan.objects.append(step.object)
            plan.parents.append(step.parent)
            plan.subtreeEnds.append(index + 1)
            let children = (step.object.node?.childrenMayNeedZSort ?? false)
                ? step.object.renderOrderedChildren() : step.object.children
            if !children.isEmpty {
                pending.append((step.object, step.parent, index))
                for child in children.reversed() { pending.append((child, index, nil)) }
            }
        }
        flatOrderCache = plan
        return plan
    }
}
