// swift-tools-version: 6.4
import PackageDescription
import CompilerPluginSupport

// Shared UI and DevTools build without SDL, GPU drivers or native font libraries.
// Browser targets depend only on the protocol, never on the socket transport.
let package = Package(
    name: "CoveUIPortable",
    platforms: [.macOS(.v26), .iOS(.v26)],
    products: [
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
    ],
    dependencies: [
        .package(url: "https://github.com/swiftlang/swift-syntax.git", exact: "604.0.0"),
        .package(path: "../System"),
        .package(path: "../PlatformCore"),
        .package(url: "https://github.com/facebook/yoga.git", from: "3.2.1"),
        .package(url: "https://github.com/apple/swift-nio.git", from: "2.103.0"),
        .package(path: "../Logging"),
    ],
    targets: [
        .target(name: "CoveUIAgentCore", dependencies: [.product(name: "CoveFileSystem", package: "System")]),
        .target(name: "CoveUIAgentTransport", dependencies: [
            "CoveUIAgentCore", .product(name: "NIOCore", package: "swift-nio"),
            .product(name: "NIOPosix", package: "swift-nio"),
        ]),
        .testTarget(name: "CoveUIAgentTransportTests", dependencies: [
            "CoveUIAgentTransport", .product(name: "NIOEmbedded", package: "swift-nio"),
        ]),
        .testTarget(name: "CoveUIAgentCoreTests", dependencies: ["CoveUIAgentCore"]),
        .target(name: "CYoga", dependencies: [.product(name: "yoga", package: "yoga")], publicHeadersPath: "include",
                linkerSettings: [.linkedLibrary("msvcrt", .when(platforms: [.windows])),
                                 .linkedLibrary("msvcprt", .when(platforms: [.windows]))]),
        .target(name: "CoveUIScene", dependencies: ["CoveUICore", "CYoga", .product(name: "CovePlatformCore", package: "PlatformCore")]),
        .macro(name: "CoveUIReadMacros", dependencies: [
            .product(name: "SwiftSyntaxMacros", package: "swift-syntax"),
            .product(name: "SwiftCompilerPlugin", package: "swift-syntax"),
            .product(name: "SwiftDiagnostics", package: "swift-syntax"),
        ]),
        .target(name: "CoveUIComposeCore", dependencies: ["CoveUIScene", "CoveUIReadMacros"]),
        .target(name: "CoveUIDevToolsScene", dependencies: ["CoveUIScene", "CoveUIDevToolsProtocol", "CoveUIAgentCore"]),
        .target(name: "CThreadLocal"),
        .target(name: "CoveUICore", dependencies: ["CThreadLocal"]),
        .testTarget(name: "CoveUISceneTests", dependencies: ["CoveUIComposeCore", "CoveUIDevToolsScene", "CoveUIDevToolsProtocol"]),
        .testTarget(name: "CoveUICoreTests", dependencies: ["CoveUICore"]),
        .target(name: "CoveUIDevToolsProtocol", dependencies: [.product(name: "CoveLogCore", package: "Logging"), .product(name: "CovePlatformCore", package: "PlatformCore")]),
        .target(name: "CoveUIDevToolsServer", dependencies: [
            "CoveUIDevToolsProtocol",
            .product(name: "NIOCore", package: "swift-nio"),
            .product(name: "NIOPosix", package: "swift-nio"),
            .product(name: "NIOHTTP1", package: "swift-nio"),
            .product(name: "NIOWebSocket", package: "swift-nio"),
            .product(name: "CoveLogging", package: "Logging"),
        ]),
        .executableTarget(name: "CoveUIDevToolsProbe", dependencies: [
            "CoveUIDevToolsServer", "CoveUIDevToolsScene", "CoveUIComposeCore", .product(name: "CoveLogging", package: "Logging"),
        ]),
        .testTarget(name: "CoveUIDevToolsServerTests", dependencies: [
            "CoveUIDevToolsServer", "CoveUIDevToolsProtocol", .product(name: "CoveLogging", package: "Logging"),
            .product(name: "NIOEmbedded", package: "swift-nio"),
        ]),
    ]
)
