// swift-tools-version: 6.4
import PackageDescription
let package = Package(name: "GuavaToolSupport",
    platforms: [.macOS(.v26), .iOS(.v26)], products: [.library(name: "ToolSupport", targets: ["ToolSupport"])], targets: [.target(name: "ToolSupport")])
