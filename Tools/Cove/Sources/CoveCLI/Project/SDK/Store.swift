internal import Foundation
private import CoveFileSystem

struct SDKStore {
    struct Selection: Codable { let path: String }
    let directory: URL

    init(directory: URL? = nil) { self.directory = directory ?? Installation.directory }
    var selected: URL? { Self.selection(at: directory.appendingPathComponent("sdk.json")) }
    func installed(_ version: String) -> URL { directory.appendingPathComponent("versions/\(version)") }

    /// Each version contains its CLI, framework sources and host native libraries.
    func installedURLs() -> [URL] {
        let parent = directory.appendingPathComponent("versions")
        guard let children = try? FileManager.default.contentsOfDirectory(at: parent, includingPropertiesForKeys: nil, options: [.skipsHiddenFiles]) else { return [] }
        return children.filter(CoveSDK.isSDK)
    }

    static func selection(at file: URL) -> URL? {
        guard let data = try? Data(contentsOf: file),
              let selection = try? JSONDecoder().decode(Selection.self, from: data) else { return nil }
        return URL(fileURLWithPath: selection.path)
    }

    func use(_ root: URL) throws {
        guard CoveSDK.isSDK(root) else { throw ToolError.invalid("Not a Cove SDK: \(root.path)") }
        try Files.json(Selection(path: root.standardizedFileURL.path), to: directory.appendingPathComponent("sdk.json"))
        print("Selected Cove SDK: \(root.path)")
    }

    func list() throws {
        let installed = directory.appendingPathComponent("versions")
        if FileManager.default.fileExists(atPath: installed.path) {
            for sdk in try Files.children(installed).filter({ CoveSDK.isSDK($0) }) { print(sdk.path) }
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
            throw ToolError.invalid("Use cove sdk install VERSION or latest")
        }
        return try FileLock.withLock(at: directory.appendingPathComponent("locks/sdk-\(version).lock")) {
            let target = installed(version)
            if CoveSDK.isSDK(target) { return version }
            let staging = directory.appendingPathComponent("downloads/" + UUID().uuidString)
            try Files.directory(staging.deletingLastPathComponent())
            defer { try? Files.remove(staging) }
            let payload = try SDKDownload.fetch(version: version, into: staging)
            try Files.directory(target.deletingLastPathComponent())
            let commands = (try? InstallationReceipt.read(at: directory))?.commands ?? Installation.commands
            let receipt = InstallationReceipt(version: version, directory: directory, commands: commands)
            try Files.json(receipt, to: payload.appendingPathComponent(InstallationReceipt.filename))
            do { try FileManager.default.moveItem(at: payload, to: target) }
            catch {
                guard CoveSDK.isSDK(target) else { throw error }
            }
            if selected == nil { try use(target) }
            print("Installed Cove SDK \(version): \(target.path)")
            return version
        }
    }
}
