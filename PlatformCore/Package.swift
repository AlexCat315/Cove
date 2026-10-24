// swift-tools-version: 6.4
import PackageDescription

let package = Package(
    name: "GuavaPlatformCore",
    platforms: [.macOS(.v26), .iOS(.v26)],
    products: [.library(name: "GuavaPlatformCore", targets: ["GuavaPlatformCore"])],
    targets: [.target(name: "GuavaPlatformCore"),
              .testTarget(name: "GuavaPlatformCoreTests", dependencies: ["GuavaPlatformCore"])]
)
