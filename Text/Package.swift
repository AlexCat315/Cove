// swift-tools-version: 6.4
import PackageDescription
import Foundation
let artifacts = ProcessInfo.processInfo.environment["COVE_TEXT_ARTIFACT_ROOT"].map { "../" + $0 } ?? "../vendor"
let package = Package(
    name: "CoveUIText",
    platforms: [.macOS(.v26), .iOS(.v26)],
    products: [.library(name: "CoveUIText", targets: ["CoveUIText"])],
    dependencies: [.package(path: "../Core")],
    targets: [
        .binaryTarget(name: "CFreeType", path: "\(artifacts)/CFreeType.artifactbundle"),
        .binaryTarget(name: "CHarfBuzz", path: "\(artifacts)/CHarfBuzz.artifactbundle"),
        .target(name: "CoveUIText", dependencies: ["CFreeType", "CHarfBuzz", .product(name: "CoveUICore", package: "Core")]),
        .testTarget(name: "CoveUITextTests", dependencies: ["CoveUIText"]),
    ]
)
