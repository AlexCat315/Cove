import Testing
import CoveUICore

@Test func nativeTextScaleInterpolatesTheSystemRamp() {
    let scale = UIEnvironment.TextScale(samples: [
        .init(size: 16, scaledSize: 32), .init(size: 32, scaledSize: 48)
    ])
    #expect(scale.factor == 2)
    #expect(scale.scale(16) == 32)
    #expect(scale.scale(24) == 40)
    #expect(scale.scale(8) == 16)
    #expect(scale.scale(64) == 96)
    #expect(scale.scale(-24) == -40)
    #expect(scale.scale(0) == 0)
}

@Test func invalidTextScaleAndInsetsHaveDeterministicDefaults() {
    for factor: Float in [.nan, .infinity, -.infinity, 0, -1] {
        #expect(UIEnvironment.TextScale(factor).scale(16) == 16)
    }
    for samples: [UIEnvironment.TextScale.Sample] in [[],
        [.init(size: .nan, scaledSize: 1)],
        [.init(size: 16, scaledSize: 32), .init(size: 16, scaledSize: 48)],
        [.init(size: 16, scaledSize: 32), .init(size: 32, scaledSize: 16)]] {
        #expect(UIEnvironment.TextScale(samples: samples).factor == 1)
    }
    var safe = UIEnvironment.Insets()
    safe.top = .nan; safe.leading = -.infinity; safe.bottom = -10; safe.trailing = 14
    #expect(safe.normalized.top == 0 && safe.normalized.leading == 0 && safe.normalized.bottom == 0)
    #expect(safe.normalized.trailing == 14)
    #expect(UIEnvironment.Keyboard(height: .infinity).height == 0)
}
