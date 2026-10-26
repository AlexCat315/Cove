// swift-tools-version: 6.4
import PackageDescription

let package = Package(name: "CoveLogging", platforms: [.macOS(.v26), .iOS(.v26)], products: [
    .library(name: "CoveLogging", targets: ["CoveLogging"]),
    .library(name: "CoveLogCore", targets: ["CoveLogCore"]),
    .library(name: "CCoveLog", targets: ["CCoveLog"]),
], dependencies: [.package(url: "https://github.com/apple/swift-log.git", from: "1.15.1")], targets: [
    .target(name: "CCoveLog", publicHeadersPath: "include", linkerSettings: [.linkedLibrary("log", .when(platforms: [.android]))]),
    .target(name: "CoveLogCore", dependencies: [.product(name: "Logging", package: "swift-log")]),
    .target(name: "CoveLogging", dependencies: ["CCoveLog", "CoveLogCore"]),
    .testTarget(name: "CoveLoggingTests", dependencies: ["CoveLogging", "CCoveLog"]),
])
