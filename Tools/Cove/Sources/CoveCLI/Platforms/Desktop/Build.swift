internal import ToolSupport
import Foundation

struct DesktopBuild {
    let project: AppProject
    let root: URL
    let arguments: Arguments

    func run() throws {
        let binaries = try compile()
        let artifact = try DesktopPackage(project: project, platform: arguments.platform)
            .assemble(binaries: binaries, configuration: arguments.configuration, output: arguments.url(.output))
        try NativeDesktopResources(project: project, platform: arguments.platform).sign(artifact, configuration: arguments.configuration)
        print("Built \(arguments.platform.rawValue) application: \(artifact.directory.path)")
        if arguments.operation == .run {
            try DesktopRun.run(artifact.executable, products: binaries, project: project, root: root, arguments: arguments)
        }
    }

    private func compile() throws -> URL {
        try NativeSDK.prepare(root: root)
        let package = try DesktopLauncher(project: project, root: root).prepare()
        let command = SwiftPackage.command(directory: package, configuration: arguments.configuration,
            scratchPath: project.buildDirectory)
        try Shell.run(command + ["--product", project.desktopProduct], environment: SwiftPackage.environment(root: root))
        for helper in project.configuration.targets.desktopHelpers {
            try Shell.run(command + ["--product", helper], environment: SwiftPackage.environment(root: root))
        }
        return URL(fileURLWithPath: try Shell.run(command + ["--show-bin-path"], environment: SwiftPackage.environment(root: root), capture: true))
    }
}

enum SwiftPackage {
    static func environment(root: URL) throws -> [String: String] {
        var environment = ProcessInfo.processInfo.environment
        environment["COVE_SDK_ROOT"] = root.path
        #if os(Linux)
        environment["COVE_NATIVE_LINK_ORDER"] = "1"
        #endif
        #if os(Windows)
        try WindowsBuildEnvironment.activate(&environment)
        #endif
        return environment
    }

    static func command(directory: URL, configuration: BuildConfiguration, verb: String = "build", scratchPath: URL? = nil) -> [String] {
        var command = [CoveSDK.swift, verb, "--package-path", directory.path,
                       "--configuration", configuration.swiftConfiguration, "--jobs", "4"] + configuration.swiftFlags
        #if os(macOS)
        command += ["--build-system", "swiftbuild"]
        #else
        command += ["--build-system", "native"]
        #endif
        if let scratchPath { command += ["--scratch-path", scratchPath.path] }
        return command
    }
}
