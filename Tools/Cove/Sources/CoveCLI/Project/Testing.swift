import Foundation
private import ToolSupport

/// Testing needs a package, not application identity or native launch targets.
/// Applications and ordinary packages use SwiftPM's standard .build directory.
struct PackageTests {
    let command: [String]
    /// Desktop helper builds, so tests that launch a helper never run a stale one.
    let helpers: [[String]]
    let environment: [String: String]
    let timeout: TimeInterval?

    init(_ arguments: Arguments) throws {
        let directory = arguments.directory
        guard FileManager.default.fileExists(atPath: directory.appendingPathComponent("Package.swift").path) else {
            throw ToolError.invalid("\(directory.path) must contain Package.swift to run cove test")
        }
        let project = FileManager.default.fileExists(atPath: directory.appendingPathComponent("cove.json").path)
            ? try AppProject(at: directory) : nil
        let root = try CoveSDK.locate(arguments, project: project)
        guard !arguments.flags.isSuperset(of: [.skipBuild, .buildOnly]) else {
            throw ToolError.invalid("Select either --build-only or --skip-build")
        }
        let scratchPath = arguments.url(.scratchPath)
        var command = SwiftPackage.command(directory: directory, configuration: arguments.configuration, verb: "test",
            scratchPath: scratchPath)
        if let filter = arguments.value(.filter) { command += ["--filter", filter] }
        if arguments.flags.contains(.noParallel) { command.append("--no-parallel") }
        let build = SwiftPackage.command(directory: directory, configuration: arguments.configuration, scratchPath: scratchPath)
        if arguments.flags.contains(.skipBuild) { command.append("--skip-build") }
        if let library = arguments.value(.testingLibrary) {
            switch library {
            case "xctest": command.append("--disable-swift-testing")
            case "swift-testing": command.append("--disable-xctest")
            default: throw ToolError.invalid("--testing-library must be xctest or swift-testing")
            }
        }
        if let events = arguments.url(.eventOutput) {
            try Files.directory(events.deletingLastPathComponent())
            command += ["--experimental-event-stream-output", events.path]
        }
        if let value = arguments.value(.timeout) {
            guard let seconds = TimeInterval(value), seconds.isFinite, seconds > 0, seconds <= 86_400 else {
                throw ToolError.invalid("--timeout must be between 0 and 86400 seconds")
            }
            timeout = seconds
        } else { timeout = nil }
        helpers = arguments.flags.contains(.skipBuild) ? []
            : (project?.configuration.targets.desktopHelpers ?? []).map { build + ["--product", $0] }
        if arguments.flags.contains(.buildOnly) { command = build + ["--build-tests"] }
        self.command = command; environment = try SwiftPackage.environment(root: root)
    }

    func run() throws {
        for helper in helpers { try Shell.run(helper, environment: environment) }
        let result = try ProcessRunner.run(command, environment: environment, timeout: timeout, stream: true)
        guard result.status == 0 else { throw ToolError.failed(command[0], result.status) }
    }
}
