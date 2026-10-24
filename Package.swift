// swift-tools-version: 6.4
import PackageDescription
import Foundation

let runtimeArtifacts = ProcessInfo.processInfo.environment["GUAVA_RUNTIME_ARTIFACT_ROOT"] ?? "Runtime/vendor"
let textArtifacts = ProcessInfo.processInfo.environment["GUAVA_TEXT_ARTIFACT_ROOT"] ?? "vendor"
let windowsCxxRuntime: [LinkerSetting] = ["msvcrt", "msvcprt"].map {
    .linkedLibrary($0, .when(platforms: [.windows]))
}


let packageDirectory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
#if os(macOS)
let nativeSystem = "macos"
#elseif os(Windows)
let nativeSystem = "windows"
#else
let nativeSystem = "linux"
#endif
#if arch(arm64)
let nativeArchitecture = "arm64"
#else
let nativeArchitecture = "x86_64"
#endif
let nativeDocument = (try? Data(contentsOf: packageDirectory.appendingPathComponent("Artifacts/native.json")))
    .flatMap { try? JSONSerialization.jsonObject(with: $0) as? [String: Any] }
let nativePlatforms = nativeDocument?["platforms"] as? [String: [String: [String: String]]] ?? [:]
let nativeArchives = nativePlatforms["\(nativeSystem)-\(nativeArchitecture)"] ?? [:]
func nativeTarget(_ name: String, directory: String) -> Target {
    let local = "\(directory)/\(name).artifactbundle"
    if FileManager.default.fileExists(atPath: packageDirectory.appendingPathComponent(local).path) {
        return .binaryTarget(name: name, path: local)
    }
    if let artifact = nativeArchives[name], let url = artifact["url"], let checksum = artifact["sha256"] {
        return .binaryTarget(name: name, url: url, checksum: checksum)
    }
    return .binaryTarget(name: name, path: local)
}

let platformTargets: [Target] = [
    .target(name: "GuavaPlatformCore", path: "PlatformCore/Sources/GuavaPlatformCore"),
    .testTarget(name: "GuavaPlatformCoreTests", dependencies: ["GuavaPlatformCore"], path: "PlatformCore/Tests/GuavaPlatformCoreTests")
]

let loggingTargets: [Target] = [
    .target(name: "CGuavaLog", path: "Logging/Sources/CGuavaLog", publicHeadersPath: "include", linkerSettings: [.linkedLibrary("log", .when(platforms: [.android]))]),
    .target(name: "GuavaLogCore", dependencies: [.product(name: "Logging", package: "swift-log")], path: "Logging/Sources/GuavaLogCore"),
    .target(name: "GuavaLogging", dependencies: ["CGuavaLog", "GuavaLogCore"], path: "Logging/Sources/GuavaLogging"),
    .testTarget(name: "GuavaLoggingTests", dependencies: ["GuavaLogging", "CGuavaLog"], path: "Logging/Tests/GuavaLoggingTests")
]

