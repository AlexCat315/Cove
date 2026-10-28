import Testing
@testable import CoveUICompose

@Suite("CoveUICompose resources")
struct CoveUIComposeResourceBundleTests {
    @Test("built-in icons resolve outside SwiftPM's generated accessor")
    func builtInIconResolves() {
        #expect(UIIcons.Navigation.chevronDown.url != nil)
        #expect(UIIcons.Action.check.url != nil)
        // Dropdown triggers draw the wider arrow mygo draws rather than the
        // navigation chevron, so both variants must ship.
        #expect(UIIcons.Navigation.dropdownChevronDown.url != nil)
        #expect(UIIcons.Navigation.dropdownChevronUp.url != nil)
    }
}
