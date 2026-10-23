// swift-tools-version: 6.4
import PackageDescription
import Foundation
let contentPath = ProcessInfo.processInfo.environment["GUAVA_ANDROID_UI_PACKAGE"]
let showcase = contentPath == nil
let contentPackages: [Package.Dependency] = contentPath.map { [.package(path: $0)] } ?? []
let contentTargets: [Target.Dependency] = contentPath.map {
    [.product(name: "AppContent", package: URL(fileURLWithPath: $0).lastPathComponent)]
} ?? []
let package = Package(name: "GuavaAndroid", products: [
    .executable(name: "GuavaAndroidProbe", targets: ["GuavaAndroidProbe"]),
    .library(name: "GuavaAndroidApp", type: .dynamic, targets: ["GuavaAndroidApp"])
], dependencies: [.package(path: "../.."), .package(path: "Contract")]
    + contentPackages + (showcase ? [.package(path: "../../Examples")] : []),
 targets: [
    .target(name: "CAndroidHost", dependencies: [.product(name: "CGuavaLog", package: "GuavaUI")], publicHeadersPath: "include", linkerSettings: [.linkedLibrary("android"), .linkedLibrary("log")]),
    .target(name: "GuavaAndroidApp", dependencies: [
        .product(name: "GuavaLogging", package: "GuavaUI"),
        .product(name: "GuavaAndroidContract", package: "Contract"),
        .product(name: "NativeRHI", package: "GuavaUI"),
        .product(name: "GuavaUIRuntime", package: "GuavaUI"),
        .product(name: "GuavaUICompose", package: "GuavaUI"),
        .product(name: "GuavaUIAndroid", package: "GuavaUI"),
        .product(name: "GuavaUIDevTools", package: "GuavaUI"), "CAndroidHost"
    ] + contentTargets + (showcase ? [.product(name: "GalleryContent", package: "Examples"), .product(name: "CounterExample", package: "Examples")] : []),
        swiftSettings: [.define(showcase ? "GUAVA_ANDROID_SHOWCASE" : "GUAVA_ANDROID_UI")],
        linkerSettings: [.linkedLibrary("ssl"), .linkedLibrary("crypto"), .linkedLibrary("z"), .unsafeFlags(["-Xlinker", "-z", "-Xlinker", "defs"])]),
    .executableTarget(name: "GuavaAndroidProbe", dependencies: [.product(name: "GuavaLogging", package: "GuavaUI"), .product(name: "NativeRHI", package: "GuavaUI"), .product(name: "GuavaUIRuntime", package: "GuavaUI"), .product(name: "GuavaUIComposeCore", package: "GuavaUI")], linkerSettings: [.linkedLibrary("ssl"), .linkedLibrary("crypto"), .linkedLibrary("z")])
])
