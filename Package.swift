// swift-tools-version: 6.4
import PackageDescription
import Foundation

let runtimeArtifacts = ProcessInfo.processInfo.environment["COVE_RUNTIME_ARTIFACT_ROOT"] ?? "Runtime/vendor"
let textArtifacts = ProcessInfo.processInfo.environment["COVE_TEXT_ARTIFACT_ROOT"] ?? "vendor"
let windowsCxxRuntime: [LinkerSetting] = ["msvcrt", "msvcprt"].map {
    .linkedLibrary($0, .when(platforms: [.windows]))
}


func nativeTarget(_ name: String, directory: String) -> Target {
    let local = "\(directory)/\(name).artifactbundle"
    return .binaryTarget(name: name, path: local)
}

let platformTargets: [Target] = [
    .target(name: "CovePlatformCore", path: "PlatformCore/Sources/CovePlatformCore"),
    .testTarget(name: "CovePlatformCoreTests", dependencies: ["CovePlatformCore"], path: "PlatformCore/Tests/CovePlatformCoreTests")
]

let systemTargets: [Target] = [
    .target(name: "CCoveSystem", path: "System/Sources/CCoveSystem", linkerSettings: [
        .linkedLibrary("advapi32", .when(platforms: [.windows])),
    ]),
    .target(name: "CoveFileSystem", dependencies: ["CCoveSystem"], path: "System/Sources/CoveFileSystem"),
    .target(name: "CoveProcess", dependencies: ["CCoveSystem"], path: "System/Sources/CoveProcess"),
    .executableTarget(name: "CoveProcessFixture", dependencies: ["CoveProcess"], path: "System/Sources/CoveProcessFixture"),
    .testTarget(name: "CoveFileSystemTests", dependencies: ["CoveFileSystem"], path: "System/Tests/CoveFileSystemTests"),
    .testTarget(name: "CoveProcessTests", dependencies: ["CoveProcess", "CoveProcessFixture"], path: "System/Tests/CoveProcessTests"),
]

let loggingTargets: [Target] = [
    .target(name: "CCoveLog", path: "Logging/Sources/CCoveLog", publicHeadersPath: "include", linkerSettings: [.linkedLibrary("log", .when(platforms: [.android]))]),
    .target(name: "CoveLogCore", dependencies: [.product(name: "Logging", package: "swift-log")], path: "Logging/Sources/CoveLogCore"),
    .target(name: "CoveLogging", dependencies: ["CCoveLog", "CoveLogCore"], path: "Logging/Sources/CoveLogging"),
    .testTarget(name: "CoveLoggingTests", dependencies: ["CoveLogging", "CCoveLog"], path: "Logging/Tests/CoveLoggingTests")
]

