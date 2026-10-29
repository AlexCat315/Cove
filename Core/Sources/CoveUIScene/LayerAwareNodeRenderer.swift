import CoveUICore
#if canImport(CoreGraphics)
import CoreGraphics
#endif
import Foundation

/// Executes a flat paint plan and records each layer-root subtree
/// into its own cached `DrawList`, then composites cached layer slices into
/// the frame `DrawList` by appending their vertices/indices.
///
/// Cache invariants per layer-root `RenderObject`:
/// - `cacheInvalid == false` AND
/// - the layer's placement at composite time matches the one captured when the
///   cache was recorded (`lastPlacement`: position, paint and shadow scales) AND
/// - the layer's parent clip stack at composite time matches the one captured
///   when the cache was recorded (tracked by `lastClipStack`).
///
/// Stable unclipped leaf layers may move under the same parent clip without
/// re-recording. Their cached vertices are translated while being appended;
/// this is the common path for text and controls moving during scrolling. That
/// shortcut needs the scale to be unchanged as well — translating vertices
/// cannot resize them, so a scaling layer has to re-record.
///
/// Visible geometry matches `NodeRenderer` for the same tree. Completely
/// clipped subtrees and cached offscreen leaves can be omitted. Cached layers
/// append as independent batch runs rather than coalescing across boundaries.
public final class LayerAwareNodeRenderer {

    public init() {}
    private var contentPlacements: [Affine2D] = []

    private final class OpenNode {
        let object: RenderObject
        let node: Node
        let end: Int
        let list: DrawList
        let destination: DrawList?
        let own: Affine2D
        let parentClips: [UIClip]
        let childClips: [UIClip]
        let allowsContainerCache: Bool
        let firstVertex: Int
        let descendantStart: Int
        let mask: UIRoundedRect?
        let clipped: Bool
        let inheritedClipRevision: UInt64

        init(object: RenderObject, node: Node, end: Int, list: DrawList, destination: DrawList?,
             own: Affine2D, parentClips: [UIClip], childClips: [UIClip],
             allowsContainerCache: Bool, firstVertex: Int, descendantStart: Int,
             mask: UIRoundedRect?, clipped: Bool, inheritedClipRevision: UInt64) {
            self.object = object; self.node = node; self.end = end
            self.list = list; self.destination = destination
            self.own = own
            self.parentClips = parentClips; self.childClips = childClips
            self.allowsContainerCache = allowsContainerCache
            self.firstVertex = firstVertex; self.descendantStart = descendantStart
            self.mask = mask; self.clipped = clipped; self.inheritedClipRevision = inheritedClipRevision
        }
    }

    /// Execute the cached preorder plan. Exit records preserve painter, mask,
    /// overlay and opacity ordering without using the call stack.
    public func render(tree: RenderTree, into frame: DrawList) {
        guard let node = tree.root?.node else { return }
        frame.setViewportBounds(UIRect(x: Float(node.frame.origin.x), y: Float(node.frame.origin.y),
                                       width: Float(node.frame.width), height: Float(node.frame.height)))
        let plan = tree.paintPlan()
        // Keep the hot buffer locally owned, avoiding a property access for
        // every node. A reentrant render can borrow its own independent buffer.
        var placements: [Affine2D] = []
        swap(&placements, &contentPlacements)
        defer { contentPlacements = placements }
        if placements.count != plan.objects.count {
            placements = Array(repeating: .identity, count: plan.objects.count)
        }
        var open: [OpenNode] = []
        var index = 0
        while index < plan.objects.count || !open.isEmpty {
            let parent = open.last
            if let parent, index >= parent.end {
                finish(open.removeLast())
                continue
            }
            let object = plan.objects[index]
            let end = plan.subtreeEnds[index]
            guard let node = object.node, node.isLayoutVisible else { index = end; continue }
            let destination = parent?.list ?? frame
            let parentIndex = plan.parents[index]
            let placed = parentIndex >= 0 ? placements[parentIndex] : .identity
            let parentClips = parent?.childClips ?? []
            let allowsCache = parent?.allowsContainerCache ?? true
            let own = node.placementStep(scrolling: .zero).followed(by: placed)
            let bounds = UIRect(x: own.e, y: own.f,
                                width: Float(node.frame.width) * own.a, height: Float(node.frame.height) * own.a)
            if node.clipsToBounds, bounds.width > 0, bounds.height > 0,
               RenderCulling.isOutside(bounds, list: destination) { index = end; continue }

            if !object.children.isEmpty {
                placements[index] = node.contentOffset == .zero ? own :
                    node.placementStep(scrolling: node.contentOffset).followed(by: placed)
            }
            // Pure structural wrappers contribute only placement. They need
            // neither a paint record nor an exit operation.
            if !object.isLayerRoot, node.opacity == 1, node.subtreeOpacity == 1,
               node.backgroundColor == nil, node.backgroundGradient == nil,
               node.borderColor == nil || node.borderWidth <= 0, node.revealRequest == nil {
                index += 1
                continue
            }

            var list = destination
            var cacheDestination: DrawList?
            var childAllowsCache = allowsCache
            var inheritedClipRevision: UInt64 = 0
            if object.isLayerRoot {
                let placementUnchanged = object.lastPlacement == own
                let clipUnchanged = object.lastClipStack == parentClips
                if reuse(object, node: node, into: destination, own: own, clips: parentClips,
                         placementUnchanged: placementUnchanged, clipUnchanged: clipUnchanged) {
                    index = end
                    continue
                }
                let deferContainer = !object.children.isEmpty &&
                    (!allowsCache || object.cacheInvalid || !placementUnchanged || !clipUnchanged)
                if !deferContainer {
                    list = DrawList()
                    list.pixelScale = destination.pixelScale
                    list.geometryAntialiasing = destination.geometryAntialiasing
                    list.setViewportBounds(destination.viewportBounds)
                    for clip in parentClips { list.pushClip(clip) }
                    inheritedClipRevision = list.clipRevision
                    cacheDestination = destination
                    childAllowsCache = false
                }
            }
            open.append(begin(object, node: node, end: end, into: list, destination: cacheDestination,
                              own: own, parentClips: parentClips,
                              allowsContainerCache: childAllowsCache, inheritedClipRevision: inheritedClipRevision))
            index += 1
        }
    }

