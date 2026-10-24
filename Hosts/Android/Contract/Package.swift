// swift-tools-version: 6.4
import PackageDescription

let package = Package(
    name: "GuavaAndroidContract",
    platforms: [.macOS(.v26), .iOS(.v26)],
    products: [.library(name: "GuavaAndroidContract", targets: ["GuavaAndroidContract"])],
    targets: [
        .target(name: "GuavaAndroidContract"),
        .testTarget(name: "GuavaAndroidContractTests", dependencies: ["GuavaAndroidContract"]),
    ]
)
