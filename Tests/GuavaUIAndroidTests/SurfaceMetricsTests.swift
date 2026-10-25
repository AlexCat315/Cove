import Foundation
import Testing
import GuavaUIAndroid
import GuavaUICompose
import GuavaUIRuntime

/// Reports the surface facts it received and exposes the area left after
/// safe-area and keyboard insets inside `SafeArea`.
private struct MetricsProbe: View {
    var body: some View {
        SurfaceReader { surface in
            SafeArea {
                Box {
                    Box {}.frame(width: 1, height: 1).debugName("probe-idiom-\(surface.idiom)")
                    Box {}.frame(width: 1, height: 1).debugName("probe-scale-\(surface.textScale.factor)")
                    Box {}.frame(width: 1, height: 1).debugName("probe-keyboard-\(Int(surface.keyboard.height))")
                    Box {}.flex(1).debugName("probe-fill")
                }
                .flex(1, shrink: 1, basis: 0)
                .frame(minWidth: 0, minHeight: 0)
                .debugName("probe-content")
            }
        }
    }
}

@Test @MainActor func androidSurfaceMetricsReachCompositionAndPadding() throws {
    let session = AndroidContentSession(scale: 1) { MetricsProbe() }
    var safeArea = UIEnvironment.Insets()
    safeArea.top = 59
    safeArea.bottom = 34
    session.updateSurface(idiom: .phone, safeArea: safeArea, keyboard: .init(height: 336), textScale: .init(1.25))
    session.prepare(pixels: SIMD2(393, 852), scale: 1, deltaTime: 0)

    let controls = session.inspect().controls
    #expect(controls["probe-idiom-phone"] != nil)
    #expect(controls["probe-scale-1.25"] != nil)
    #expect(controls["probe-keyboard-336"] != nil)
    let content = try #require(controls["probe-content"])
    #expect(abs(content.minY - 59) <= 0.5)
    #expect(abs(content.height - (852 - 59 - 336)) <= 0.5)
}

@Test @MainActor func hidingTheKeyboardRestoresTheSafeAreaFloor() throws {
    let session = AndroidContentSession(scale: 1) { MetricsProbe() }
    var safeArea = UIEnvironment.Insets()
    safeArea.top = 20
    safeArea.bottom = 34
    session.updateSurface(idiom: .tablet, safeArea: safeArea, keyboard: .init(height: 300), textScale: .init(1))
    session.prepare(pixels: SIMD2(744, 1000), scale: 1, deltaTime: 0)

    session.updateSurface(idiom: .tablet, safeArea: safeArea, keyboard: .init(height: 0), textScale: .init(2))
    session.prepare(pixels: SIMD2(744, 1000), scale: 1, deltaTime: 1 / 60.0)

    let controls = session.inspect().controls
    #expect(controls["probe-idiom-tablet"] != nil)
    #expect(controls["probe-scale-2.0"] != nil)
    #expect(controls["probe-keyboard-0"] != nil)
    let content = try #require(controls["probe-content"])
    #expect(abs(content.height - (1000 - 20 - 34)) <= 0.5)
}

@Test @MainActor func androidSurfaceFactsSurviveRotationAndDensityChanges() throws {
    let session = AndroidContentSession(scale: 2) { MetricsProbe() }
    var safe = UIEnvironment.Insets()
    safe.top = 24; safe.bottom = 16; safe.leading = 10
    let text = UIEnvironment.TextScale(samples: [.init(size: 16, scaledSize: 24), .init(size: 32, scaledSize: 40)])
    session.updateSurface(idiom: .phone, safeArea: safe, keyboard: .init(height: 200), textScale: text)
    session.prepare(pixels: SIMD2(800, 1600), scale: 2, deltaTime: 0)
    session.didPresent()
    #expect(!session.needsFrame)
    session.updateSurface(idiom: .phone, safeArea: safe, keyboard: .init(height: 200), textScale: text)
    #expect(!session.needsFrame)

    session.suspend()
    session.prepare(pixels: SIMD2(2400, 1200), scale: 3, deltaTime: 0)
    #expect(session.logicalSize == SIMD2(800, 400))
    #expect(session.scale == 3)
    let controls = session.inspect().controls
    #expect(controls["probe-idiom-phone"] != nil)
    #expect(controls["probe-scale-1.5"] != nil)
    #expect(controls["probe-keyboard-200"] != nil)
    let content = try #require(controls["probe-content"])
    #expect(content.minX == 10 && content.minY == 24)
    #expect(content.height == 176)
}

@Test @MainActor func androidRejectsInvalidMetricsBeforePublishingThem() throws {
    let session = AndroidContentSession(scale: .nan) { MetricsProbe() }
    var safe = UIEnvironment.Insets()
    safe.top = .nan; safe.bottom = -.infinity; safe.leading = -20; safe.trailing = 12
    session.updateSurface(idiom: .phone, safeArea: safe, keyboard: .init(height: .infinity), textScale: .init(.nan))
    session.prepare(pixels: SIMD2(400, 800), scale: .nan, deltaTime: 0)
    let controls = session.inspect().controls
    #expect(controls["probe-scale-1.0"] != nil && controls["probe-keyboard-0"] != nil)
    let content = try #require(controls["probe-content"])
    #expect(content.minX == 0 && content.minY == 0 && content.width == 388 && content.height == 800)
}
