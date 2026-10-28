// swift-tools-version: 6.4
import PackageDescription
import CompilerPluginSupport
let package = Package(
    name: "CoveUIComposition",
    platforms: [.macOS(.v26), .iOS(.v26)],
    products: [.library(name: "CoveUIComposition", targets: ["CoveUIComposeCore"])],
    dependencies: [.package(path: "../../Core"),
                   .package(url: "https://github.com/swiftlang/swift-syntax.git", exact: "604.0.0")],
    targets: [
        .macro(name: "CoveUIReadMacros", dependencies: [
            .product(name: "SwiftSyntaxMacros", package: "swift-syntax"),
            .product(name: "SwiftCompilerPlugin", package: "swift-syntax"),
            .product(name: "SwiftDiagnostics", package: "swift-syntax"),
        ]),
        .target(name: "CoveUIComposeCore", dependencies: [.product(name: "CoveUICore", package: "Core"), "CoveUIReadMacros"]),
    ]
)