    /// A cache hit also handles offscreen leaves: its whole subtree is skipped.
    private func reuse(_ object: RenderObject, node: Node, into list: DrawList, own: Affine2D,
                       clips: [UIClip], placementUnchanged: Bool, clipUnchanged: Bool) -> Bool {
        guard !object.cacheInvalid, clipUnchanged, let cached = object.cachedLayerList,
              cached.pixelScale == list.pixelScale,
              cached.geometryAntialiasing == list.geometryAntialiasing else { return false }
        if object.children.isEmpty, let bounds = object.cachedPaintBounds,
           let old = object.lastPlacement, own.a == old.a, own.shadowScale == old.shadowScale {
            let moved = UIRect(x: bounds.x + own.e - old.e, y: bounds.y + own.f - old.f,
                               width: bounds.width, height: bounds.height)
            if RenderCulling.isOutside(moved, list: list) { return true }
        }
        if placementUnchanged { list.appendPreclipped(cached, clips: clips); return true }
        if object.children.isEmpty, !node.clipsToBounds, !object.cachedLayerUsesLocalClips,
           let old = object.lastPlacement, own.a == old.a, own.shadowScale == old.shadowScale,
           !Self.crossesRoundedCorner(object.cachedPaintBounds, from: old, to: own, clips: clips) {
            list.appendPreclipped(cached, clips: clips,
                                  vertexTranslationX: own.e - old.e, vertexTranslationY: own.f - old.f)
            return true
        }
        return false
    }

    private func begin(_ object: RenderObject, node: Node, end: Int, into list: DrawList,
                       destination: DrawList?, own: Affine2D, parentClips: [UIClip],
                       allowsContainerCache: Bool, inheritedClipRevision: UInt64) -> OpenNode {
        let firstVertex = list.vertices.count
        let scale = own.a, x = own.e, y = own.f
        let width = Float(node.frame.width) * scale, height = Float(node.frame.height) * scale
        let radius = node.cornerRadius * scale, borderWidth = node.borderWidth * scale
        if let reveal = node.revealRequest { list.beginReveal(reveal) }
        let clipped = node.clipsToBounds && width > 0 && height > 0
        let clip = UIRect(x: x, y: y, width: width, height: height)
        if clipped { list.pushClip(clip) }
        emitShadow(node, into: list, x: x, y: y, w: width, h: height,
                   radius: radius, scale: own.shadowScale)
        let frosted = node.backdropBlur > 0 && node.backgroundColor != nil
        if !frosted {
            emitBorder(node, into: list, x: x, y: y, w: width, h: height, radius: radius, borderWidth: borderWidth)
        }
        emitBackground(node, into: list, x: x, y: y, w: width, h: height, radius: radius, borderWidth: borderWidth)
        if frosted {
            emitBorder(node, into: list, x: x, y: y, w: width, h: height, radius: radius, borderWidth: borderWidth)
        }
        let mask = contentMask(node, x: x, y: y, width: width, height: height,
                               radius: radius, borderWidth: borderWidth)
        if let mask { list.pushMask(mask) }
        if let draw = node.draw {
            paintScaled(draw, into: list, origin: CGPoint(x: Double(x), y: Double(y)), scale: scale)
        }
        return OpenNode(object: object, node: node, end: end, list: list, destination: destination,
                        own: own,
                        parentClips: parentClips, childClips: clipped ? parentClips + [UIClip(clip, mask: mask)] : parentClips,
                        allowsContainerCache: allowsContainerCache, firstVertex: firstVertex,
                        descendantStart: list.vertices.count, mask: mask, clipped: clipped,
                        inheritedClipRevision: inheritedClipRevision)
    }

