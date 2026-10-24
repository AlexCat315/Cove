// swift-tools-version: 6.4
import PackageDescription
#if os(Windows)
let diagnostics: [Target] = []
let client: [Target.Dependency] = []
#else
let diagnostics: [Target] = [.target(name: "DevTools", dependencies: [.product(name: "ToolSupport", package: "Support")])]
let client: [Target.Dependency] = ["DevTools"]
#endif
let package = Package(name: "GuavaSDKTools",
    platforms: [.macOS(.v26), .iOS(.v26)], products: [.executable(name: "guava-dev", targets: ["GuavaDev"])], dependencies: [.package(path: "../Support")], targets: diagnostics + [.executableTarget(name: "GuavaDev", dependencies: [.product(name: "ToolSupport", package: "Support")] + client)])
