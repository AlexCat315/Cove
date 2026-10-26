import Foundation

enum ToolchainSetup {
    static func report(_ arguments: Arguments, root: URL) throws {
        try arguments.platform.requireAvailable()
        print("Cove SDK: \(root.path)\nHost: \(AppPlatform.host.rawValue); target: \(arguments.platform.rawValue)")
        let checks = ToolchainRequirements.inspect(arguments, root: root)
        for check in checks { print("[\(check.ready ? "OK" : "MISSING")] \(check.name)") }
        let remedies = checks.filter { !$0.ready }.map(\.remedy)
        for remedy in Array(Set(remedies)).sorted() { print("\n\(remedy)") }
        guard remedies.isEmpty else { throw ToolError.invalid("Complete the target-specific steps above, then rerun cove setup.") }
    }

    static func run(_ arguments: Arguments, root: URL) throws {
        if arguments.platform == .android {
            _ = try AndroidToolchain(root: root, arguments: arguments)
            try AndroidToolchain.ensureSwiftSDK()
        }
        try report(arguments, root: root)
        if arguments.platform == .ios {
            try NativeSDK.prepare(root: root, apple: .init(sdk: arguments.value(.sdk) ?? "iphonesimulator", architecture: arguments.value(.architecture) ?? "arm64"))
        } else if arguments.platform == .android {
            _ = try NativeDependencies(toolchain: AndroidToolchain(root: root, arguments: arguments)).prepare()
        } else {
            // SDK native archives are built once, shared by all application projects.
            try NativeSDK.prepare(root: root)
        }
        print("Ready. cove create app MyApp; cove run MyApp --\(arguments.platform.rawValue)")
    }
}
