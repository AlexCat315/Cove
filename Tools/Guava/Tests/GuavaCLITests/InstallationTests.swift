import Foundation
import Testing
@testable import GuavaCLI

@Test func sdkSelectionIsSeparateFromApplicationConfiguration() throws {
    let base = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? Files.remove(base) }
    let sdk = base.appendingPathComponent("SDK with spaces")
    try makeSDK(at: sdk)
    let store = SDKStore(directory: base.appendingPathComponent("user"))
    #expect(store.selected == nil)
    try store.use(sdk)
    #expect(store.selected?.path == sdk.path)
    #expect(throws: (any Error).self) { try store.use(base) }
    #expect(store.selected?.path == sdk.path)
}

@Test func ordinaryApplicationLauncherIsGeneratedWithoutAuthoredEntryPoint() throws {
    let base = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? Files.remove(base) }
    let arguments = try Arguments(["create", "app", base.path, "--platforms", "mac"])
    try CreateApplication.run(arguments, root: try GuavaSDK.locate(Arguments(["test"])))
    let project = try AppProject(at: base)
    #expect(!FileManager.default.fileExists(atPath: base.appendingPathComponent("Sources/DesktopApp").path))
    let authored = try Data(contentsOf: base.appendingPathComponent("Package.swift"))
    let generated = try DesktopLauncher(project: project, root: URL(fileURLWithPath: "/SDK with spaces")).prepare()
    #expect(generated.path.hasPrefix(project.buildDirectory.path))
    #expect(project.desktopProduct == "Application")
    #expect(try String(contentsOf: generated.appendingPathComponent("Sources/Application/main.swift"), encoding: .utf8).contains("makeApplicationContent()"))
    #expect(try Data(contentsOf: base.appendingPathComponent("Package.swift")) == authored)
}

@Test func installationCommandsRequireSDKVersionAndTargetSelection() throws {
    let install = try Arguments(["sdk", "install", "0.1.0"])
    #expect(install.operation == .sdkInstall)
    #expect(install.subject == "0.1.0")
    #expect(try Arguments(["setup", "--android"]).platform == .android)
    #expect(throws: (any Error).self) { try Arguments(["sdk", "list", "unexpected"]) }
}

func makeSDK(at root: URL, version: String = "0.1.0") throws {
    try Files.text("// Package", to: root.appendingPathComponent("Package.swift"))
    try Files.json(SDKRelease(version: version, swift: "6.4.1", repository: "https://github.com/AlexCat315/GuavaUI.git"), to: root.appendingPathComponent(SDKRelease.manifest))
}

func makeProject(at directory: URL, sdkVersion: String) throws {
    try Files.text("// Package", to: directory.appendingPathComponent("Package.swift"))
    let configuration = ProjectConfiguration(application: .init(id: "com.example.test", name: "Test App"),
                                             toolchain: .init(sdkVersion: sdkVersion))
    try Files.json(configuration, to: directory.appendingPathComponent("guava.json"))
}

/// Clears GUAVA_SDK_ROOT around a body so the explicit env override cannot
/// shadow the isolated store; the suite is serialized because the process
/// environment is shared.
private func withoutSDKRoot(_ body: () throws -> Void) throws {
    let oldRoot = ProcessInfo.processInfo.environment["GUAVA_SDK_ROOT"]
    unsetenv("GUAVA_SDK_ROOT")
    defer {
        if let oldRoot { setenv("GUAVA_SDK_ROOT", oldRoot, 1) } else { unsetenv("GUAVA_SDK_ROOT") }
    }
    try body()
}

@Suite(.serialized)
struct SDKLocationTests {
    @Test func latestPinFloatsToNewestInstalledSDK() throws {
        let base = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? Files.remove(base) }
        let store = SDKStore(directory: base.appendingPathComponent("home"))
        try makeSDK(at: store.installed("0.1.9"), version: "0.1.9")
        try makeSDK(at: store.installed("0.1.10"), version: "0.1.10")
        // A stale selection must not override "latest".
        try store.use(store.installed("0.1.9"))
        let project = base.appendingPathComponent("App")
        try makeProject(at: project, sdkVersion: SDKRelease.latestTag)

        try withoutSDKRoot {
            let located = try GuavaSDK.locate(Arguments(["build"]),
                                              project: try AppProject(at: project), store: store)
            #expect(try SDKRelease.read(at: located).version == "0.1.10")
        }
    }

    @Test func latestPinWithoutInstalledSDKIsActionable() throws {
        let base = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? Files.remove(base) }
        let store = SDKStore(directory: base.appendingPathComponent("home"))
        try Files.directory(store.directory)
        let project = base.appendingPathComponent("App")
        try makeProject(at: project, sdkVersion: SDKRelease.latestTag)

        try withoutSDKRoot {
            #expect(throws: (any Error).self) {
                _ = try GuavaSDK.locate(Arguments(["build"]),
                                        project: try AppProject(at: project), store: store)
            }
        }
    }

    @Test func exactPinStillRequiresMatchingVersion() throws {
        let base = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? Files.remove(base) }
        let store = SDKStore(directory: base.appendingPathComponent("home"))
        try makeSDK(at: store.installed("0.1.9"), version: "0.1.9")
        let project = base.appendingPathComponent("App")
        // 9.9.9 exists neither in the isolated store nor as the running dev checkout.
        try makeProject(at: project, sdkVersion: "9.9.9")

        try withoutSDKRoot {
            #expect(throws: (any Error).self) {
                _ = try GuavaSDK.locate(Arguments(["build"]),
                                        project: try AppProject(at: project), store: store)
            }
        }
    }
}

@Test func latestIsAValidManifestRequirement() throws {
    let latest = ProjectConfiguration(application: .init(id: "com.example.test", name: "Test App"),
                                      toolchain: .init(sdkVersion: SDKRelease.latestTag))
    try latest.validate()
    #expect(SDKRelease.isVersionRequirement("latest"))
    #expect(SDKRelease.isVersionRequirement("0.1.10"))
    #expect(!SDKRelease.isVersionRequirement("v0.1.10"))
}
