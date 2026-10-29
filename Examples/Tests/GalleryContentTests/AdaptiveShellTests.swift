import Foundation
import CoveUICompose
import CoveUIRuntime
import Testing

@testable import GalleryContent

@Suite("Gallery adaptive shell", .serialized)
@MainActor
struct AdaptiveShellTests {
  @Test("Page switches leave the adaptive shell and unrelated sidebar destinations untouched")
  func pageSwitchIsolation() throws {
    try scene { gallery, graph, _, _ in
      let all = nodes(graph.tree.root)
      let shellNode = all.first { $0.viewTag == String(reflecting: GalleryView.self) }
      let shell = try #require(shellNode)
      let beforeShell = shell.recompositionMetrics?.count
      let sizeControl = try named("gallery-size", graph)
      var before: [ElementID: UInt64] = [:]
      for node in all where node.viewTag?.hasPrefix("CoveUICompose.SidebarDestination<") == true {
        before[node.id] = node.recompositionMetrics?.count ?? 0
      }
      #expect(before.count > 80)
      let renderer = LayerAwareNodeRenderer()
      // Match a running application: the shell already has retained geometry.
      for _ in 0..<2 { renderer.render(tree: graph.renderTree, into: DrawList()) }
      graph.tree.flush()
      let start = ContinuousClock.now
      gallery.selectPage(.slider)
      let commitStart = ContinuousClock.now
      graph.recomposer.commitAll()
      let commitEnd = ContinuousClock.now
      graph.computeLayoutIfNeeded(width: 1280, height: 820)
      let layoutEnd = ContinuousClock.now
      renderer.render(tree: graph.renderTree, into: DrawList())
      let drawEnd = ContinuousClock.now
      settle(graph, width: 1280)
      let elapsed = start.duration(to: .now).components
      var changed: [Node] = []
      for node in nodes(graph.tree.root) {
        if let previous = before[node.id], (node.recompositionMetrics?.count ?? 0) > previous {
          changed.append(node)
        }
      }
      #expect(shell.recompositionMetrics?.count == beforeShell)
      #expect(try named("gallery-size", graph) === sizeControl)
      #expect(changed.count <= 3)
      #expect(gallery.currentPage == .slider)
      #expect(try named("sidebar-item-slider", graph).accessibility?.state.isSelected == true)
      print("COVE_ROUTE_SWITCH rows=\(changed.count)/\(before.count) settleMs=\(Double(elapsed.seconds) * 1_000 + Double(elapsed.attoseconds) / 1e15)")
      func milliseconds(_ start: ContinuousClock.Instant, _ end: ContinuousClock.Instant) -> Double {
        let parts = start.duration(to: end).components
        return Double(parts.seconds) * 1_000 + Double(parts.attoseconds) / 1e15
      }
      print("COVE_ROUTE_FIRST_FRAME commitMs=\(milliseconds(commitStart, commitEnd)) layoutMs=\(milliseconds(commitEnd, layoutEnd)) drawMs=\(milliseconds(layoutEnd, drawEnd))")
    }
  }

