// swift-tools-version: 6.4
import PackageDescription

// The developer-facing CLI owns project workflows; platform backends are internal.
let package = Package(
    name: "GuavaTools",
    platforms: [.macOS(.v26), .iOS(.v26)],
    products: [.executable(name: "guava", targets: ["GuavaCLI"])],
    dependencies: [
        .package(path: "../Support"),
        .package(path: "../../Logging"),.package(path: "../../Hosts/Android/Contract"), .package(path: "../../Portable")],
    targets: [
        .executableTarget(name: "GuavaCLI", dependencies: [.product(name: "ToolSupport", package: "Support"),.product(name: "GuavaLogging", package: "Logging"),
            .product(name: "GuavaAndroidContract", package: "Contract"),
            .product(name: "GuavaUIDevToolsProtocol", package: "Portable"),
        ], resources: [.copy("Templates"), .copy("SDK.json")]),
        .testTarget(name: "GuavaCLITests", dependencies: ["GuavaCLI"]),
    ]
)

for target in package.targets { target.swiftSettings = [.enableUpcomingFeature("InternalImportsByDefault")] }
