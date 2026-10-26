import Testing
@testable import CoveUIWorkspace

@Suite("CoveUIWorkspace resources")
struct CoveUIWorkspaceResourceBundleTests {
    @Test("workspace icons resolve through the packaged resource locator")
    func workspaceIconResolves() {
        #expect(CoveUIWorkspaceResourceBundle.bundle.url(
            forResource: "collapse",
            withExtension: "svg"
        ) != nil)
    }
}
