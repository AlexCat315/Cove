// swift-tools-version: 6.4
import PackageDescription

let package = Package(
    name: "GuavaTerminalDemo",
    platforms: [.macOS(.v13)],
    products: [.executable(name: "TerminalDemo", targets: ["TerminalDemo"])],
    dependencies: [.package(name: "GuavaUI", path: "..")],
    targets: [
        .executableTarget(name: "TerminalDemo", dependencies: [
            .product(name: "GuavaUITerminal", package: "GuavaUI"),
            .product(name: "GuavaUI", package: "GuavaUI"),
        ])
    ]
)
