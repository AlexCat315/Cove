// swift-tools-version: 6.4
import PackageDescription
let windowsCxxRuntime: [LinkerSetting] = ["msvcrt", "msvcprt"].map { .linkedLibrary($0, .when(platforms: [.windows])) }
let package = Package(
    name: "CoveUIGraphics",
    platforms: [.macOS(.v26), .iOS(.v26)],
    products: [.library(name: "CoveUIGraphics", targets: ["CoveUIGraphics"])],
    dependencies: [.package(path: "../Core"), .package(path: "../Text"), .package(path: "../Imaging"),
                   .package(path: "../Logging"), .package(path: "../PlatformCore")],
    targets: [
    .target(name: "CDX12Bridge", path: "Sources/CDX12Bridge", publicHeadersPath: "include",
                linkerSettings: windowsCxxRuntime + [.linkedLibrary("d3d12", .when(platforms: [.windows])),
                                 .linkedLibrary("dxgi", .when(platforms: [.windows])),
                                 .linkedLibrary("dxguid", .when(platforms: [.windows]))]),
    .target(name: "CVulkanHeaders", path: "Sources/CVulkanHeaders", publicHeadersPath: "include",
                linkerSettings: [.linkedLibrary("dl", .when(platforms: [.linux, .android])),
                                 .linkedLibrary("pthread", .when(platforms: [.linux]))]),
    .target(name: "NativeRHI", dependencies: ["CDX12Bridge",
            .target(name: "CVulkanHeaders", condition: .when(platforms: [.windows, .linux, .android])),
            .product(name: "CoveLogging", package: "Logging")], path: "Sources/NativeRHI", exclude: ["docs"]),

        .target(name: "CoveUIGraphics", dependencies: ["NativeRHI", .product(name: "CoveUICore", package: "Core"),
            .product(name: "CoveUIText", package: "Text"), .product(name: "CoveUIImaging", package: "Imaging"),
            .product(name: "CovePlatformCore", package: "PlatformCore"), .product(name: "CoveLogging", package: "Logging")],
            resources: [.copy("Resources/Shaders")]),
        .testTarget(name: "NativeRHITests", dependencies: ["NativeRHI"], resources: [.copy("Fixtures")]),
    ], cxxLanguageStandard: .cxx17
)
