// swift-tools-version: 6.4
import PackageDescription

let package = Package(
    name: "GuavaSystem",
    platforms: [.macOS(.v26), .iOS(.v26)],
    products: [
        .library(name: "GuavaFileSystem", targets: ["GuavaFileSystem"]),
        .library(name: "GuavaProcess", targets: ["GuavaProcess"]),
    ],
    targets: [

        .target(name: "CGuavaSystem", linkerSettings: [
            .linkedLibrary("advapi32", .when(platforms: [.windows])),
        ]),
        .target(name: "GuavaFileSystem", dependencies: ["CGuavaSystem"]),
        .target(name: "GuavaProcess", dependencies: ["CGuavaSystem"]),
        .executableTarget(name: "GuavaProcessFixture", dependencies: ["GuavaProcess"]),
        .testTarget(name: "GuavaFileSystemTests", dependencies: ["GuavaFileSystem"]),
        .testTarget(name: "GuavaProcessTests", dependencies: ["GuavaProcess", "GuavaProcessFixture"]),
    ]
)

for target in package.targets where target.type != .system && target.name != "CGuavaSystem" {
    target.swiftSettings = [.enableUpcomingFeature("InternalImportsByDefault")]
}
