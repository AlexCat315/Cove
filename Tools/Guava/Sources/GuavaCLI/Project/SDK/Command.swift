internal import Foundation
private import ToolSupport

enum SDKCommand {
    /// Select the project's tool before interpreting flags introduced by a newer SDK.
    static func bootstrap(_ raw: [String]) throws {
        guard let verb = raw.first, !["self", "sdk", "internal"].contains(verb) else { return }
        if let root = try selectedSDK(raw, store: SDKStore()) { try invoke(root, raw: raw) }
    }

    static func selectedSDK(_ raw: [String], store: SDKStore,
                            environment: [String: String] = ProcessInfo.processInfo.environment) throws -> URL? {
        if let index = raw.firstIndex(of: "--sdk-root"), raw.indices.contains(index + 1) {
            return URL(fileURLWithPath: raw[index + 1]).standardizedFileURL
        }
        if let path = environment["GUAVA_SDK_ROOT"] { return URL(fileURLWithPath: path).standardizedFileURL }
        let values = Set(Option.allCases.map { "--" + $0.rawValue })
        let tokens = Array(raw.prefix(while: { $0 != "--" }))
        let paths = tokens.indices.dropFirst().filter { !tokens[$0].hasPrefix("--") && !values.contains(tokens[$0 - 1]) }
            .map { URL(fileURLWithPath: tokens[$0]).standardizedFileURL }
        let cwd = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
        let project = (paths + [cwd]).first { FileManager.default.fileExists(atPath: $0.appendingPathComponent("guava.json").path) }
        let data = project.flatMap { try? Data(contentsOf: $0.appendingPathComponent("guava.json")) }
        let json = data.flatMap { try? JSONSerialization.jsonObject(with: $0) as? [String: Any] }
        let required = (json?["toolchain"] as? [String: Any])?["sdkVersion"] as? String
        if let project, let local = SDKStore.selection(at: project.appendingPathComponent(".guava/sdk.json")) { return local }
        let bundled = InstallationReceipt.executableDirectory
        let candidates = ([store.selected].compactMap { $0 } + store.installedURLs() + [bundled]).filter(GuavaSDK.isSDK)
        guard let required else { return candidates.first }
        if required == SDKRelease.latestTag {
            return candidates.max { (try? SDKRelease.read(at: $0).version.compare(SDKRelease.read(at: $1).version, options: .numeric)) == .orderedAscending }
        }
        if let root = candidates.first(where: { (try? SDKRelease.read(at: $0).version) == required }) { return root }
        return try store.installed(store.install(version: required))
    }

    static func forward(_ arguments: Arguments, raw: [String]) throws {
        switch arguments.operation {
        case .selfUpdate, .installCLI, .packageCLI, .packageNative, .sdkInstall, .sdkUse, .sdkList, .clean, .devices:
            return
        default: break
        }
        let root = try GuavaSDK.locate(arguments)
        try invoke(root, raw: raw)
    }

    private static func invoke(_ root: URL, raw: [String]) throws {
        let release = try SDKRelease.read(at: root)
        var binary = SDKPayload.executable(at: root)
        #if os(Windows)
        if !FileManager.default.fileExists(atPath: binary.path) { binary = root.appendingPathComponent("guava.cmd") }
        #endif
        let current = InstallationReceipt.executableDirectory
        guard release.version != (try SDKRelease.current.version)
                || (GuavaSDK.isSDK(current) && current.resolvingSymlinksInPath().path != root.resolvingSymlinksInPath().path) else { return }
        guard FileManager.default.fileExists(atPath: binary.path) else {
            throw ToolError.invalid("The selected SDK is missing its CLI: \(binary.path)")
        }
        var environment = ProcessInfo.processInfo.environment
        guard environment["GUAVA_DISPATCHED_SDK"] != root.path else {
            throw ToolError.invalid("SDK CLI and manifest versions disagree at \(root.path)")
        }
        environment["GUAVA_SDK_ROOT"] = root.path
        environment["GUAVA_DISPATCHED_SDK"] = root.path
        let result = try ProcessRunner.run([binary.path] + raw, environment: environment, stream: true)
        exit(result.status)
    }
}
