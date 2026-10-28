// swift-tools-version: 6.4
import PackageDescription
let package = Package(
    name: "CoveUIDevTools",
    platforms: [.macOS(.v26), .iOS(.v26)],
    products: [.library(name: "CoveUIDevTools", targets: ["CoveUIDevTools"]),
               .library(name: "CoveUIAgent", targets: ["CoveUIAgent"]),
               .executable(name: "CoveUIDevToolsProbe", targets: ["CoveUIDevToolsProbe"])],
    dependencies: [.package(path: "Protocol"), .package(path: "Inspection"), .package(path: "../Runtime"),
        .package(path: "../Compose/Composition"), .package(path: "../Core"), .package(path: "../Graphics"),
        .package(path: "../System"), .package(path: "../PlatformCore"), .package(path: "../Logging"),
        .package(url: "https://github.com/apple/swift-nio.git", from: "2.103.0")],
    targets: [
        .target(name: "CoveUIAgentTransport", dependencies: [.product(name: "CoveUIInspection", package: "Inspection"),
            .product(name: "NIOCore", package: "swift-nio"), .product(name: "NIOPosix", package: "swift-nio")]),
        .target(name: "CoveUIAgent", dependencies: ["CoveUIAgentTransport", .product(name: "CoveUIInspection", package: "Inspection")]),
        .target(name: "CoveUIDevToolsServer", dependencies: [.product(name: "CoveUIDevToolsProtocol", package: "Protocol"),
            .product(name: "NIOCore", package: "swift-nio"), .product(name: "NIOPosix", package: "swift-nio"),
            .product(name: "NIOHTTP1", package: "swift-nio"), .product(name: "NIOWebSocket", package: "swift-nio"),
            .product(name: "CoveLogging", package: "Logging")]),
        .target(name: "CoveUIDevTools", dependencies: ["CoveUIDevToolsServer", .product(name: "CoveUIInspection", package: "Inspection"),
            .product(name: "CoveUIDevToolsProtocol", package: "Protocol"), .product(name: "CoveUIRuntime", package: "Runtime"),
            .product(name: "CoveUIGraphics", package: "Graphics"), .product(name: "CoveFileSystem", package: "System"),
            .product(name: "CovePlatformCore", package: "PlatformCore"), .product(name: "CoveLogging", package: "Logging")]),
        .executableTarget(name: "CoveUIDevToolsProbe", dependencies: ["CoveUIDevToolsServer", .product(name: "CoveUIInspection", package: "Inspection"),
            .product(name: "CoveUIComposition", package: "Composition"), .product(name: "CoveLogging", package: "Logging")]),
        .testTarget(name: "CoveUIAgentTransportTests", dependencies: ["CoveUIAgentTransport", .product(name: "NIOEmbedded", package: "swift-nio")]),
        .testTarget(name: "CoveUIDevToolsServerTests", dependencies: ["CoveUIDevToolsServer", .product(name: "CoveUIDevToolsProtocol", package: "Protocol"),
            .product(name: "CoveLogging", package: "Logging"), .product(name: "NIOEmbedded", package: "swift-nio")]),
    ]
)
