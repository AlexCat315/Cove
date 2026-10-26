// swift-tools-version: 6.4
import PackageDescription

let package = Package(
    name: "CoveSystem",
    platforms: [.macOS(.v26), .iOS(.v26)],
    products: [
        .library(name: "CoveFileSystem", targets: ["CoveFileSystem"]),
        .library(name: "CoveProcess", targets: ["CoveProcess"]),
    ],
    targets: [

        .target(name: "CCoveSystem", linkerSettings: [
            .linkedLibrary("advapi32", .when(platforms: [.windows])),
        ]),
        .target(name: "CoveFileSystem", dependencies: ["CCoveSystem"]),
        .target(name: "CoveProcess", dependencies: ["CCoveSystem"]),
        .executableTarget(name: "CoveProcessFixture", dependencies: ["CoveProcess"]),
        .testTarget(name: "CoveFileSystemTests", dependencies: ["CoveFileSystem"]),
        .testTarget(name: "CoveProcessTests", dependencies: ["CoveProcess", "CoveProcessFixture"]),
    ]
)

for target in package.targets where target.type != .system && target.name != "CCoveSystem" {
    target.swiftSettings = [.enableUpcomingFeature("InternalImportsByDefault")]
}
