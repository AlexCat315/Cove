// swift-tools-version: 6.4
import PackageDescription
let package = Package(
    name: "CoveUI",
    platforms: [.macOS(.v26), .iOS(.v26)],
    products: [
        .library(name: "CoveUI", targets: ["CoveUI"]),
        .library(name: "CoveUIWebView", targets: ["CoveUIWebView"]),
        .library(name: "CoveUIWorkspace", targets: ["CoveUIWorkspace"]),
        .library(name: "CoveUITerminal", targets: ["CoveUITerminal"]),
        .library(name: "CoveUIAndroid", targets: ["CoveUIAndroid"]),
    ],
    dependencies: [.package(path: "Core"), .package(path: "Graphics"),
        .package(path: "Shell"), .package(path: "Compose"), .package(path: "Compose/Composition"),
        .package(path: "Runtime"), .package(path: "DevTools"), .package(path: "Logging"), .package(path: "PlatformCore"),
        .package(url: "https://github.com/apple/swift-nio.git", from: "2.103.0")],
    targets: [
        .target(name: "CoveUI", dependencies: [.product(name: "CoveUIRuntime", package: "Runtime"),
            .product(name: "CoveUICompose", package: "Compose"), .product(name: "CoveUIDevTools", package: "DevTools"),
            .product(name: "CoveUIGraphics", package: "Graphics"), .product(name: "CoveUIShell", package: "Shell"),
            .product(name: "CovePlatformCore", package: "PlatformCore"), .product(name: "CoveLogging", package: "Logging")],
            path: "Hosts/Desktop/Sources/CoveUI", resources: [.process("Resources")]),
        .target(name: "CoveUIWebView", dependencies: [.product(name: "CoveUICompose", package: "Compose")]),
        .target(name: "CoveUIWorkspace", dependencies: [.product(name: "CoveUICompose", package: "Compose"),
            .product(name: "CoveUIRuntime", package: "Runtime"), .product(name: "CovePlatformCore", package: "PlatformCore")], resources: [.process("Resources")]),
        .target(name: "CoveUIAndroid", dependencies: [.product(name: "CoveUICompose", package: "Compose"),
            .product(name: "CoveUIRuntime", package: "Runtime"), .product(name: "CoveUIDevTools", package: "DevTools")], path: "Hosts/Android/Integration"),
        .binaryTarget(name: "GhosttyVt", path: "vendor/GhosttyVt.artifactbundle"),
        .target(name: "CoveUITerminal", dependencies: ["GhosttyVt", .product(name: "CoveUICompose", package: "Compose"),
            .product(name: "CoveUIRuntime", package: "Runtime")], path: "Terminal/Sources/CoveUITerminal",
            linkerSettings: [.linkedLibrary("ntdll", .when(platforms: [.windows])), .linkedLibrary("kernel32", .when(platforms: [.windows]))]),
        .testTarget(name: "CoveUITerminalTests", dependencies: ["CoveUITerminal", .product(name: "CoveUICompose", package: "Compose"),
            .product(name: "CoveUIRuntime", package: "Runtime")], path: "Terminal/Tests/CoveUITerminalTests"),
        .testTarget(name: "CoveUIAndroidTests", dependencies: ["CoveUIAndroid", .product(name: "CoveUICompose", package: "Compose"),
            .product(name: "CoveUIRuntime", package: "Runtime")]),
    .testTarget(
            name: "CoveUIRuntimeTests",
            dependencies: [
                .product(name: "CoveUICore", package: "Core"),
                .product(name: "CoveUIRuntime", package: "Runtime"),
                .product(name: "CoveUIGraphics", package: "Graphics"),
                .product(name: "CoveUIShell", package: "Shell"),
            ], path: "Tests/CoveUIRuntimeTests"
        ),
    .testTarget(
            name: "CoveUIComposeTests",
            dependencies: [
                .product(name: "NIOCore", package: "swift-nio"),
                .product(name: "NIOPosix", package: "swift-nio"),
                .product(name: "NIOHTTP1", package: "swift-nio"),
                .product(name: "CoveUIAgent", package: "DevTools"),
                .product(name: "CoveUIComposition", package: "Composition"),
                .product(name: "CoveUICompose", package: "Compose"),
                .product(name: "CoveUIRuntime", package: "Runtime"),
                .product(name: "CovePlatformCore", package: "PlatformCore"),
            ], path: "Tests/CoveUIComposeTests"
        ),
    .testTarget(
            name: "CoveUITests",
            dependencies: [
                "CoveUI",
                .product(name: "CoveUIDevTools", package: "DevTools"),
                .product(name: "CoveUICompose", package: "Compose"),
                "CoveUIWorkspace",
                .product(name: "CoveUIGraphics", package: "Graphics"),
            ], path: "Tests/CoveUITests"
        ),
    .testTarget(
            name: "CoveUIWebViewTests",
            dependencies: [
                "CoveUIWebView",
                .product(name: "CoveUICompose", package: "Compose"),
            ], path: "Tests/CoveUIWebViewTests"
        ),
    .testTarget(
            name: "CoveUIWorkspaceTests",
            dependencies: [
                "CoveUIWorkspace",
                .product(name: "CoveUICompose", package: "Compose"),
                .product(name: "CoveUIRuntime", package: "Runtime"),
            ], path: "Tests/CoveUIWorkspaceTests"
        ),
    .testTarget(
            name: "CoveUIDevToolsTests",
            dependencies: [
                .product(name: "CoveUIDevTools", package: "DevTools"),
                .product(name: "CoveUIRuntime", package: "Runtime"),
                .product(name: "CoveUIGraphics", package: "Graphics"),
            ], path: "Tests/CoveUIDevToolsTests"
        )
    ]
)
