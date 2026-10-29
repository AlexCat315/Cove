import Testing
import Foundation
import CoveUICore
import CovePlatformCore
@testable import CoveUICompose

/// Geometry that drives selection handles, the magnifier and hit-testing. These
/// run with no window or font atlas, so they exercise the pure math directly.
@Suite("Text Selection Geometry")
struct TextSelectionGeometryTests {

    private func sampleLayout() -> TextLayoutResult {
        let glyphs = [
            PositionedGlyph(glyphID: 0, fontID: 0, cluster: 0, x: 0, y: 0, atlasInfo: nil),
            PositionedGlyph(glyphID: 1, fontID: 0, cluster: 1, x: 10, y: 0, atlasInfo: nil),
            PositionedGlyph(glyphID: 2, fontID: 0, cluster: 2, x: 20, y: 0, atlasInfo: nil),
        ]
        let line = TextLine(glyphs: glyphs, baselineY: 10, topY: 0, width: 30, startCluster: 0, endCluster: 3)
        return TextLayoutResult(lines: [line], totalWidth: 30, totalHeight: 12)
    }

    @Test("offset(at:) maps a point to the nearest text offset")
    func offsetAt() {
        let layout = sampleLayout()
        #expect(layout.offset(at: CGPoint(x: 0, y: 6)) == 0)
        #expect(layout.offset(at: CGPoint(x: 25, y: 6)) == 3)
        // Midway between the first two glyphs resolves to the later cluster.
        #expect(layout.offset(at: CGPoint(x: 5, y: 6)) == 1)
    }

    @Test("selectionRects(_:) covers the selected byte range")
    func selectionRects() {
        let layout = sampleLayout()
        let rects = layout.selectionRects(0..<3)
        #expect(rects.count == 1)
        let r = rects[0]
        #expect(abs(r.origin.x - 0) < 0.001)
        #expect(abs(r.width - 30) < 0.001)
        #expect(abs(r.height - 12) < 0.001)
        #expect(layout.selectionRects(0..<0).isEmpty)
    }

    @Test("caretRect(at:) marks an endpoint for the drag handles")
    func caretRect() {
        let layout = sampleLayout()
        let start = layout.caretRect(at: 0)
        #expect(abs(start.origin.x - 0) < 0.001)
        let end = layout.caretRect(at: 3)
        #expect(abs(end.origin.x - 20) < 0.001)
        #expect(abs(end.height - 12) < 0.001)
    }

    @Test("TextBoundaries.word selects the word under an offset")
    func wordBoundary() {
        let first = TextBoundaries.word(in: "hello world", at: 3)
        #expect(first == 0..<5)
        let second = TextBoundaries.word(in: "hello world", at: 8)
        #expect(second == 6..<11)
    }

    @Test("PointerKind round-trips and events carry a source kind")
    func pointerKind() throws {
        let data = try JSONEncoder().encode(PointerKind.touch)
        #expect(try JSONDecoder().decode(PointerKind.self, from: data) == .touch)

        let down = MouseButtonEvent(button: .left, x: 1, y: 2, clicks: 1, kind: .touch)
        #expect(down.kind == .touch)
        let move = MouseMotionEvent(x: 1, y: 2, deltaX: 0, deltaY: 0, kind: .pen)
        #expect(move.kind == .pen)
        // Default source is a mouse when none is supplied (keeps legacy behaviour).
        let plain = MouseButtonEvent(button: .left, x: 0, y: 0, clicks: 1)
        #expect(plain.kind == .mouse)
    }
}