let portableTargets: [Target] = [
    .target(name: "CoveUIAgentCore", dependencies: ["CoveFileSystem"], path: "Portable/Sources/CoveUIAgentCore"),
    .target(name: "CoveUIAgentTransport", dependencies: [
            "CoveUIAgentCore", .product(name: "NIOCore", package: "swift-nio"),
            .product(name: "NIOPosix", package: "swift-nio"),
        ], path: "Portable/Sources/CoveUIAgentTransport"),
    .testTarget(name: "CoveUIAgentTransportTests", dependencies: [
            "CoveUIAgentTransport", .product(name: "NIOEmbedded", package: "swift-nio"),
        ], path: "Portable/Tests/CoveUIAgentTransportTests"),
    .testTarget(name: "CoveUIAgentCoreTests", dependencies: ["CoveUIAgentCore"], path: "Portable/Tests/CoveUIAgentCoreTests"),
    .target(name: "CYoga", dependencies: [.product(name: "yoga", package: "yoga")], path: "Portable/Sources/CYoga", publicHeadersPath: "include", linkerSettings: windowsCxxRuntime),
    .target(name: "CoveUIScene", dependencies: ["CoveUICore", "CYoga", .target(name: "CovePlatformCore")], path: "Portable/Sources/CoveUIScene"),
    .target(name: "CoveUIComposeCore", dependencies: ["CoveUIScene"], path: "Portable/Sources/CoveUIComposeCore"),
    .target(name: "CoveUIDevToolsScene", dependencies: ["CoveUIScene", "CoveUIDevToolsProtocol", "CoveUIAgentCore"], path: "Portable/Sources/CoveUIDevToolsScene"),
    .target(name: "CoveUICore", path: "Portable/Sources/CoveUICore"),
    .testTarget(name: "CoveUISceneTests", dependencies: ["CoveUIComposeCore", "CoveUIDevToolsScene", "CoveUIDevToolsProtocol"], path: "Portable/Tests/CoveUISceneTests"),
    .testTarget(name: "CoveUICoreTests", dependencies: ["CoveUICore"], path: "Portable/Tests/CoveUICoreTests"),
    .target(name: "CoveUIDevToolsProtocol", dependencies: [.target(name: "CoveLogCore"), .target(name: "CovePlatformCore")], path: "Portable/Sources/CoveUIDevToolsProtocol"),
    .target(name: "CoveUIDevToolsServer", dependencies: [
            "CoveUIDevToolsProtocol",
            .product(name: "NIOCore", package: "swift-nio"),
            .product(name: "NIOPosix", package: "swift-nio"),
            .product(name: "NIOHTTP1", package: "swift-nio"),
            .product(name: "NIOWebSocket", package: "swift-nio"),
            .target(name: "CoveLogging"),
        ], path: "Portable/Sources/CoveUIDevToolsServer"),
    .executableTarget(name: "CoveUIDevToolsProbe", dependencies: [
            "CoveUIDevToolsServer", "CoveUIDevToolsScene", "CoveUIComposeCore", .target(name: "CoveLogging"),
        ], path: "Portable/Sources/CoveUIDevToolsProbe"),
    .testTarget(name: "CoveUIDevToolsServerTests", dependencies: [
            "CoveUIDevToolsServer", "CoveUIDevToolsProtocol", .target(name: "CoveLogging"),
            .product(name: "NIOEmbedded", package: "swift-nio"),
        ], path: "Portable/Tests/CoveUIDevToolsServerTests")
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
            .target(name: "CoveLogging")], path: "Runtime/Sources/NativeRHI", exclude: ["docs"]),
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
                    .linkedLibrary("android", .when(platforms: [.android])),
                    .linkedLibrary("GLESv2", .when(platforms: [.android])),
                    .linkedLibrary("OpenSLES", .when(platforms: [.android]))]),
    .target(name: "PlatformShell", dependencies: ["CSDL3",
            .target(name: "CovePlatformCore"),
            .target(name: "CoveLogging")], path: "Runtime/Sources/PlatformShell"),
    .target(name: "CImageDecodeBridge", dependencies: ["lunasvg", "plutovg", "webp", "sharpyuv"],
                path: "Runtime/Sources/Bridge/CImageDecodeBridge", publicHeadersPath: "include",
                cxxSettings: [.define("LUNASVG_BUILD_STATIC")], linkerSettings: windowsCxxRuntime),
]

let textTargets: [Target] = [
    nativeTarget("GhosttyVt", directory: "vendor"),
    .target(name: "CoveUITerminal", dependencies: ["GhosttyVt", "CoveUICompose", "CoveUIRuntime"], path: "Terminal/Sources/CoveUITerminal",
                linkerSettings: [.linkedLibrary("ntdll", .when(platforms: [.windows])),
                                 .linkedLibrary("kernel32", .when(platforms: [.windows]))]),
    .testTarget(name: "CoveUITerminalTests", dependencies: ["CoveUITerminal", "CoveUICompose", "CoveUIRuntime"], path: "Terminal/Tests/CoveUITerminalTests"),
    nativeTarget("CFreeType", directory: textArtifacts),
    nativeTarget("CHarfBuzz", directory: textArtifacts),
    .target(name: "CoveUIText", dependencies: [
            "CFreeType", "CHarfBuzz", .target(name: "CoveUICore"),
        ], path: "Text/Sources/CoveUIText"),
    .testTarget(name: "CoveUITextTests", dependencies: ["CoveUIText"], path: "Text/Tests/CoveUITextTests")
]

