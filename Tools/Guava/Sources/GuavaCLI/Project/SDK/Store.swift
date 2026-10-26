internal import Foundation

struct SDKStore {
    struct Selection: Codable { let path: String }
    let directory: URL

    init(directory: URL? = nil) { self.directory = directory ?? Installation.directory }
    var selected: URL? { Self.selection(at: directory.appendingPathComponent("sdk.json")) }
    func installed(_ version: String) -> URL { directory.appendingPathComponent("sdks/\(version)") }

    /// All valid SDK checkouts under sdks/, unordered.
    func installedURLs() -> [URL] {
        let parent = directory.appendingPathComponent("sdks")
        guard let children = try? FileManager.default.contentsOfDirectory(at: parent, includingPropertiesForKeys: nil) else { return [] }
        return children.filter(GuavaSDK.isSDK)
    }

    static func selection(at file: URL) -> URL? {
        guard let data = try? Data(contentsOf: file),
              let selection = try? JSONDecoder().decode(Selection.self, from: data) else { return nil }
        return URL(fileURLWithPath: selection.path)
    }

    func use(_ root: URL) throws {
        guard GuavaSDK.isSDK(root) else { throw ToolError.invalid("Not a Guava SDK: \(root.path)") }
        try Files.json(Selection(path: root.standardizedFileURL.path), to: directory.appendingPathComponent("sdk.json"))
        print("Selected Guava SDK: \(root.path)")
    }

    func list() throws {
        let installed = directory.appendingPathComponent("sdks")
        if FileManager.default.fileExists(atPath: installed.path) {
            for sdk in try Files.children(installed).filter({ GuavaSDK.isSDK($0) }) { print(sdk.path) }
        }
        print("Selected: \(selected?.path ?? "none")")
    }

    /// Installs the SDK; "latest" is resolved to the latest release first.
    /// Returns the concrete version that was installed.
    @discardableResult
    func install(version requested: String) throws -> String {
        let repository = try SDKRelease.current.repository
        let version = requested == "latest"
            ? try SDKRelease.latestVersion(repository: repository)
            : requested
        guard SDKRelease.isVersion(version) else {
            throw ToolError.invalid("Use guava sdk install VERSION or latest")
        }
        let target = installed(version)
        if GuavaSDK.isSDK(target) { try use(target); return version }
        let staging = directory.appendingPathComponent("downloads/" + UUID().uuidString)
        try Files.directory(staging.deletingLastPathComponent())
        defer { try? Files.remove(staging) }
        try Shell.run(["git", "clone", "--depth", "1", "--branch", "v" + version,
                       "--recurse-submodules", "--shallow-submodules", "--", repository, staging.path])
        guard try SDKRelease.read(at: staging).version == version else { throw ToolError.invalid("SDK tag and manifest version disagree") }
        try Files.directory(target.deletingLastPathComponent())
        try FileManager.default.moveItem(at: staging, to: target)
        try use(target)
        return version
    }
}
