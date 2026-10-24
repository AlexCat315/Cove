// swift-tools-version: 6.4
import PackageDescription

// Portable example content, deliberately outside the production UI package.
let package = Package(
    name: "CounterExample",
    platforms: [.macOS(.v26), .iOS(.v26)],
    products: [.library(name: "CounterExample", targets: ["CounterExample"])],
    dependencies: [.package(path: "../../Portable")],
    targets: [
        .target(name: "CounterExample", dependencies: [
            .product(name: "GuavaUIComposeCore", package: "Portable"),
        ]),
        .testTarget(name: "CounterExampleTests", dependencies: [
            "CounterExample",
            .product(name: "GuavaUIDevToolsScene", package: "Portable"),
            .product(name: "GuavaUIDevToolsProtocol", package: "Portable"),
        ]),
    ]
)
