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
], dependencies: [.package(path: "../../Runtime"), .package(path: "../../Compose"), .package(path: "../../Compose/Composition"), .package(path: "../../Graphics"), .package(path: "../../DevTools"), .package(path: "../../Logging"), .package(name: "CoveUI", path: "../.."), .package(path: "Contract")]
    + contentPackages + (showcase ? [.package(path: "../../Examples"), .package(path: "../../Examples/Counter")] : []),
 targets: [
    .target(name: "CAndroidHost", dependencies: [.product(name: "CCoveLog", package: "Logging")], publicHeadersPath: "include", linkerSettings: [.linkedLibrary("android"), .linkedLibrary("log")]),
    .target(name: "CoveAndroidApp", dependencies: [
        .product(name: "CoveLogging", package: "Logging"),
        .product(name: "CoveAndroidContract", package: "Contract"),
        .product(name: "CoveUIGraphics", package: "Graphics"),
        .product(name: "CoveUIRuntime", package: "Runtime"),
        .product(name: "CoveUICompose", package: "Compose"),
        .product(name: "CoveUIAndroid", package: "CoveUI"),
        .product(name: "CoveUIDevTools", package: "DevTools"), "CAndroidHost"
    ] + contentTargets + (showcase ? [.product(name: "GalleryContent", package: "Examples"), .product(name: "CounterExample", package: "Counter")] : []),
        swiftSettings: [.define(showcase ? "COVE_ANDROID_SHOWCASE" : "COVE_ANDROID_UI")],
        linkerSettings: [.linkedLibrary("ssl"), .linkedLibrary("crypto"), .linkedLibrary("z"), .unsafeFlags(["-Xlinker", "-z", "-Xlinker", "defs"])]),
    .executableTarget(name: "CoveAndroidProbe", dependencies: [.product(name: "CoveLogging", package: "Logging"), .product(name: "CoveUIGraphics", package: "Graphics"), .product(name: "CoveUIRuntime", package: "Runtime"), .product(name: "CoveUIComposition", package: "Composition")], linkerSettings: [.linkedLibrary("ssl"), .linkedLibrary("crypto"), .linkedLibrary("z")])
])
