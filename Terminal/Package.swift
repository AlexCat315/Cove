// swift-tools-version: 6.4
import PackageDescription
import Foundation

let directory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
let document = try JSONSerialization.jsonObject(with: Data(contentsOf: directory.appendingPathComponent("cove.json"))) as! [String: Any]
let requiredSDK = (document["toolchain"] as! [String: String])["sdkVersion"]!
let environment = ProcessInfo.processInfo.environment
#if os(Windows)
let sdkHome = environment["COVE_HOME"].map { URL(fileURLWithPath: $0) }
    ?? URL(fileURLWithPath: environment["LOCALAPPDATA"]!).appendingPathComponent("Cove")
#elseif os(Linux)
let sdkHome = environment["COVE_HOME"].map { URL(fileURLWithPath: $0) }
    ?? URL(fileURLWithPath: environment["XDG_DATA_HOME"] ?? FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".local/share").path).appendingPathComponent("cove")
#else
let sdkHome = environment["COVE_HOME"].map { URL(fileURLWithPath: $0) }
    ?? FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".cove")
#endif
func sdkSelection(_ file: URL) -> URL? {
    guard let data = try? Data(contentsOf: file),
          let value = try? JSONSerialization.jsonObject(with: data) as? [String: String],
          let path = value["path"] else { return nil }
    return URL(fileURLWithPath: path)
}
func sdkVersion(_ root: URL) -> String? {
    guard FileManager.default.fileExists(atPath: root.appendingPathComponent("Package.swift").path),
          let data = try? Data(contentsOf: root.appendingPathComponent("Tools/Cove/Sources/CoveCLI/SDK.json")),
          let value = try? JSONSerialization.jsonObject(with: data) as? [String: String] else { return nil }
    return value["version"]
}
let overrideSDK = environment["COVE_SDK_ROOT"].map { URL(fileURLWithPath: $0) }
    ?? sdkSelection(directory.appendingPathComponent(".cove/sdk.json"))
let selectedSDK = sdkSelection(sdkHome.appendingPathComponent("sdk.json"))
let installedSDKs = (try? FileManager.default.contentsOfDirectory(at: sdkHome.appendingPathComponent("versions"), includingPropertiesForKeys: nil, options: [.skipsHiddenFiles])) ?? []
let candidates = ([selectedSDK].compactMap { $0 } + installedSDKs).filter { sdkVersion($0) != nil }
let resolvedSDK = overrideSDK ?? (requiredSDK == "latest"
    ? candidates.max { sdkVersion($0)!.compare(sdkVersion($1)!, options: .numeric) == .orderedAscending }
    : candidates.first { sdkVersion($0) == requiredSDK })
guard let resolvedSDK, let actualSDK = sdkVersion(resolvedSDK),
      requiredSDK == "latest" || requiredSDK == actualSDK else {
    fatalError("Install the shared Cove SDK with cove sdk install \(requiredSDK), then select it with cove sdk use VERSION or PATH")
}

let package = Package(
    name: "CoveTerminalDemo",
    platforms: [.macOS(.v26), .iOS(.v26)],
    products: [.executable(name: "TerminalDemo", targets: ["TerminalDemo"])],
    dependencies: [.package(name: "CoveUI", path: resolvedSDK.path)],
    targets: [
        .executableTarget(name: "TerminalDemo", dependencies: [
            .product(name: "CoveUITerminal", package: "CoveUI"),
            .product(name: "CoveUI", package: "CoveUI"),
        ])
    ]
)
