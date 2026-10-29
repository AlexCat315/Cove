// swift-tools-version: 6.4
import PackageDescription
let package = Package(
    name: "CoveUICompose",
    platforms: [.macOS(.v26), .iOS(.v26)],
    products: [.library(name: "CoveUICompose", targets: ["CoveUICompose"])],
    dependencies: [.package(path: "Composition"), .package(path: "../Core"), .package(path: "../Text"),
        .package(path: "../Imaging"), .package(path: "../System"), .package(path: "../Logging"), .package(path: "../PlatformCore"),
        .package(url: "https://github.com/swiftlang/swift-markdown.git", from: "0.9.0")],
    targets: [
        .target(name: "CTextSelection", publicHeadersPath: "include", cSettings: [.unsafeFlags(["-O3"])]),
        .target(name: "CoveUICompose", dependencies: ["CTextSelection",
        .product(name: "CoveUIComposition", package: "Composition"), .product(name: "CoveUICore", package: "Core"),
        .product(name: "CoveUIText", package: "Text"), .product(name: "CoveUIImaging", package: "Imaging"),
        .product(name: "CoveFileSystem", package: "System"), .product(name: "CoveLogging", package: "Logging"),
        .product(name: "CovePlatformCore", package: "PlatformCore"), .product(name: "Markdown", package: "swift-markdown"),
    ], resources: [.process("Resources")])]
)
