// swift-tools-version: 6.4
import PackageDescription

// The developer-facing CLI owns project workflows; platform backends are internal.
let package = Package(
    name: "CoveTools",
    platforms: [.macOS(.v26), .iOS(.v26)],
    products: [.executable(name: "cove", targets: ["CoveCLI"])],
    dependencies: [
        .package(path: "../Support"),
        .package(path: "../../System"),
        .package(path: "../../Logging"),.package(path: "../../Hosts/Android/Contract"), .package(path: "../../Portable")],
    targets: [
        .executableTarget(name: "CoveCLI", dependencies: [.product(name: "ToolSupport", package: "Support"),.product(name: "CoveLogging", package: "Logging"),
            .product(name: "CoveFileSystem", package: "System"),
            .product(name: "CoveProcess", package: "System"),
            .product(name: "CoveAndroidContract", package: "Contract"),
            .product(name: "CoveUIDevToolsProtocol", package: "Portable"),
        ], resources: [.copy("Templates"), .copy("SDK.json")]),
        .testTarget(name: "CoveCLITests", dependencies: ["CoveCLI"]),
    ]
)

for target in package.targets { target.swiftSettings = [.enableUpcomingFeature("InternalImportsByDefault")] }
