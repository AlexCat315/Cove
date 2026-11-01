// swift-tools-version: 6.4
import PackageDescription

// Shared example content uses the supported composition API.
let package = Package(
    name: "CounterExample",
    platforms: [.macOS(.v26), .iOS(.v26)],
    products: [.library(name: "CounterExample", targets: ["CounterExample"])],
    dependencies: [.package(path: "../../Compose/Composition"), .package(path: "../../DevTools/Inspection"), .package(path: "../../DevTools/Protocol")],
    targets: [
        .target(name: "CounterExample", dependencies: [
            .product(name: "CoveUIComposition", package: "Composition"),
        ]),
        .testTarget(name: "CounterExampleTests", dependencies: [
            "CounterExample",
            .product(name: "CoveUIInspection", package: "Inspection"),
            .product(name: "CoveUIDevToolsProtocol", package: "Protocol"),
        ]),
    ]
)
