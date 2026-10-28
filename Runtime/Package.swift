// swift-tools-version: 6.4
import PackageDescription
let package = Package(
    name: "CoveUIRuntime",
    platforms: [.macOS(.v26), .iOS(.v26)],
    products: [.library(name: "CoveUIRuntime", targets: ["CoveUIRuntime"])],
    dependencies: [.package(path: "../Core"), .package(path: "../Text"), .package(path: "../Imaging"),
        .package(path: "../Graphics"), .package(path: "../Shell"), .package(path: "../Compose"),
        .package(path: "../Logging"), .package(path: "../PlatformCore")],
    targets: [.target(name: "CoveUIRuntime", dependencies: [
        .product(name: "CoveUICore", package: "Core"), .product(name: "CoveUIText", package: "Text"),
        .product(name: "CoveUIImaging", package: "Imaging"), .product(name: "CoveUIGraphics", package: "Graphics"),
        .product(name: "CoveUIShell", package: "Shell"), .product(name: "CoveUICompose", package: "Compose"),
        .product(name: "CoveLogging", package: "Logging"), .product(name: "CovePlatformCore", package: "PlatformCore"),
    ], linkerSettings: [.linkedLibrary("winmm", .when(platforms: [.windows]))])]
)
