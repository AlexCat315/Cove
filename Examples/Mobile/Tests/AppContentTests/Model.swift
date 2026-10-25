import Testing
@testable import AppContent

@Test func sampleProjectsCoverEveryKindOfWork() {
    #expect(Set(MobileModel().projects.map(\.kind)) == ["Game", "Animation", "Video", "Code"])
}