let portableTargets: [Target] = [
    .target(name: "GuavaUIAgentCore", path: "Portable/Sources/GuavaUIAgentCore"),
    .target(name: "GuavaUIAgentTransport", dependencies: [
            "GuavaUIAgentCore", .product(name: "NIOCore", package: "swift-nio"),
            .product(name: "NIOPosix", package: "swift-nio"),
        ], path: "Portable/Sources/GuavaUIAgentTransport"),
    .testTarget(name: "GuavaUIAgentTransportTests", dependencies: [
            "GuavaUIAgentTransport", .product(name: "NIOEmbedded", package: "swift-nio"),
        ], path: "Portable/Tests/GuavaUIAgentTransportTests"),
    .testTarget(name: "GuavaUIAgentCoreTests", dependencies: ["GuavaUIAgentCore"], path: "Portable/Tests/GuavaUIAgentCoreTests"),
    .target(name: "CYoga", dependencies: [.product(name: "yoga", package: "yoga")], path: "Portable/Sources/CYoga", publicHeadersPath: "include", linkerSettings: windowsCxxRuntime),
    .target(name: "GuavaUIScene", dependencies: ["GuavaUICore", "CYoga", .target(name: "GuavaPlatformCore")], path: "Portable/Sources/GuavaUIScene"),
    .target(name: "GuavaUIComposeCore", dependencies: ["GuavaUIScene"], path: "Portable/Sources/GuavaUIComposeCore"),
    .target(name: "GuavaUIDevToolsScene", dependencies: ["GuavaUIScene", "GuavaUIDevToolsProtocol", "GuavaUIAgentCore"], path: "Portable/Sources/GuavaUIDevToolsScene"),
    .target(name: "GuavaUICore", path: "Portable/Sources/GuavaUICore"),
    .testTarget(name: "GuavaUISceneTests", dependencies: ["GuavaUIComposeCore", "GuavaUIDevToolsScene", "GuavaUIDevToolsProtocol"], path: "Portable/Tests/GuavaUISceneTests"),
    .testTarget(name: "GuavaUICoreTests", dependencies: ["GuavaUICore"], path: "Portable/Tests/GuavaUICoreTests"),
    .target(name: "GuavaUIDevToolsProtocol", dependencies: [.target(name: "GuavaLogCore"), .target(name: "GuavaPlatformCore")], path: "Portable/Sources/GuavaUIDevToolsProtocol"),
    .target(name: "GuavaUIDevToolsServer", dependencies: [
            "GuavaUIDevToolsProtocol",
            .product(name: "NIOCore", package: "swift-nio"),
            .product(name: "NIOPosix", package: "swift-nio"),
            .product(name: "NIOHTTP1", package: "swift-nio"),
            .product(name: "NIOWebSocket", package: "swift-nio"),
            .target(name: "GuavaLogging"),
        ], path: "Portable/Sources/GuavaUIDevToolsServer"),
    .executableTarget(name: "GuavaUIDevToolsProbe", dependencies: [
            "GuavaUIDevToolsServer", "GuavaUIDevToolsScene", "GuavaUIComposeCore", .target(name: "GuavaLogging"),
        ], path: "Portable/Sources/GuavaUIDevToolsProbe"),
    .testTarget(name: "GuavaUIDevToolsServerTests", dependencies: [
            "GuavaUIDevToolsServer", "GuavaUIDevToolsProtocol", .target(name: "GuavaLogging"),
            .product(name: "NIOEmbedded", package: "swift-nio"),
        ], path: "Portable/Tests/GuavaUIDevToolsServerTests")
]

