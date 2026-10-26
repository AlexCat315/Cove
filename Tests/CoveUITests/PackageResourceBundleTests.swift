import Testing
@testable import CoveUI

@Suite("CoveUI resources")
struct CoveUIResourceBundleTests {
    @Test("window chrome icons resolve through the packaged resource locator")
    func windowChromeIconResolves() {
        #expect(CoveUIResourceBundle.bundle.url(
            forResource: "close",
            withExtension: "svg"
        ) != nil)
    }
}
