import Foundation
import Testing
@testable import CoveCLI

private func temporaryProject() throws -> URL {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent("cove-cli-test-" + UUID().uuidString)
    try Files.directory(directory)
    try Files.text("// Swift package", to: directory.appendingPathComponent("Package.swift"))
    let configuration = ProjectConfiguration(application: .init(id: "com.example.test", name: "Test App"),
                                             toolchain: .init(sdkVersion: "0.1.0"))
    try Files.json(configuration, to: directory.appendingPathComponent("cove.json"))
    return directory
}

@Test func manifestDefaultsAndRoundTrip() throws {
    let data = Data(#"{"application":{"id":"com.example.app","name":"App"},"toolchain":{"sdkVersion":"0.1.0"},"version":{}}"#.utf8)
    let configuration = try JSONDecoder().decode(ProjectConfiguration.self, from: data)
    try configuration.validate()
    #expect(configuration.targets.platforms == [.mac, .win, .linux, .android, .ios])
    #expect(configuration.targets.desktopExecutable == nil)
    #expect(configuration.targets.desktopHelpers.isEmpty)
    let encoded = try JSONEncoder().encode(configuration)
    #expect(try JSONDecoder().decode(ProjectConfiguration.self, from: encoded) == configuration)
    #expect(!String(decoding: encoded, as: UTF8.self).contains("device"))
}

@Test(arguments: ["com", "Com.example.app", "com.example.my_app", "com.example..app", "com.example.123"])
func invalidApplicationIdentifiersFail(id: String) {
    #expect(throws: (any Error).self) { try ApplicationIdentity(id: id, name: "App").validate() }
}

@Test func cleanPreservesAuthoredData() throws {
    let directory = try temporaryProject()
    defer { try? Files.remove(directory) }
    let project = try AppProject(at: directory)
    try Files.text("authored", to: directory.appendingPathComponent(".cove/scene.json"))
    try Files.text("cache", to: project.buildDirectory.appendingPathComponent("swift/object.o"))
    try Files.text("old cache", to: directory.appendingPathComponent(".cove/build/swift/object.o"))
    try project.clean()
    #expect(!FileManager.default.fileExists(atPath: project.buildDirectory.path))
    #expect(!FileManager.default.fileExists(atPath: directory.appendingPathComponent(".cove/build").path))
    #expect(try String(contentsOf: directory.appendingPathComponent(".cove/scene.json"), encoding: .utf8) == "authored")
    try project.clean()
}

@Test func cleanRejectsSymlinkedCache() throws {
    let directory = try temporaryProject()
    let other = try temporaryProject()
    defer { try? Files.remove(directory); try? Files.remove(other) }
    try Files.directory(directory.appendingPathComponent(".cove"))
    try FileManager.default.createSymbolicLink(at: directory.appendingPathComponent(".build"), withDestinationURL: other)
    let project = try AppProject(at: directory)
    #expect(throws: (any Error).self) { try project.clean() }
    #expect(FileManager.default.fileExists(atPath: other.appendingPathComponent("cove.json").path))
}

@Test func createRefusesToOverwriteAnExistingProject() throws {
    let directory = try temporaryProject()
    defer { try? Files.remove(directory) }
    let arguments = try Arguments(["create", "app", directory.path])
    #expect(throws: (any Error).self) { try CreateApplication.run(arguments, root: directory) }
}

@Test func cleanLeavesAuthoredCoveDirectoryUntouched() throws {
    let directory = try temporaryProject()
    let other = try temporaryProject()
    defer { try? Files.remove(directory); try? Files.remove(other) }
    try Files.text("authored", to: other.appendingPathComponent("build/scene.json"))
    try FileManager.default.createSymbolicLink(at: directory.appendingPathComponent(".cove"), withDestinationURL: other)
    try AppProject(at: directory).clean()
    #expect(FileManager.default.fileExists(atPath: other.appendingPathComponent("build/scene.json").path))
}