let runtimeTargets: [Target] = [
    .testTarget(name: "NativeRHITests", dependencies: ["NativeRHI"], path: "Runtime/Tests/NativeRHITests", resources: [.copy("Fixtures")]),
    nativeTarget("SDL3", directory: runtimeArtifacts),
    nativeTarget("lunasvg", directory: runtimeArtifacts),
    nativeTarget("plutovg", directory: runtimeArtifacts),
    nativeTarget("webp", directory: runtimeArtifacts),
    nativeTarget("sharpyuv", directory: runtimeArtifacts),
    .target(name: "CDX12Bridge", path: "Runtime/Sources/Bridge/CDX12Bridge", publicHeadersPath: "include",
                linkerSettings: windowsCxxRuntime + [.linkedLibrary("d3d12", .when(platforms: [.windows])),
                                 .linkedLibrary("dxgi", .when(platforms: [.windows])),
                                 .linkedLibrary("dxguid", .when(platforms: [.windows]))]),
    .target(name: "CVulkanHeaders", path: "Runtime/Sources/Bridge/CVulkanHeaders", publicHeadersPath: "include",
                linkerSettings: [.linkedLibrary("dl", .when(platforms: [.linux, .android])),
                                 .linkedLibrary("pthread", .when(platforms: [.linux]))]),
    .target(name: "NativeRHI", dependencies: ["CDX12Bridge",
            .target(name: "CVulkanHeaders", condition: .when(platforms: [.windows, .linux, .android])),
            .target(name: "GuavaLogging")], path: "Runtime/Sources/NativeRHI", exclude: ["docs"]),
    .target(name: "CSDL3", dependencies: ["SDL3"], path: "Runtime/Sources/Bridge/CSDL3", publicHeadersPath: "include",
                linkerSettings: [
                    .linkedFramework("AVFoundation", .when(platforms: [.macOS, .iOS])),
                    .linkedFramework("AudioToolbox", .when(platforms: [.macOS, .iOS])),
                    .linkedFramework("CoreAudio", .when(platforms: [.macOS, .iOS])),
                    .linkedFramework("CoreFoundation", .when(platforms: [.macOS, .iOS])),
                    .linkedFramework("CoreGraphics", .when(platforms: [.macOS, .iOS])),
                    .linkedFramework("CoreHaptics", .when(platforms: [.macOS, .iOS])),
                    .linkedFramework("CoreMedia", .when(platforms: [.macOS, .iOS])),
                    .linkedFramework("CoreVideo", .when(platforms: [.macOS, .iOS])),
                    .linkedFramework("GameController", .when(platforms: [.macOS, .iOS])),
                    .linkedFramework("Metal", .when(platforms: [.macOS, .iOS])),
                    .linkedFramework("QuartzCore", .when(platforms: [.macOS, .iOS])),
                    .linkedFramework("UniformTypeIdentifiers", .when(platforms: [.macOS, .iOS])),
                    .linkedFramework("Carbon", .when(platforms: [.macOS])),
                    .linkedFramework("Cocoa", .when(platforms: [.macOS])),
                    .linkedFramework("ForceFeedback", .when(platforms: [.macOS])),
                    .linkedFramework("IOKit", .when(platforms: [.macOS])),
                    .linkedFramework("UIKit", .when(platforms: [.iOS])),
                    .linkedFramework("CoreBluetooth", .when(platforms: [.iOS])),
                    .linkedFramework("OpenGLES", .when(platforms: [.iOS])),
                    .linkedLibrary("winmm", .when(platforms: [.windows])),
                    .linkedLibrary("setupapi", .when(platforms: [.windows])),
                    .linkedLibrary("imm32", .when(platforms: [.windows])),
                    .linkedLibrary("version", .when(platforms: [.windows])),
                    .linkedLibrary("ole32", .when(platforms: [.windows])),
                    .linkedLibrary("oleaut32", .when(platforms: [.windows])),
                    .linkedLibrary("uuid", .when(platforms: [.windows])),
                    .linkedLibrary("advapi32", .when(platforms: [.windows])),
                    .linkedLibrary("shell32", .when(platforms: [.windows])),
                    .linkedLibrary("user32", .when(platforms: [.windows])),
                    .linkedLibrary("gdi32", .when(platforms: [.windows])),
                    .linkedLibrary("d3d11", .when(platforms: [.windows])),
                    .linkedLibrary("dxgi", .when(platforms: [.windows])),
                    .linkedLibrary("d3d12", .when(platforms: [.windows])),
                    .linkedLibrary("android", .when(platforms: [.android]))]),
    .target(name: "PlatformShell", dependencies: ["CSDL3",
            .target(name: "GuavaPlatformCore"),
            .target(name: "GuavaLogging")], path: "Runtime/Sources/PlatformShell"),
    .target(name: "CImageDecodeBridge", dependencies: ["lunasvg", "plutovg", "webp", "sharpyuv"],
                path: "Runtime/Sources/Bridge/CImageDecodeBridge", publicHeadersPath: "include",
                cxxSettings: [.define("LUNASVG_BUILD_STATIC")], linkerSettings: windowsCxxRuntime),
]

let textTargets: [Target] = [
    nativeTarget("GhosttyVt", directory: "vendor"),
    .target(name: "GuavaUITerminal", dependencies: ["GhosttyVt", "GuavaUICompose", "GuavaUIRuntime"], path: "Terminal/Sources/GuavaUITerminal",
                linkerSettings: [.linkedLibrary("ntdll", .when(platforms: [.windows])),
                                 .linkedLibrary("kernel32", .when(platforms: [.windows]))]),
    .testTarget(name: "GuavaUITerminalTests", dependencies: ["GuavaUITerminal", "GuavaUICompose", "GuavaUIRuntime"], path: "Terminal/Tests/GuavaUITerminalTests"),
    nativeTarget("CFreeType", directory: textArtifacts),
    nativeTarget("CHarfBuzz", directory: textArtifacts),
    .target(name: "GuavaUIText", dependencies: [
            "CFreeType", "CHarfBuzz", .target(name: "GuavaUICore"),
        ], path: "Text/Sources/GuavaUIText"),
    .testTarget(name: "GuavaUITextTests", dependencies: ["GuavaUIText"], path: "Text/Tests/GuavaUITextTests")
]

