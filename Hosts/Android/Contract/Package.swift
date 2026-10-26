// swift-tools-version: 6.4
import PackageDescription

let package = Package(
    name: "CoveAndroidContract",
    platforms: [.macOS(.v26), .iOS(.v26)],
    products: [.library(name: "CoveAndroidContract", targets: ["CoveAndroidContract"])],
    targets: [
        .target(name: "CoveAndroidContract"),
        .testTarget(name: "CoveAndroidContractTests", dependencies: ["CoveAndroidContract"]),
    ]
)
