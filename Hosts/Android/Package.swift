// swift-tools-version: 6.4
import PackageDescription
import Foundation
let contentPath = ProcessInfo.processInfo.environment["COVE_ANDROID_UI_PACKAGE"]
let showcase = contentPath == nil
let contentPackages: [Package.Dependency] = contentPath.map { [.package(path: $0)] } ?? []
let contentTargets: [Target.Dependency] = contentPath.map {
    [.product(name: "AppContent", package: URL(fileURLWithPath: $0).lastPathComponent)]
} ?? []
let package = Package(name: "CoveAndroid",
    platforms: [.macOS(.v26), .iOS(.v26)], products: [
    .executable(name: "CoveAndroidProbe", targets: ["CoveAndroidProbe"]),
    .library(name: "CoveAndroidApp", type: .dynamic, targets: ["CoveAndroidApp"])
], dependencies: [.package(name: "CoveUI", path: "../.."), .package(path: "Contract")]
    + contentPackages + (showcase ? [.package(path: "../../Examples")] : []),
 targets: [
    .target(name: "CAndroidHost", dependencies: [.product(name: "CCoveLog", package: "CoveUI")], publicHeadersPath: "include", linkerSettings: [.linkedLibrary("android"), .linkedLibrary("log")]),
    .target(name: "CoveAndroidApp", dependencies: [
        .product(name: "CoveLogging", package: "CoveUI"),
        .product(name: "CoveAndroidContract", package: "Contract"),
        .product(name: "NativeRHI", package: "CoveUI"),
        .product(name: "CoveUIRuntime", package: "CoveUI"),
        .product(name: "CoveUICompose", package: "CoveUI"),
        .product(name: "CoveUIAndroid", package: "CoveUI"),
        .product(name: "CoveUIDevTools", package: "CoveUI"), "CAndroidHost"
    ] + contentTargets + (showcase ? [.product(name: "GalleryContent", package: "Examples"), .product(name: "CounterExample", package: "Examples")] : []),
        swiftSettings: [.define(showcase ? "COVE_ANDROID_SHOWCASE" : "COVE_ANDROID_UI")],
        linkerSettings: [.linkedLibrary("ssl"), .linkedLibrary("crypto"), .linkedLibrary("z"), .unsafeFlags(["-Xlinker", "-z", "-Xlinker", "defs"])]),
    .executableTarget(name: "CoveAndroidProbe", dependencies: [.product(name: "CoveLogging", package: "CoveUI"), .product(name: "NativeRHI", package: "CoveUI"), .product(name: "CoveUIRuntime", package: "CoveUI"), .product(name: "CoveUIComposeCore", package: "CoveUI")], linkerSettings: [.linkedLibrary("ssl"), .linkedLibrary("crypto"), .linkedLibrary("z")])
])
