// swift-tools-version: 6.4
import PackageDescription
let package = Package(
    name: "CoveUIDevToolsProtocol",
    platforms: [.macOS(.v26), .iOS(.v26)],
    products: [.library(name: "CoveUIDevToolsProtocol", targets: ["CoveUIDevToolsProtocol"])],
    dependencies: [.package(path: "../../Logging"), .package(path: "../../PlatformCore")],
    targets: [.target(name: "CoveUIDevToolsProtocol", dependencies: [
        .product(name: "CoveLogCore", package: "Logging"), .product(name: "CovePlatformCore", package: "PlatformCore")])]
)
