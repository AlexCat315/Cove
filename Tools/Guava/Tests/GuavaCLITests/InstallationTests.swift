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