let uiTargets: [Target] = [
    .target(name: "GuavaUIAgent", dependencies: [
            .target(name: "GuavaUIAgentCore"),
            .target(name: "GuavaUIAgentTransport"),
            .target(name: "GuavaUIDevToolsScene"),
        ], path: "Sources/GuavaUIAgent"),
    .target(name: "GuavaUIAndroid", dependencies: ["GuavaUICompose", "GuavaUIRuntime", "GuavaUIDevTools"], path: "Platforms/Android"),
    .target(
            name: "GuavaUIRuntime",
            dependencies: [
                .target(name: "GuavaUICore"),
                .target(name: "CYoga"),
                .target(name: "GuavaUIScene"),
                .target(name: "GuavaUIText"),
                .target(name: "CFreeType"),
                .target(name: "CHarfBuzz"),
                .target(name: "NativeRHI"),
                .target(name: "PlatformShell"),
                .target(name: "GuavaPlatformCore"),
                .target(name: "CImageDecodeBridge"),
                .target(name: "GuavaLogging"),
            ], path: "Sources/GuavaUIRuntime",
            resources: [.copy("Resources/Shaders")],
            linkerSettings: [
                // winmm provides timeBeginPeriod/timeEndPeriod, used by the run
                // loop to raise the Windows timer resolution so SDL's timed
                // event waits aren't quantized to ~15.6ms for fixed rates.
                .linkedLibrary("winmm", .when(platforms: [.windows])),
            ]
        ),
    .target(
            name: "GuavaUICompose",
            dependencies: ["GuavaUIRuntime", .target(name: "GuavaLogging"),
                           .product(name: "Markdown", package: "swift-markdown"),
                           .target(name: "GuavaUIScene"),
                           .target(name: "GuavaUIComposeCore"),
                           .target(name: "GuavaPlatformCore")], path: "Sources/GuavaUICompose",
            resources: [
                .process("Resources"),
            ]
        ),
    .target(
            name: "GuavaUIWebView",
            dependencies: ["GuavaUICompose"], path: "Sources/GuavaUIWebView"
        ),
    .target(
            name: "GuavaUIWorkspace",
            dependencies: [
                "GuavaUICompose",
                "GuavaUIRuntime",
                .target(name: "GuavaPlatformCore"),
            ], path: "Sources/GuavaUIWorkspace",
            resources: [
                .process("Resources"),
            ]
        ),
    .target(
            name: "GuavaUI",
            dependencies: [
                "GuavaUIRuntime",
                "GuavaUICompose",
                "GuavaUIDevTools",
                .target(name: "PlatformShell"),
                .target(name: "NativeRHI"),
                .target(name: "GuavaPlatformCore"),
                .target(name: "GuavaLogging"),
            ], path: "Sources/GuavaUI",
            resources: [
                .process("Resources"),
            ]
        ),
    .target(
            name: "GuavaUIDevTools",
            dependencies: [
                .target(name: "GuavaUIDevToolsServer"),
                .target(name: "GuavaUIDevToolsScene"),
                .target(name: "GuavaUIDevToolsProtocol"),
                "GuavaUIRuntime",
                .target(name: "GuavaPlatformCore"),
                .target(name: "NativeRHI"),
                .target(name: "GuavaLogging"),
            ], path: "Sources/GuavaUIDevTools"
        ),
    .testTarget(name: "GuavaUIAndroidTests", dependencies: ["GuavaUIAndroid", "GuavaUICompose", "GuavaUIRuntime"], path: "Tests/GuavaUIAndroidTests"),
    .testTarget(
            name: "GuavaUIRuntimeTests",
            dependencies: [
                .target(name: "GuavaUIScene"),
                "GuavaUIRuntime",
                .target(name: "NativeRHI"),
                .target(name: "PlatformShell"),
            ], path: "Tests/GuavaUIRuntimeTests"
        ),
    .testTarget(
            name: "GuavaUIComposeTests",
            dependencies: [
                .product(name: "NIOCore", package: "swift-nio"),
                .product(name: "NIOPosix", package: "swift-nio"),
                .product(name: "NIOHTTP1", package: "swift-nio"),
                "GuavaUIAgent",
                .target(name: "GuavaUIComposeCore"),
                "GuavaUICompose",
                "GuavaUIRuntime",
                .target(name: "GuavaPlatformCore"),
            ], path: "Tests/GuavaUIComposeTests"
        ),
    .testTarget(
            name: "GuavaUITests",
            dependencies: [
                "GuavaUI",
                "GuavaUIDevTools",
                "GuavaUICompose",
                "GuavaUIWorkspace",
                .target(name: "NativeRHI"),
            ], path: "Tests/GuavaUITests"
        ),
    .testTarget(
            name: "GuavaUIWebViewTests",
            dependencies: [
                "GuavaUIWebView",
                "GuavaUICompose",
            ], path: "Tests/GuavaUIWebViewTests"
        ),
    .testTarget(
            name: "GuavaUIWorkspaceTests",
            dependencies: [
                "GuavaUIWorkspace",
                "GuavaUICompose",
                "GuavaUIRuntime",
            ], path: "Tests/GuavaUIWorkspaceTests"
        ),
    .testTarget(
            name: "GuavaUIDevToolsTests",
            dependencies: [
                "GuavaUIDevTools",
                "GuavaUIRuntime",
                .target(name: "NativeRHI"),
            ], path: "Tests/GuavaUIDevToolsTests"
        )
]

