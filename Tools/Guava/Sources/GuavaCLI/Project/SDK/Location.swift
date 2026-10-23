internal import Foundation

enum GuavaSDK {
    static func locate(_ arguments: Arguments, project: AppProject? = nil) throws -> URL {
        let environment = ProcessInfo.processInfo.environment
        let local = project.flatMap { SDKStore.selection(at: $0.directory.appendingPathComponent(".guava/sdk.json")) }
        let store = SDKStore()
        let required = project?.configuration.toolchain.sdkVersion
        let selected = store.selected.flatMap { root in
            required == nil || (try? SDKRelease.read(at: root).version) == required ? root : nil
        }
        let explicit = arguments.url(.sdkRoot)
            ?? environment["GUAVA_SDK_ROOT"].map { URL(fileURLWithPath: $0).standardizedFileURL }
            ?? local
        let pinned = required.map { store.installed($0) }.flatMap { isSDK($0) ? $0 : nil }
        let root = explicit ?? selected ?? pinned
            ?? checkout(containing: URL(fileURLWithPath: #filePath))
        guard let root, isSDK(root) else {
            throw ToolError.invalid("Install the SDK with guava sdk install \(required ?? "VERSION"), or select a checkout with --sdk-root")
        }
        let release = try SDKRelease.read(at: root)
        if let required, release.version != required {
            throw ToolError.invalid("Project requires SDK \(required); selected checkout provides \(release.version)")
        }
        return root
    }

    static func isSDK(_ root: URL) -> Bool {
        (try? SDKRelease.read(at: root)) != nil
            && FileManager.default.fileExists(atPath: root.appendingPathComponent("Package.swift").path)
    }

    private static func checkout(containing source: URL) -> URL? {
        var directory = source.deletingLastPathComponent()
        while directory.path != directory.deletingLastPathComponent().path {
            if isSDK(directory) { return directory }
            directory.deleteLastPathComponent()
        }
        return nil
    }

    static var swift: String { ProcessInfo.processInfo.environment["GUAVA_SWIFT_EXECUTABLE"] ?? "swift" }
}
