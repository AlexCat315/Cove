import Testing
@testable import AppContent

@Test func welcomeResourceIsPackaged() {
    #expect(AppResources.welcome == "Welcome to CoveUI · 你好，CoveUI")
}
