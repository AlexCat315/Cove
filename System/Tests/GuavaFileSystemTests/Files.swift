private import Foundation
private import Testing
@testable import GuavaFileSystem
#if os(Windows)
private import WinSDK
#endif

@Suite("Filesystem contracts")
struct FileSystemTests {
    @Test func replacementPublishesAndPreservesDestinationOnFailure() throws {
        let root = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let destination = root.appendingPathComponent("file"), staging = root.appendingPathComponent("staging")
        try AtomicFile.write(Data("old".utf8), to: destination)
        try Data("new".utf8).write(to: staging)
        try FileReplacement.replace(destination, with: staging)
        #expect(try String(contentsOf: destination, encoding: .utf8) == "new")
        #expect(!FileManager.default.fileExists(atPath: staging.path))
        #expect(throws: (any Error).self) { try FileReplacement.replace(destination, with: staging) }
        #expect(try String(contentsOf: destination, encoding: .utf8) == "new")
        #expect(throws: (any Error).self) { try FileReplacement.replace(destination, with: destination) }
        #expect(try String(contentsOf: destination, encoding: .utf8) == "new")
    }

    @Test func failedAtomicWriteCleansStagingFiles() throws {
        let root = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let destination = root.appendingPathComponent("directory")
        try FileManager.default.createDirectory(at: destination, withIntermediateDirectories: false)
        try Data("keep".utf8).write(to: destination.appendingPathComponent("child"))
        #expect(throws: (any Error).self) { try AtomicFile.write(Data("new".utf8), to: destination) }
        #expect(try String(contentsOf: destination.appendingPathComponent("child"), encoding: .utf8) == "keep")
        #expect(try FileManager.default.contentsOfDirectory(atPath: root.path) == ["directory"])
    }

    @Test func ownerOnlyFilesRemainReadableAndWritable() throws {
        let root = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        try FilePermissions.restrictToCurrentUser(root)
        let file = root.appendingPathComponent("secret")
        try AtomicFile.write(Data("secret".utf8), to: file)
        try FilePermissions.restrictToCurrentUser(file)
        #expect(try String(contentsOf: file, encoding: .utf8) == "secret")
        try AtomicFile.write(Data("updated".utf8), to: file, access: .ownerOnly)
        #expect(try String(contentsOf: file, encoding: .utf8) == "updated")
        #if !os(Windows)
        let mode = try FileManager.default.attributesOfItem(atPath: file.path)[.posixPermissions] as? NSNumber
        #expect(mode?.intValue == 0o600)
        let directoryMode = try FileManager.default.attributesOfItem(atPath: root.path)[.posixPermissions] as? NSNumber
        #expect(directoryMode?.intValue == 0o700)
        #else
        var acl: UnsafeMutablePointer<ACL>?
        var descriptor: UnsafeMutableRawPointer?
        let status = file.path.withCString(encodedAs: UTF16.self) {
            GetNamedSecurityInfoW(UnsafeMutablePointer(mutating: $0), SE_FILE_OBJECT,
                DWORD(DACL_SECURITY_INFORMATION), nil, nil, &acl, nil, &descriptor)
        }
        defer { if let descriptor { _ = LocalFree(descriptor) } }
        #expect(status == 0)
        #expect(acl?.pointee.AceCount == 1)
        var control: SECURITY_DESCRIPTOR_CONTROL = 0
        var revision: DWORD = 0
        #expect(GetSecurityDescriptorControl(descriptor, &control, &revision))
        #expect(control & UInt16(SE_DACL_PROTECTED) != 0)
        #endif
    }

    #if os(Windows)
    @Test func sharingViolationPreservesOldFileAndCleansStaging() throws {
        let root = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let file = root.appendingPathComponent("locked")
        try AtomicFile.write(Data("old".utf8), to: file)
        let handle = file.path.withCString(encodedAs: UTF16.self) {
            CreateFileW($0, DWORD(GENERIC_READ), DWORD(FILE_SHARE_READ | FILE_SHARE_WRITE),
                        nil, DWORD(OPEN_EXISTING), DWORD(FILE_ATTRIBUTE_NORMAL), nil)
        }
        #expect(handle != INVALID_HANDLE_VALUE)
        defer { _ = CloseHandle(handle) }
        #expect(throws: (any Error).self) { try AtomicFile.write(Data("new".utf8), to: file) }
        #expect(try String(contentsOf: file, encoding: .utf8) == "old")
        #expect(try FileManager.default.contentsOfDirectory(atPath: root.path) == ["locked"])
    }
    #endif

    private func temporaryDirectory() throws -> URL {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("guava-system-\(UUID())")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        return root
    }
}
