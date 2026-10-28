// swift-tools-version: 6.4
import PackageDescription
import Foundation
let artifacts = ProcessInfo.processInfo.environment["COVE_RUNTIME_ARTIFACT_ROOT"].map { "../" + $0 } ?? "../Native/vendor"
let nativeLinkOrder: [LinkerSetting] = ProcessInfo.processInfo.environment["COVE_NATIVE_LINK_ORDER"] == "1"
    ? ["plutovg", "sharpyuv"].map { .linkedLibrary($0, .when(platforms: [.linux, .android])) } : []
let package = Package(
    name: "CoveUIImaging",
    platforms: [.macOS(.v26), .iOS(.v26)],
    products: [.library(name: "CoveUIImaging", targets: ["CoveUIImaging"])],
    dependencies: [.package(path: "../Core")],
    targets: [
        .binaryTarget(name: "lunasvg", path: "\(artifacts)/lunasvg.artifactbundle"),
        .binaryTarget(name: "plutovg", path: "\(artifacts)/plutovg.artifactbundle"),
        .binaryTarget(name: "webp", path: "\(artifacts)/webp.artifactbundle"),
        .binaryTarget(name: "sharpyuv", path: "\(artifacts)/sharpyuv.artifactbundle"),
        .target(name: "CImageDecodeBridge", dependencies: ["lunasvg", "plutovg", "webp", "sharpyuv"], publicHeadersPath: "include",
                cxxSettings: [.define("LUNASVG_BUILD_STATIC")],
                linkerSettings: ["msvcrt", "msvcprt"].map { .linkedLibrary($0, .when(platforms: [.windows])) } + nativeLinkOrder),
        .target(name: "CoveUIImaging", dependencies: ["CImageDecodeBridge", .product(name: "CoveUICore", package: "Core")]),
    ], cxxLanguageStandard: .cxx17
)
