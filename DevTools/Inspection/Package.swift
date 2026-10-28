// swift-tools-version: 6.4
import PackageDescription
let package = Package(
    name: "CoveUIInspection",
    platforms: [.macOS(.v26), .iOS(.v26)],
    products: [.library(name: "CoveUIInspection", targets: ["CoveUIDevToolsScene"])],
    dependencies: [.package(path: "../../Core"), .package(path: "../Protocol"), .package(path: "../../System")],
    targets: [
        .target(name: "CoveUIAgentCore", dependencies: [.product(name: "CoveFileSystem", package: "System")]),
        .target(name: "CoveUIDevToolsScene", dependencies: ["CoveUIAgentCore", .product(name: "CoveUICore", package: "Core"),
            .product(name: "CoveUIDevToolsProtocol", package: "Protocol")]),
        .testTarget(name: "CoveUIAgentCoreTests", dependencies: ["CoveUIAgentCore"]),
    ]
)
