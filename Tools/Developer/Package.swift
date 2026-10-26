// swift-tools-version: 6.4
import PackageDescription
#if os(Windows)
let diagnostics: [Target] = []
let client: [Target.Dependency] = []
#else
let diagnostics: [Target] = [
    .target(name: "DevTools", dependencies: [.product(name: "ToolSupport", package: "Support")]),
    .testTarget(name: "DevToolsTests", dependencies: ["DevTools"]),
]
let client: [Target.Dependency] = ["DevTools"]
#endif
let package = Package(name: "CoveSDKTools",
    platforms: [.macOS(.v26), .iOS(.v26)], products: [.executable(name: "cove-dev", targets: ["CoveDev"])], dependencies: [.package(path: "../Support")], targets: diagnostics + [.executableTarget(name: "CoveDev", dependencies: [.product(name: "ToolSupport", package: "Support")] + client)])
