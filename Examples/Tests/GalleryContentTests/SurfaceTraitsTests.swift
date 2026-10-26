import CoveUICompose
import CoveUIRuntime
import Testing
@testable import GalleryContent

@Suite("Host surface traits", .serialized)
@MainActor
struct SurfaceTraitsTests {
    @Test("The gallery reports host facts without installing a simulated device")
    func reportsHostFacts() throws {
        let previous = TextEnvironmentHolder.current
        TextEnvironmentHolder.current = .bootstrapped(atlasTextureID: 1, primaryFontName: SystemFontDefaults.primaryFontName)
        defer { TextEnvironmentHolder.current = previous }
        let graph = ViewGraph(tree: NodeTree(), recomposer: Recomposer())
        defer { graph.install(root: EmptyView()) }
        var surface = UIEnvironment()
        surface.idiom = .phone
        surface.viewport.width = 393; surface.viewport.height = 852
        surface.safeArea.top = 59; surface.safeArea.bottom = 34
        surface.keyboard = .init(height: 336); surface.textScale = .init(1.3)
        graph.install(root: SurfaceRoot(surface) { ViewportTraitsStory() })
        for _ in 0..<8 { graph.recomposer.commitAll(); graph.computeLayout(width: 393, height: 852) }
        func nodes(_ node: Node) -> [Node] { [node] + node.children.flatMap(nodes) }
        let all = try nodes(#require(graph.tree.root))
        let strings = all.compactMap { $0.layoutNode?.textInputs?.text }
        #expect(strings.contains("Idiom: phone"))
        #expect(strings.contains("Safe area: 59 / 0 / 34 / 0"))
        #expect(strings.contains("Text scale: 130%   ·   Keyboard: visible"))
        #expect(!all.contains { ($0.attachments[LayoutDebugAttachmentKey.debugName] as? String)?.hasPrefix("phone-preview") == true })
    }
}
