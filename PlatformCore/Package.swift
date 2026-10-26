// swift-tools-version: 6.4
import PackageDescription

let package = Package(
    name: "CovePlatformCore",
    platforms: [.macOS(.v26), .iOS(.v26)],
    products: [.library(name: "CovePlatformCore", targets: ["CovePlatformCore"])],
    targets: [.target(name: "CovePlatformCore"),
              .testTarget(name: "CovePlatformCoreTests", dependencies: ["CovePlatformCore"])]
)
