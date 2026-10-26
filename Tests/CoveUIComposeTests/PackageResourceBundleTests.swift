import Testing
@testable import CoveUICompose

@Suite("CoveUICompose resources")
struct CoveUIComposeResourceBundleTests {
    @Test("built-in icons resolve outside SwiftPM's generated accessor")
    func builtInIconResolves() {
        #expect(UIIcons.Navigation.chevronDown.url != nil)
        #expect(UIIcons.Action.check.url != nil)
    }
}
