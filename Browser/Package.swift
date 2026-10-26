// swift-tools-version: 6.4
import PackageDescription

let package = Package(
    name: "CoveUIBrowser",
    platforms: [.macOS(.v26), .iOS(.v26)],
    dependencies: [.package(path: "../Portable"), .package(path: "../Examples/Counter"), .package(path: "../Text"), .package(path: "../Logging")],
    targets: [
        .executableTarget(name: "CoveUIBrowserPrototype", dependencies: [
            .product(name: "CoveLogging", package: "Logging"),
            .product(name: "CoveUIText", package: "Text"),
            .product(name: "CounterExample", package: "Counter"),
            .product(name: "CoveUICore", package: "Portable"),
            .product(name: "CoveUIComposeCore", package: "Portable"),
            .product(name: "CoveUIDevToolsScene", package: "Portable"),
            .product(name: "CoveUIDevToolsProtocol", package: "Portable"),
        ]),
    ]
)
