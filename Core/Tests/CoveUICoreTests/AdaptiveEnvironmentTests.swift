import Testing
import CoveUICore

@Test func surfaceAdaptsToSpaceAndInputIndependently() {
    var environment = UIEnvironment()
    #expect(environment.viewport.widthClass == .expanded)
    environment.viewport.width = 390
    #expect(environment.viewport.widthClass == .compact)
    environment.viewport.width = 768
    environment.input = [.touch, .keyboard]
    #expect(environment.viewport.widthClass == .medium)
    #expect(environment.minimumControlExtent == 44 && !environment.supportsHover)
    environment.input = [.pointer, .keyboard]
    #expect(environment.minimumControlExtent == 28 && environment.supportsHover)
    environment.input = [.touch, .pointer]
    #expect(environment.minimumControlExtent == 44 && environment.supportsHover)
}

@Test func idiomIsAHostFactRatherThanAWidthGuess() {
    var environment = UIEnvironment()
    #expect(environment.idiom == .desktop)
    environment.viewport.width = 390
    #expect(environment.idiom == .desktop)
    environment.idiom = .phone
    environment.viewport.width = 932
    #expect(environment.idiom == .phone)
    environment.idiom = .tablet
    #expect(environment.idiom != .phone)
    #expect(UIEnvironment.Idiom.allCases == [.phone, .tablet, .desktop])
}

@Test func textScaleAndKeyboardStaySeparateFromTheSafeArea() {
    var environment = UIEnvironment()
    #expect(environment.textScale.factor == 1)
    #expect(!environment.keyboard.isVisible)
    environment.safeArea.bottom = 34
    environment.keyboard.height = 336
    environment.textScale = .init(1.3)
    #expect(environment.keyboard.isVisible)
    #expect(environment.safeArea.bottom == 34)
    #expect(environment.textScale.factor == 1.3)
}
