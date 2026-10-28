// swift-tools-version: 6.4
import PackageDescription
let package = Package(
    name: "CoveUICore",
    platforms: [.macOS(.v26), .iOS(.v26)],
    products: [.library(name: "CoveUICore", targets: ["CoveUIScene"])],
    dependencies: [.package(path: "../PlatformCore"), .package(url: "https://github.com/facebook/yoga.git", from: "3.2.1")],
    targets: [
        .target(name: "CoveUICore"),
        .target(name: "CYoga", dependencies: [.product(name: "yoga", package: "yoga")], publicHeadersPath: "include",
                linkerSettings: [.linkedLibrary("msvcrt", .when(platforms: [.windows])), .linkedLibrary("msvcprt", .when(platforms: [.windows]))]),
        .target(name: "CoveUIScene", dependencies: ["CoveUICore", "CYoga", .product(name: "CovePlatformCore", package: "PlatformCore")]),
        .testTarget(name: "CoveUICoreTests", dependencies: ["CoveUICore"]),
    ], cxxLanguageStandard: .cxx17
)
