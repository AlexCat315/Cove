internal import ToolSupport
import Foundation

struct ToolchainCheck {
    let name: String
    let ready: Bool
    let remedy: String

    static func executable(_ name: String, remedy: String) -> Self {
        Self(name: name, ready: (try? Shell.executable(name)) != nil, remedy: remedy)
    }
}

enum ToolchainRequirements {
    static func supportsCompiler(_ version: String) -> Bool {
        version.range(of: #"Swift version 6\.4(?:\.[0-9]+)?(?:\s|$)"#, options: .regularExpression) != nil
    }

    static func inspect(_ arguments: Arguments, root: URL) -> [ToolchainCheck] {
        let common = "Install Git, CMake and Ninja. macOS: brew install git cmake ninja; "
            + "Ubuntu/Debian: sudo apt-get install git cmake ninja-build build-essential pkg-config. "
            + "Windows: winget install Git.Git Kitware.CMake Ninja-build.Ninja; reopen the terminal."
        let prebuilt = (try? JSONDecoder().decode(NativeArtifacts.self, from: Data(contentsOf: root.appendingPathComponent("Artifacts/native.json"))))?.platforms[CLIUpdate.platform]?.isEmpty == false
        let commands = prebuilt ? ["git"] : ["git", "cmake", "ninja", "zig"]
        var checks = commands.map { ToolchainCheck.executable($0, remedy: common) }
        let version = (try? Shell.run([GuavaSDK.swift, "--version"], capture: true, quiet: true)) ?? ""
        checks.append(.init(name: "Swift 6.4", ready: supportsCompiler(version),
            remedy: "Install Swift 6.4.1 from https://www.swift.org/install/. With Swiftly: swiftly install 6.4.1; swiftly use 6.4.1. GUAVA_SWIFT_EXECUTABLE can select an existing compiler."))
        #if os(macOS)
        checks.append(.init(name: "Apple platform SDK", ready: (try? Shell.run(["xcrun", "--show-sdk-path"], capture: true, quiet: true)) != nil,
            remedy: "Run xcode-select --install, complete the Apple installer, then reopen the terminal."))
        #elseif os(Windows)
        var windows = ProcessInfo.processInfo.environment
        checks.append(.init(name: "Visual Studio C++ tools and Windows SDK", ready: (try? WindowsBuildEnvironment.activate(&windows)) != nil,
            remedy: "Install Visual Studio Build Tools with Desktop development with C++ and a Windows SDK."))
        #else
        checks.append(.executable("cc", remedy: common))
        if arguments.platform == .linux {
            checks.append(.executable("glslc", remedy: "Install the Vulkan SDK from https://vulkan.lunarg.com/ and a Vulkan-capable GPU driver; export VULKAN_SDK."))
        }
        #endif
        if arguments.platform == .ios {
            for name in ["xcodebuild", "xcrun"] {
                checks.append(.executable(name, remedy: "Install full Xcode and Rust; rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios"))
            }
        }
        if arguments.platform == .android { checks += AndroidRequirements.inspect(arguments, root: root, version: version) }
        return checks
    }
}