  private func nodes(_ root: Node?) -> [Node] {
    var pending = root.map { [$0] } ?? []
    var result: [Node] = []
    while let node = pending.popLast() {
      result.append(node)
      pending.append(contentsOf: node.children.reversed())
    }
    return result
  }
  private func named(_ name: String, _ graph: ViewGraph) throws -> Node {
    try #require(
      nodes(graph.tree.root).first {
        $0.attachments[LayoutDebugAttachmentKey.debugName] as? String == name
      })
  }
  private func control(_ label: String, _ graph: ViewGraph) throws -> Node {
    try #require(
      nodes(graph.tree.root).first {
        $0.accessibility?.label == label && $0.isFocusable && $0.acceptsSubtreeInput
      })
  }
  private func scene(
    _ work: (GalleryView, ViewGraph, PlatformInputContext, EventDispatcher) throws -> Void
  ) rethrows {
    let previous = TextEnvironmentHolder.current
    TextEnvironmentHolder.current = .bootstrapped(
      atlasTextureID: 1, primaryFontName: SystemFontDefaults.primaryFontName)
    defer { TextEnvironmentHolder.current = previous }
    let context = PlatformInputContext()
    let scheduler = AnimatorScheduler()
    let portals = PortalStore()
    context.addScopedAmbient(PortalStoreAmbient(portals))
    try context.withCurrent {
      try AnimatorScheduler.$current.withValue(scheduler) {
        let gallery = GalleryView(initialPage: .button)
        let graph = ViewGraph(tree: NodeTree(), recomposer: Recomposer())
        let input = EventDispatcher(
          tree: graph.tree, interactions: context.interactions,
          capture: context.pointerCapture, focusChain: context.focusChain)
        graph.install(root: gallery.reducedMotion())
        settle(graph, width: 1280)
        defer {
          graph.install(root: EmptyView())
          scheduler.reset()
        }
        try work(gallery, graph, context, input)
      }
    }
  }
  private func settle(_ graph: ViewGraph, width: Float) {
    for _ in 0..<10 {
      graph.recomposer.commitAll()
      graph.computeLayout(width: width, height: 820)
    }
  }
  private func click(_ node: Node, _ input: EventDispatcher) {
    let frame = node.absoluteFrame
    let event = MouseButtonEvent(
      button: .left, x: Float(frame.midX), y: Float(frame.midY), clicks: 1)
    input.dispatch(.mouseButtonDown(event))
    input.dispatch(.mouseButtonUp(event))
  }

  @Test("390, 720 and 1280 layouts keep the story and toolbar usable without resetting local state")
  func reflow() throws {
    try scene { _, graph, context, input in
      let primary = try named("gallery-primary-button", graph)
      click(primary, input)
      settle(graph, width: 1280)
      let row = try named("sidebar-item-button", graph)
      let size = try named("gallery-size", graph)
      for width: Float in [390, 720, 1280, 390] {
        settle(graph, width: width)
        #expect(primary.id == (try named("gallery-primary-button", graph)).id)
        #expect(row.id == (try named("sidebar-item-button", graph)).id)
        #expect(size.id == (try named("gallery-size", graph)).id)
        #expect(nodes(graph.tree.root).contains { $0.layoutNode?.textInputs?.text == "Actions: 1" })
        let scroll = try named("gallery-content", graph)
        #expect(scroll.frame.height > 250 && scroll.absoluteFrame.maxX <= CGFloat(width))
        #expect(scroll.absoluteFrame.minX == (width < 600 ? 0 : width < 1000 ? 56 : 247))
        for name in ["gallery-size", "gallery-text-scale", "gallery-reset"] {
          let frame = try named(name, graph).absoluteFrame
          #expect(frame.minX >= 0 && frame.maxX <= CGFloat(width))
          #expect(frame.maxY <= scroll.absoluteFrame.minY)
        }
        #expect(row.acceptsSubtreeInput == (width >= 600))
      }
      settle(graph, width: 1280)
      context.focusChain.focus(try control("Search components…", graph))
      settle(graph, width: 720)
      #expect(context.focusChain.focused?.id == row.id)
      #expect(!graph.recomposer.hasPending)
    }
  }

  @Test(
    "Compact and rail open the retained search; selection closes the drawer and themes update in both shells"
  )
  func navigationSession() throws {
    try scene { gallery, graph, context, input in
      let search = try control("Search components…", graph)
      settle(graph, width: 390)
      click(try control("Open navigation", graph), input)
      settle(graph, width: 390)
      #expect(context.focusChain.hasModalScope)
      #expect(search.id == (try control("Search components…", graph)).id)
      context.focusChain.focus(search)
      input.dispatch(.textInput("slider"))
      settle(graph, width: 390)
      #expect(search.resolvedAccessibilityValue == "slider")
      settle(graph, width: 1280)
      #expect(!context.focusChain.hasModalScope && search.resolvedAccessibilityValue == "slider")
      settle(graph, width: 720)
      // The open navigation session survives the expanded interval.
      #expect(context.focusChain.hasModalScope)
      click(try named("sidebar-item-slider", graph), input)
      settle(graph, width: 720)
      #expect(gallery.currentPage == .slider && !context.focusChain.hasModalScope)
      #expect(search.resolvedAccessibilityValue == "slider")
      let railTheme = try named("gallery-theme-rail", graph)
      click(railTheme, input)
      settle(graph, width: 720)
      #expect(gallery.isDark)
      click(try control("Open navigation", graph), input)
      settle(graph, width: 720)
      #expect(search.id == (try control("Search components…", graph)).id)
      click(try named("gallery-theme", graph), input)
      settle(graph, width: 720)
      #expect(!gallery.isDark)
    }
  }
}