    private func finish(_ record: OpenNode) {
        let list = record.list, object = record.object, node = record.node
        if record.mask != nil { list.popMask() }
        list.multiplyOpacity(fromVertexIndex: record.descendantStart, by: node.opacity)
        if let overlay = node.overlayDraw {
            paintScaled(overlay, into: list, origin: CGPoint(x: Double(record.own.e), y: Double(record.own.f)),
                        scale: record.own.a)
        }
        list.multiplyOpacity(node.subtreeOpacity, fromVertex: record.firstVertex)
        if record.clipped { list.popClip() }
        guard object.isLayerRoot else { return }
        object.lastPlacement = record.own
        object.lastClipStack = record.parentClips
        object.cacheInvalid = false
        guard let destination = record.destination else {
            object.cachedLayerList = nil
            return
        }
        for clip in record.parentClips.reversed() { list.popClip(clip) }
        object.cachedLayerList = list
        object.cachedLayerUsesLocalClips = list.clipRevision != record.inheritedClipRevision
        object.cachedPaintBounds = object.children.isEmpty ? RenderCulling.bounds(of: list) : nil
        if let bounds = object.cachedPaintBounds, RenderCulling.isOutside(bounds, list: destination) { return }
        destination.appendPreclipped(list, clips: record.parentClips)
    }

    /// A cached leaf was clipped where it was recorded. If the mask cut any of
    /// it there, or it would touch a rounded corner after moving, re-record it
    /// instead of translating geometry that was cut for the old position. A
    /// leaf recorded entirely outside the mask kept its vertices but lost every
    /// triangle, so moving it into view would paint nothing.
    private static func crossesRoundedCorner(_ bounds: UIRect?, from old: Affine2D, to new: Affine2D, clips: [UIClip]) -> Bool {
        let masks = clips.compactMap(\.mask).filter { $0.radius > 0 }
        guard !masks.isEmpty else { return false }
        guard let bounds else { return true }
        let moved = UIRect(x: bounds.x + new.e - old.e, y: bounds.y + new.f - old.f, width: bounds.width, height: bounds.height)
        return masks.contains { !$0.contains(bounds) || $0.touchesCorner(moved) }
    }

    // MARK: - Painter helpers (mirror NodeRenderer.swift)

    private func emitShadow(_ node: Node, into list: DrawList,
                            x: Float, y: Float, w: Float, h: Float,
                            radius: Float, scale: Float) {
        let rect = UIRect(x: x, y: y, width: w, height: h)
        // An opaque background is painted over this node immediately after, so
        // the shadow's interior is never visible. Clipping to it keeps the
        // fringe and drops the wasted fills.
        let occluded = node.backgroundColor != nil ? rect : nil
        list.addShadow(LayeredShadow(ambient: node.ambientShadow.scaled(by: scale),
                                    key: node.shadow.scaled(by: scale)),
                       rect: rect, cornerRadius: radius, opacity: node.opacity,
                       frostedRect: node.backdropBlur > 0 ? rect : nil,
                       occludedRect: occluded)
    }

    private func emitBorder(_ node: Node, into list: DrawList,
                            x: Float, y: Float, w: Float, h: Float,
                            radius: Float, borderWidth: Float) {
        guard let bc = node.borderColor, borderWidth > 0,
              w > 0, h > 0 else { return }
        let rect = UIRect(x: x, y: y, width: w, height: h)
        let color = applyOpacity(bc, node.opacity)
        list.addRoundedRectStroke(rect, radius: radius,
                                  width: borderWidth, color: color)
    }

    private func emitBackground(_ node: Node, into list: DrawList,
                                x: Float, y: Float, w: Float, h: Float,
                                radius: Float, borderWidth: Float) {
        guard node.backgroundColor != nil || node.backgroundGradient != nil, w > 0, h > 0 else { return }
        let inset: Float = (node.borderColor != nil && node.borderWidth > 0) ? borderWidth : 0
        let bgWidth = max(0, w - 2 * inset)
        let bgHeight = max(0, h - 2 * inset)
        guard bgWidth > 0 && bgHeight > 0 else { return }
        let rect = UIRect(x: x + inset, y: y + inset,
                          width: bgWidth, height: bgHeight)
        let bgRadius = max(0, radius - inset)
        // The backdrop covers the whole node, not just the fill: the border
        // ring is drawn on top of it and must stay crisp.
        if node.backdropBlur > 0 {
            list.beginBackdrop(UIBackdrop(rect: UIRect(x: x, y: y, width: w, height: h),
                                          cornerRadius: radius,
                                          blurRadius: node.backdropBlur, opacity: node.opacity))
        }
        if let gradient = node.backgroundGradient {
            list.addLinearGradient(rect, radius: bgRadius, gradient: gradient.multipliedAlpha(node.opacity))
        } else if let bg = node.backgroundColor {
            let color = applyOpacity(bg, node.opacity)
            if bgRadius > 0 {
                list.addRoundedRect(rect, radius: bgRadius, color: color)
            } else {
                list.addRect(rect, color: color)
            }
        }
    }

    private func applyOpacity(_ color: Color, _ opacity: Float) -> Color {
        guard opacity < 1 else { return color }
        return Color(r: color.r, g: color.g, b: color.b, a: color.a * opacity)
    }
}
