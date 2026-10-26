// swift-tools-version: 6.4
import PackageDescription
import Foundation

// Browser and Android builds stage target-specific artifacts separately from desktop libraries.
let artifacts = ProcessInfo.processInfo.environment["COVE_TEXT_ARTIFACT_ROOT"] ?? "../vendor"
let package = Package(
    name: "CoveUIText",
    platforms: [.macOS(.v26), .iOS(.v26)],
    products: [
        .library(name: "CoveUIText", targets: ["CoveUIText"]),
        .library(name: "CFreeType", targets: ["CFreeType"]),
        .library(name: "CHarfBuzz", targets: ["CHarfBuzz"]),
    ],
    dependencies: [.package(path: "../Portable")],
    targets: [
        .binaryTarget(name: "CFreeType", path: "\(artifacts)/CFreeType.artifactbundle"),
        .binaryTarget(name: "CHarfBuzz", path: "\(artifacts)/CHarfBuzz.artifactbundle"),
        .target(name: "CoveUIText", dependencies: [
            "CFreeType", "CHarfBuzz", .product(name: "CoveUICore", package: "Portable"),
        ]),
        .testTarget(name: "CoveUITextTests", dependencies: ["CoveUIText"]),
    ]
)
