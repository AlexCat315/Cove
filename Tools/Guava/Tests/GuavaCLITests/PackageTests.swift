import Foundation
import Testing
@testable import GuavaCLI

@Test func plainPackageTestsUseNativeCacheAndPreserveFilterAndSerialExecution() throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent("Swift package with spaces \(UUID())")
    defer { try? Files.remove(directory) }
    try Files.text("// Package", to: directory.appendingPathComponent("Package.swift"))
    let plan = try PackageTests(Arguments(["test", directory.path, "--filter", "ListRowTests", "--no-parallel", "--profile"]))
    #expect(plan.command.contains(directory.path))
    #expect(plan.command.contains("--no-parallel"))
    #expect(plan.command.suffix(3) == ["--filter", "ListRowTests", "--no-parallel"])
    #expect(!plan.command.contains("--scratch-path"))
    #expect(plan.command.contains("release") && plan.command.contains("-DGUAVA_PROFILE"))
    #expect(plan.environment["GUAVA_SDK_ROOT"].map { GuavaSDK.isSDK(URL(fileURLWithPath: $0)) } == true)
    #expect(!FileManager.default.fileExists(atPath: directory.appendingPathComponent("guava.json").path))
}

@Test func applicationTestsKeepTheirOwnedCacheAndValidateAuthoredManifest() throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? Files.remove(directory) }
    try Files.text("// Package", to: directory.appendingPathComponent("Package.swift"))
    let root = try GuavaSDK.locate(Arguments(["test"]))
    let configuration = ProjectConfiguration(application: .init(id: "com.example.tests", name: "Tests"),
                                             toolchain: .init(sdkVersion: try SDKRelease.read(at: root).version))
    try Files.json(configuration, to: directory.appendingPathComponent("guava.json"))
    let plan = try PackageTests(Arguments(["test", directory.path]))
    #expect(!plan.command.contains("--scratch-path"))
    #expect(plan.helpers.isEmpty)
    var withHelper = configuration
    withHelper.targets.desktopHelpers = ["GuavaPluginHost"]
    try Files.json(withHelper, to: directory.appendingPathComponent("guava.json"))
    let helpers = try PackageTests(Arguments(["test", directory.path])).helpers
    #expect(helpers.count == 1 && helpers[0].suffix(2) == ["--product", "GuavaPluginHost"])
    try Files.text("{}", to: directory.appendingPathComponent("guava.json"))
    #expect(throws: (any Error).self) { try PackageTests(Arguments(["test", directory.path])) }
}

@Test func packageTestsRejectMissingPackagesAndMisplacedSerialFlag() throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    #expect(throws: (any Error).self) { try PackageTests(Arguments(["test", directory.path])) }
    for args in [["run", "--no-parallel"], ["devtools", "--no-parallel"], ["test", "--no-parallel", "--no-parallel"]] {
        #expect(throws: (any Error).self) { try Arguments(args) }
    }
}

@Test func testPlansSeparateCompilationFromFrameworkExecution() throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? Files.remove(directory) }
    try Files.text("// Package", to: directory.appendingPathComponent("Package.swift"))
    let build = try PackageTests(Arguments(["test", directory.path, "--build-only"]))
    #expect(build.command[1] == "build" && build.command.contains("--build-tests"))
    let tests = try PackageTests(Arguments(["test", directory.path, "--skip-build", "--testing-library", "swift-testing", "--timeout", "30"]))
    #expect(tests.helpers.isEmpty && tests.command.contains("--skip-build"))
    #expect(tests.command.contains("--disable-xctest") && tests.timeout == 30)
    for options in [["--build-only", "--skip-build"], ["--testing-library", "unknown"], ["--timeout", "nan"], ["--timeout", "0"]] {
        #expect(throws: (any Error).self) { try PackageTests(Arguments(["test", directory.path] + options)) }
    }
}

@Test func androidHostManifestWorksFromAVersionNamedSDKDirectory() throws {
    var source = URL(fileURLWithPath: #filePath)
    for _ in 0..<5 { source.deleteLastPathComponent() }
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? Files.remove(directory) }
    let sdk = directory.appendingPathComponent("0.1.2")
    let host = sdk.appendingPathComponent("Hosts/Android")
    try Files.text(String(contentsOf: source.appendingPathComponent("Hosts/Android/Package.swift"), encoding: .utf8),
                   to: host.appendingPathComponent("Package.swift"))
    var environment = ProcessInfo.processInfo.environment
    environment.removeValue(forKey: "GUAVA_ANDROID_UI_PACKAGE")
    let output = try Shell.run([GuavaSDK.swift, "package", "--package-path", host.path, "dump-package"],
                               environment: environment, capture: true, quiet: true)
    let manifest = try #require(try JSONSerialization.jsonObject(with: Data(output.utf8)) as? [String: Any])
    #expect(manifest["name"] as? String == "GuavaAndroid")
}
