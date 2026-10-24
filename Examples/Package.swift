// swift-tools-version: 6.4
import PackageDescription

let package = Package(
    name: "GuavaUIExamples",
    platforms: [.macOS(.v26), .iOS(.v26)],
    products: [
        .library(name: "GalleryContent", targets: ["GalleryContent"]),
        .library(name: "CounterExample", targets: ["CounterExample"]),
        .executable(name: "GuavaUIDemo", targets: ["GuavaUIDemo"]),
        .executable(name: "PetDemo", targets: ["PetDemo"]),
    ],
    dependencies: [.package(name: "GuavaUI", path: "..")],
    targets: [
        .target(name: "CounterExample", dependencies: [
            .product(name: "GuavaUIComposeCore", package: "GuavaUI"),
        ], path: "Counter/Sources/CounterExample"),
        .target(name: "GalleryContent", dependencies: [
            "CounterExample",
            .product(name: "GuavaUI", package: "GuavaUI"),
            .product(name: "GuavaUIWebView", package: "GuavaUI"),
            .product(name: "GuavaUIWorkspace", package: "GuavaUI"),
        ], resources: [.process("Resources")]),
        .executableTarget(name: "GuavaUIDemo", dependencies: [
            "GalleryContent",
            .product(name: "GuavaUI", package: "GuavaUI"),
            "CounterExample",
        ]),
        .executableTarget(name: "PetDemo", dependencies: [
            .product(name: "GuavaUI", package: "GuavaUI"),
        ], resources: [.process("Resources")]),
        .testTarget(name: "GalleryContentTests", dependencies: ["GalleryContent"]),
    ]
)