let uiTargets: [Target] = [
    .target(name: "CoveUIAgent", dependencies: [
            .target(name: "CoveUIAgentCore"),
            .target(name: "CoveUIAgentTransport"),
            .target(name: "CoveUIDevToolsScene"),
        ], path: "Sources/CoveUIAgent"),
    .target(name: "CoveUIAndroid", dependencies: ["CoveUICompose", "CoveUIRuntime", "CoveUIDevTools"], path: "Platforms/Android"),
    .target(
            name: "CoveUIRuntime",
            dependencies: [
                .target(name: "CoveUICore"),
                .target(name: "CYoga"),
                .target(name: "CoveUIScene"),
                .target(name: "CoveUIText"),
                .target(name: "CFreeType"),
                .target(name: "CHarfBuzz"),
                .target(name: "NativeRHI"),
                .target(name: "PlatformShell"),
                .target(name: "CovePlatformCore"),
                .target(name: "CImageDecodeBridge"),
                .target(name: "CoveLogging"),
            ], path: "Sources/CoveUIRuntime",
            resources: [.copy("Resources/Shaders")],
            linkerSettings: [
                // winmm provides timeBeginPeriod/timeEndPeriod, used by the run
                // loop to raise the Windows timer resolution so SDL's timed
                // event waits aren't quantized to ~15.6ms for fixed rates.
                .linkedLibrary("winmm", .when(platforms: [.windows])),
            ]
        ),
    .target(
            name: "CoveUICompose",
            dependencies: ["CoveFileSystem", "CoveUIRuntime", .target(name: "CoveLogging"),
                           .product(name: "Markdown", package: "swift-markdown"),
                           .target(name: "CoveUIScene"),
                           .target(name: "CoveUIComposeCore"),
                           .target(name: "CovePlatformCore")], path: "Sources/CoveUICompose",
            resources: [
                .process("Resources"),
            ]
        ),
    .target(
            name: "CoveUIWebView",
            dependencies: ["CoveUICompose"], path: "Sources/CoveUIWebView"
        ),
    .target(
            name: "CoveUIWorkspace",
            dependencies: [
                "CoveUICompose",
                "CoveUIRuntime",
                .target(name: "CovePlatformCore"),
            ], path: "Sources/CoveUIWorkspace",
            resources: [
                .process("Resources"),
            ]
        ),
    .target(
            name: "CoveUI",
            dependencies: [
                "CoveUIRuntime",
                "CoveUICompose",
                "CoveUIDevTools",
                .target(name: "PlatformShell"),
                .target(name: "NativeRHI"),
                .target(name: "CovePlatformCore"),
                .target(name: "CoveLogging"),
            ], path: "Sources/CoveUI",
            resources: [
                .process("Resources"),
            ]
        ),
    .target(
            name: "CoveUIDevTools",
            dependencies: [
                "CoveFileSystem",
                .target(name: "CoveUIDevToolsServer"),
                .target(name: "CoveUIDevToolsScene"),
                .target(name: "CoveUIDevToolsProtocol"),
                "CoveUIRuntime",
                .target(name: "CovePlatformCore"),
                .target(name: "NativeRHI"),
                .target(name: "CoveLogging"),
            ], path: "Sources/CoveUIDevTools"
        ),
    .testTarget(name: "CoveUIAndroidTests", dependencies: ["CoveUIAndroid", "CoveUICompose", "CoveUIRuntime"], path: "Tests/CoveUIAndroidTests"),
    .testTarget(
            name: "CoveUIRuntimeTests",
            dependencies: [
                .target(name: "CoveUIScene"),
                "CoveUIRuntime",
                .target(name: "NativeRHI"),
                .target(name: "PlatformShell"),
            ], path: "Tests/CoveUIRuntimeTests"
        ),
    .testTarget(
            name: "CoveUIComposeTests",
            dependencies: [
                .product(name: "NIOCore", package: "swift-nio"),
                .product(name: "NIOPosix", package: "swift-nio"),
                .product(name: "NIOHTTP1", package: "swift-nio"),
                "CoveUIAgent",
                .target(name: "CoveUIComposeCore"),
                "CoveUICompose",
                "CoveUIRuntime",
                .target(name: "CovePlatformCore"),
            ], path: "Tests/CoveUIComposeTests"
        ),
    .testTarget(
            name: "CoveUITests",
            dependencies: [
                "CoveUI",
                "CoveUIDevTools",
                "CoveUICompose",
                "CoveUIWorkspace",
                .target(name: "NativeRHI"),
            ], path: "Tests/CoveUITests"
        ),
    .testTarget(
            name: "CoveUIWebViewTests",
            dependencies: [
                "CoveUIWebView",
                "CoveUICompose",
            ], path: "Tests/CoveUIWebViewTests"
        ),
    .testTarget(
            name: "CoveUIWorkspaceTests",
            dependencies: [
                "CoveUIWorkspace",
                "CoveUICompose",
                "CoveUIRuntime",
            ], path: "Tests/CoveUIWorkspaceTests"
        ),
    .testTarget(
            name: "CoveUIDevToolsTests",
            dependencies: [
                "CoveUIDevTools",
                "CoveUIRuntime",
                .target(name: "NativeRHI"),
            ], path: "Tests/CoveUIDevToolsTests"
        )
]

