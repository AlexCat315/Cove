import Foundation
import GuavaUIRuntime

/// Main-thread owner of an offscreen view tree. Rendering returns an owned
/// snapshot that can be recorded later on the renderer's thread.
public final class OffscreenView {

    private let tree = NodeTree()
    private let recomposer = Recomposer()
    private let graph: ViewGraph
    private let layerRenderer = LayerAwareNodeRenderer()
    private let drawList = DrawList()
    private let atlasTextureID: TextureID
    private let imageAssets = ImageAssetRegistry()

    private var textEnv: TextEnvironment?
    private var lastScale: Float = 0
    private var didInstallRoot = false

    public init(atlasTextureID: TextureID = 1) {
        self.atlasTextureID = atlasTextureID
        self.graph = ViewGraph(tree: tree, recomposer: recomposer)
    }

    // MARK: - Main-thread API

    /// Install a GuavaUI `View` tree as the in-game HUD. Call once before the
    /// first `tick`. Subsequent calls are ignored — swap the root via `@State`
    /// on a wrapping view if dynamic root changes are needed.
    public func setRootView<V: View>(_ view: V) {
        guard !didInstallRoot else { return }
        ensureTextEnvironment(scale: 1)
        withTextEnvInstalled {
            graph.install(root: view)
        }
        didInstallRoot = true
    }

    /// Advance the in-game UI by one frame. Must be called on the main thread,
    /// after all observed state
    /// writes for this frame have been applied.
    ///
    /// Recomposes dirty scopes, runs Yoga layout, renders the node tree into a
    /// `DrawList`, snapshots the result, for the recording thread.
    public func render(width: Int, height: Int, contentScale: Float = 1) -> DrawListSnapshot? {
        guard width > 0, height > 0, contentScale.isFinite, contentScale > 0 else { return nil }
        if !didInstallRoot { setRootView(EmptyView()) }
        ensureTextEnvironment(scale: contentScale)

        withTextEnvInstalled {
            _ = recomposer.commitAll()
            WindowRevealContext.clearActivation(in: graph.tree.root)
            _ = graph.computeLayoutIfNeeded(width: Float(width), height: Float(height))
            drawList.reset()
            drawList.pixelScale = Float(contentScale)
            layerRenderer.render(tree: graph.renderTree, into: drawList)
        }

        var atlasUpdates: [DrawListAtlasDirty] = []
        if let env = textEnv, env.atlas.isDirty {
            if let payload = env.atlas.dirtyUploadPayload() {
                atlasUpdates.append(DrawListAtlasDirty(
                    pixels: payload.pixels,
                    regionX: UInt32(payload.region.x),
                    regionY: UInt32(payload.region.y),
                    regionWidth: UInt32(payload.region.width),
                    regionHeight: UInt32(payload.region.height),
                    textureWidth: UInt32(env.atlas.atlasWidth),
                    textureHeight: UInt32(env.atlas.atlasHeight),
                    textureID: atlasTextureID
                ))
            }
            if let payload = env.atlas.colorDirtyUploadPayload() {
                atlasUpdates.append(DrawListAtlasDirty(
                    pixels: payload.pixels,
                    regionX: UInt32(payload.region.x), regionY: UInt32(payload.region.y),
                    regionWidth: UInt32(payload.region.width), regionHeight: UInt32(payload.region.height),
                    textureWidth: UInt32(payload.textureWidth), textureHeight: UInt32(payload.textureHeight),
                    textureID: atlasTextureID.colorGlyphAtlasID, format: .color))
            }
            env.atlas.markClean()
        }

        return DrawListSnapshot(
            vertices: drawList.vertices,
            indices: drawList.indices,
            batches: drawList.batches,
            logicalSize: SIMD2(Float(width), Float(height)),
            atlasUpdates: atlasUpdates,
            resources: drawList.resources
        )
    }

    // MARK: - Private

    private func ensureTextEnvironment(scale: Float) {
        let s = max(1, scale)
        guard abs(s - lastScale) > 0.01 else { return }
        lastScale = s
        textEnv = TextEnvironment.bootstrapped(
            atlasTextureID: atlasTextureID,
            primaryFontName: SystemFontDefaults.primaryFontName,
            defaultFont: .system(size: 16),
            defaultLineHeight: 20,
            defaultColor: .white,
            rasterScale: s,
            atlasEdge: max(512, Int((512 * s).rounded(.up)))
        )
        // Cached layer geometry contains UVs from the old atlas. Repaint and
        // remeasure text even if canvas commands and logical size did not change.
        tree.root?.invalidateTextStyle()
    }

    /// Run `body` with this bridge's `TextEnvironment` installed as the
    /// process-wide current env, then restore whatever was there before.
    private func withTextEnvInstalled(_ body: () -> Void) {
        let previousEnv = TextEnvironmentHolder.current
        let previousScale = ContentScaleHolder.current
        let previousImages = ImageAssetRegistryHolder.current
        TextEnvironmentHolder.current = textEnv
        ImageAssetRegistryHolder.current = imageAssets
        ContentScaleHolder.current = lastScale > 0 ? lastScale : 1
        body()
        TextEnvironmentHolder.current = previousEnv
        ImageAssetRegistryHolder.current = previousImages
        ContentScaleHolder.current = previousScale
    }
}