let package = Package(
    name: "GuavaUI",
    platforms: [.macOS(.v26), .iOS(.v26)],
    products: [
.library(name: "GuavaPlatformCore", targets: ["GuavaPlatformCore"]),
.library(name: "GuavaLogging", targets: ["GuavaLogging"]),
    .library(name: "GuavaLogCore", targets: ["GuavaLogCore"]),
    .library(name: "CGuavaLog", targets: ["CGuavaLog"]),
.library(name: "GuavaUIAgentCore", targets: ["GuavaUIAgentCore"]),
        .library(name: "GuavaUIAgentTransport", targets: ["GuavaUIAgentTransport"]),
        .library(name: "GuavaUIScene", targets: ["GuavaUIScene"]),
        .library(name: "GuavaUIComposeCore", targets: ["GuavaUIComposeCore"]),
        .library(name: "CYoga", targets: ["CYoga"]),
        .library(name: "GuavaUIDevToolsScene", targets: ["GuavaUIDevToolsScene"]),
        .library(name: "GuavaUICore", targets: ["GuavaUICore"]),
        .library(name: "GuavaUIDevToolsProtocol", targets: ["GuavaUIDevToolsProtocol"]),
        .library(name: "GuavaUIDevToolsServer", targets: ["GuavaUIDevToolsServer"]),
        .executable(name: "GuavaUIDevToolsProbe", targets: ["GuavaUIDevToolsProbe"]),
.library(name: "CSDL3", targets: ["CSDL3"]),
        .library(name: "NativeRHI", targets: ["NativeRHI"]),
        .library(name: "PlatformShell", targets: ["PlatformShell"]),
        .library(name: "ImageDecodeBridge", targets: ["CImageDecodeBridge"]),
.library(name: "GuavaUITerminal", targets: ["GuavaUITerminal"]),
        .library(name: "GuavaUIText", targets: ["GuavaUIText"]),
        .library(name: "CFreeType", targets: ["CFreeType"]),
        .library(name: "CHarfBuzz", targets: ["CHarfBuzz"]),
.library(name: "GuavaUIAndroid", targets: ["GuavaUIAndroid"]),
        .library(name: "GuavaUIRuntime", targets: ["GuavaUIRuntime"]),
        .library(name: "GuavaUICompose", targets: ["GuavaUICompose"]),
        .library(name: "GuavaUIWebView", targets: ["GuavaUIWebView"]),
        .library(name: "GuavaUIWorkspace", targets: ["GuavaUIWorkspace"]),
        .library(name: "GuavaUI", targets: ["GuavaUI"]),
        .library(name: "GuavaUIDevTools", targets: ["GuavaUIDevTools"]),
        .library(name: "GuavaUIAgent", targets: ["GuavaUIAgent"]),
    ],
    dependencies: [
        .package(url: "https://github.com/facebook/yoga.git", from: "3.2.1"),
        .package(url: "https://github.com/apple/swift-nio.git", from: "2.103.0"),
        .package(url: "https://github.com/apple/swift-log.git", from: "1.15.1"),
        .package(url: "https://github.com/swiftlang/swift-markdown.git", from: "0.9.0"),
    ],
    targets: platformTargets + loggingTargets + portableTargets + runtimeTargets + textTargets + uiTargets,
    cxxLanguageStandard: .cxx17
)
