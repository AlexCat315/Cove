import CoveAndroidContract
import Foundation
import CoveUIAndroid
import CoveUIRuntime
import CoveUICompose
import NativeRHI
#if COVE_ANDROID_UI
import AppContent
#endif

/// Hosts application or showcase content.
@MainActor
final class HostedScene {
    struct Bounds: Codable {
        let x: Float
        let y: Float
        let width: Float
        let height: Float
        init(_ rect: CGRect) {
            x = Float(rect.minX); y = Float(rect.minY)
            width = Float(rect.width); height = Float(rect.height)
        }
    }
    struct Snapshot: Codable {
        let session: String
        let state: [String: String]
        let focused: String?
        let preedit: String
        let controls: [String: Bounds]
        let width: Float
        let height: Float
        let scale: Float
        let typography: TypographyStatistics
        let vertices: Int
        let indices: Int
        let frames: Int
        let mode: String
        let page: String?
        let editor: TextInputClient.Snapshot?
        let nodes: [SemanticNode]
    }
    let launch: SceneLaunch
    #if COVE_ANDROID_SHOWCASE
    private let showcase: ShowcaseContent?
    #endif
    let content: AndroidContentSession
    private var drawing = DrawList()
    private var frames = 0
    private let sessionID = UUID().uuidString

    var list: DrawList { drawing }
    var size: SIMD2<Float> { content.logicalSize }
    var scale: Float { content.scale }
    var needsFrame: Bool { content.needsFrame }
    var focusedID: String? { content.focusedID }
    var hasTextFocus: Bool { content.editorSnapshot != nil || focusedID == "counter.note" }
    var fontAtlas: FontAtlas {
        #if COVE_ANDROID_SHOWCASE
        if let atlas = showcase?.typography?.fonts.atlas { return atlas }
        #endif
        return content.fontAtlas
    }
    private var typographyStatistics: TypographyStatistics {
        #if COVE_ANDROID_SHOWCASE
        return showcase?.typography?.statistics ?? TypographyStatistics()
        #else
        return TypographyStatistics()
        #endif
    }
    private var currentPage: String? {
        #if COVE_ANDROID_SHOWCASE
        return showcase?.page
        #else
        return nil
        #endif
    }
    var editorSnapshot: TextInputClient.Snapshot? {
        if let editor = content.editorSnapshot { return editor }
        guard hasTextFocus else { return nil }
        let value = checkpointState["note", default: ""], preedit = content.preedit
        let caret = value.utf16.count + preedit.utf16.count
        return .init(text: value + preedit, selectionStart: caret, selectionEnd: caret,
                     compositionStart: preedit.isEmpty ? -1 : value.utf16.count, compositionEnd: preedit.isEmpty ? -1 : caret)
    }

    init(checkpoint: String, launch: SceneLaunch, scale: Float, device: Device) throws {
        self.launch = launch
        #if COVE_ANDROID_SHOWCASE
        showcase = try [.gallery, .counter].contains(launch.mode) ? ShowcaseContent(launch: launch, checkpoint: checkpoint) : nil
        #endif
        let root: AnyView
        switch launch.mode {
        case .gallery, .counter:
            #if COVE_ANDROID_SHOWCASE
            root = showcase!.root(scale: scale)
            #else
            throw RHIError.unsupportedFeature("showcase content is not linked into application APKs")
            #endif
        case .application:
            #if COVE_ANDROID_UI
            root = AnyView(LayerRoot { makeApplicationContent() })
            #else
            throw RHIError.unsupportedFeature("build this UI application's APK with cove build PATH --android")
            #endif
        case .game: throw RHIError.unsupportedFeature("game content is hosted by Diffuse")
        }
        content = AndroidContentSession(scale: scale, rendering: launch.mode == .counter ? .direct : .layers) { root }
    }

    func prepare(pixels: SIMD2<Int>, scale: Float, deltaTime: Double) {
        #if COVE_ANDROID_SHOWCASE
        let typography = showcase?.typography
        typography?.beginFrame()
        #endif
        drawing = content.prepare(pixels: pixels, scale: scale, deltaTime: deltaTime)
        #if COVE_ANDROID_SHOWCASE
        if typography?.fonts.atlas.isFull == true {
            typography?.fonts.atlas.reset()
            typography?.beginFrame()
            drawing = content.prepare(pixels: pixels, scale: scale, deltaTime: 0)
        }
        typography?.finishFrame()
        #endif
    }

    func didPresent() { frames += 1; content.didPresent() }
    func invalidate() { content.invalidate() }
    func suspend() { content.suspend() }
    func pointer(kind: Int32, x: Float, y: Float) { content.pointer(kind: kind, x: x, y: y) }
    func text(_ string: String, composing: Bool) { content.text(string, composing: composing) }
    func edit(kind: Int32, start: Int, end: Int) { content.edit(kind: kind, start: start, end: end) }
    func scroll(x: Float, y: Float, deltaX: Float, deltaY: Float) { content.scroll(x: x, y: y, deltaX: deltaX, deltaY: deltaY) }
    func key(_ scancode: UInt32, modifiers: KeyModifiers = []) {
        content.key(scancode, modifiers: modifiers)
    }
    func snapshot() -> String {
        let inspection = content.inspect()
        let controls = inspection.controls.mapValues(Bounds.init)
        let value = Snapshot(session: sessionID, state: checkpointState,
            focused: focusedID,
            preedit: content.preedit, controls: controls,
            width: size.x, height: size.y, scale: scale, typography: typographyStatistics,
            vertices: list.vertices.count, indices: list.indices.count, frames: frames,
            mode: launch.mode.rawValue, page: currentPage,
            editor: editorSnapshot, nodes: semanticNodes(inspection.semantics))
        guard let data = try? JSONEncoder().encode(value) else { return "{}" }
        return String(decoding: data, as: UTF8.self)
    }

    private var checkpointState: [String: String] {
        #if COVE_ANDROID_SHOWCASE
        return showcase?.checkpoint ?? [:]
        #else
        return [:]
        #endif
    }
}
