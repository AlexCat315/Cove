import Foundation

enum ToolchainSetup {
    static func report(_ arguments: Arguments, root: URL) throws {
        try arguments.platform.requireAvailable()
        print("Guava SDK: \(root.path)\nHost: \(AppPlatform.host.rawValue); target: \(arguments.platform.rawValue)")
        let checks = ToolchainRequirements.inspect(arguments, root: root)
        for check in checks { print("[\(check.ready ? "OK" : "MISSING")] \(check.name)") }
        let remedies = checks.filter { !$0.ready }.map(\.remedy)
        for remedy in Array(Set(remedies)).sorted() { print("\n\(remedy)") }
        guard remedies.isEmpty else { throw ToolError.invalid("Complete the target-specific steps above, then rerun guava setup.") }
    }

    static func run(_ arguments: Arguments, root: URL) throws {
        try report(arguments, root: root)
        if arguments.platform == .ios {
            try NativeSDK.prepare(root: root, apple: .init(sdk: "iphonesimulator", architecture: "arm64"))
        } else if arguments.platform != .android {
            // SDK native archives are built once, shared by all application projects.
            try NativeSDK.prepare(root: root)
        }
        #if os(Windows)
        if ProcessInfo.processInfo.environment["VULKAN_SDK"] == nil {
            let vulkan = root.appendingPathComponent("Runtime/vendor/vulkan-sdk")
            if !FileManager.default.fileExists(atPath: vulkan.appendingPathComponent("include/vulkan/vulkan.h").path) {
                try DeveloperTool.run(["vulkan", "bootstrap", "--destination", vulkan.path], root: root,
                    environment: SwiftPackage.environment(root: root))
            }
        }
        #endif
        print("Ready. guava create app MyApp; guava run MyApp --\(arguments.platform.rawValue)")
    }
}
