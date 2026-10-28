// swift-tools-version: 6.4
import PackageDescription
let package = Package(name: "CoveHeadlessTests", platforms: [.macOS(.v26), .iOS(.v26)],
    dependencies: [.package(path: "../../Core"), .package(path: "../../Compose/Composition"), .package(path: "../../DevTools/Inspection")],
    targets: [.testTarget(name: "CoveUISceneTests", dependencies: [.product(name: "CoveUICore", package: "Core"),
        .product(name: "CoveUIComposition", package: "Composition"), .product(name: "CoveUIInspection", package: "Inspection")])])