let package = Package(
    name: "CoveUI",
    platforms: [.macOS(.v26), .iOS(.v26)],
    products: [
.library(name: "CovePlatformCore", targets: ["CovePlatformCore"]),
.library(name: "CoveFileSystem", targets: ["CoveFileSystem"]),
.library(name: "CoveProcess", targets: ["CoveProcess"]),
.library(name: "CoveLogging", targets: ["CoveLogging"]),
    .library(name: "CoveLogCore", targets: ["CoveLogCore"]),
    .library(name: "CCoveLog", targets: ["CCoveLog"]),
.library(name: "CoveUIAgentCore", targets: ["CoveUIAgentCore"]),
        .library(name: "CoveUIAgentTransport", targets: ["CoveUIAgentTransport"]),
        .library(name: "CoveUIScene", targets: ["CoveUIScene"]),
        .library(name: "CoveUIComposeCore", targets: ["CoveUIComposeCore"]),
        .library(name: "CYoga", targets: ["CYoga"]),
        .library(name: "CoveUIDevToolsScene", targets: ["CoveUIDevToolsScene"]),
        .library(name: "CoveUICore", targets: ["CoveUICore"]),
        .library(name: "CoveUIDevToolsProtocol", targets: ["CoveUIDevToolsProtocol"]),
        .library(name: "CoveUIDevToolsServer", targets: ["CoveUIDevToolsServer"]),
        .executable(name: "CoveUIDevToolsProbe", targets: ["CoveUIDevToolsProbe"]),
.library(name: "CSDL3", targets: ["CSDL3"]),
        .library(name: "NativeRHI", targets: ["NativeRHI"]),
        .library(name: "PlatformShell", targets: ["PlatformShell"]),
        .library(name: "ImageDecodeBridge", targets: ["CImageDecodeBridge"]),
.library(name: "CoveUITerminal", targets: ["CoveUITerminal"]),
        .library(name: "CoveUIText", targets: ["CoveUIText"]),
        .library(name: "CFreeType", targets: ["CFreeType"]),
        .library(name: "CHarfBuzz", targets: ["CHarfBuzz"]),
.library(name: "CoveUIAndroid", targets: ["CoveUIAndroid"]),
        .library(name: "CoveUIRuntime", targets: ["CoveUIRuntime"]),
        .library(name: "CoveUICompose", targets: ["CoveUICompose"]),
        .library(name: "CoveUIWebView", targets: ["CoveUIWebView"]),
        .library(name: "CoveUIWorkspace", targets: ["CoveUIWorkspace"]),
        .library(name: "CoveUI", targets: ["CoveUI"]),
        .library(name: "CoveUIDevTools", targets: ["CoveUIDevTools"]),
        .library(name: "CoveUIAgent", targets: ["CoveUIAgent"]),
    ],
    dependencies: [
        .package(url: "https://github.com/facebook/yoga.git", from: "3.2.1"),
        .package(url: "https://github.com/apple/swift-nio.git", from: "2.103.0"),
        .package(url: "https://github.com/apple/swift-log.git", from: "1.15.1"),
        .package(url: "https://github.com/swiftlang/swift-markdown.git", from: "0.9.0"),
    ],
    targets: platformTargets + systemTargets + loggingTargets + portableTargets + runtimeTargets + textTargets + uiTargets,
    cxxLanguageStandard: .cxx17
)

for target in systemTargets where target.name != "CCoveSystem" {
    target.swiftSettings = [.enableUpcomingFeature("InternalImportsByDefault")]
}
