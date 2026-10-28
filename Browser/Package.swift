// swift-tools-version: 6.4
import PackageDescription

let package = Package(
    name: "CoveUIBrowser",
    platforms: [.macOS(.v26), .iOS(.v26)],
    dependencies: [.package(path: "../Core"), .package(path: "../Compose/Composition"), .package(path: "../DevTools/Inspection"), .package(path: "../DevTools/Protocol"), .package(path: "../Examples/Counter"), .package(path: "../Text"), .package(path: "../Logging")],
    targets: [
        .executableTarget(name: "CoveUIBrowserPrototype", dependencies: [
            .product(name: "CoveLogging", package: "Logging"),
            .product(name: "CoveUIText", package: "Text"),
            .product(name: "CounterExample", package: "Counter"),
            .product(name: "CoveUICore", package: "Core"),
            .product(name: "CoveUIComposition", package: "Composition"),
            .product(name: "CoveUIInspection", package: "Inspection"),
            .product(name: "CoveUIDevToolsProtocol", package: "Protocol"),
        ]),
    ]
)
