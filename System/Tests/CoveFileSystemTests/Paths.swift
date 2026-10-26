private import Foundation
private import Testing
@testable import CoveFileSystem

@Suite("Path identity contracts")
struct PathTests {
    @Test func missingChildrenAndSiblingPrefixes() {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("cove-path-\(UUID())")
        let child = root.appendingPathComponent("missing/file")
        #expect(PathIdentity.contains(child, in: root))
        #expect(!PathIdentity.contains(root, in: root, includingRoot: false))
        #expect(!PathIdentity.contains(URL(fileURLWithPath: root.path + "-other/file"), in: root))
        #expect(PathIdentity.relativePath(child, to: root) == "missing/file")
    }

    @Test func ancestorsTerminateAtFilesystemRoot() {
        let root = FileManager.default.temporaryDirectory
        let ancestors = PathIdentity.ancestors(of: root)
        #expect(!ancestors.isEmpty && ancestors.count < 100)
        #expect(Set(ancestors.map(\.path)).count == ancestors.count)
        let last = ancestors.last!
        #expect(PathIdentity.ancestors(of: last) == [last])
        #if !os(Windows)
        #expect(last.deletingLastPathComponent().standardizedFileURL.path == last.path)
        #endif
    }

    #if os(Windows)
    @Test func driveAndExtendedPathsHaveTheSameIdentity() {
        let path = FileManager.default.temporaryDirectory.appendingPathComponent("missing")
        let extended = URL(fileURLWithPath: "\\\\?\\" + path.path.replacingOccurrences(of: "/", with: "\\"))
        #expect(PathIdentity.sameLocation(path, extended))
        #expect(PathIdentity.ancestors(of: URL(fileURLWithPath: "C:\\" )).count == 1)
        let network = PathIdentity.ancestors(of: URL(fileURLWithPath: "\\\\server\\share\\folder"))
        #expect(network.count == 2, "UNC ancestors: \(network.map(\.path))")
    }
    #else
    @Test func symlinkEscapeAndMissingTargetAreResolved() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("cove-path-\(UUID())")
        defer { try? FileManager.default.removeItem(at: root) }
        try FileManager.default.createDirectory(at: root.appendingPathComponent("project"), withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: root.appendingPathComponent("outside"), withIntermediateDirectories: true)
        let link = root.appendingPathComponent("project/link")
        try FileManager.default.createSymbolicLink(at: link, withDestinationURL: root.appendingPathComponent("outside"))
        #expect(!PathIdentity.contains(link.appendingPathComponent("missing"), in: root.appendingPathComponent("project")))
        #expect(PathIdentity.sameLocation(link.appendingPathComponent("missing"), root.appendingPathComponent("outside/missing")))
    }
    #endif
}
