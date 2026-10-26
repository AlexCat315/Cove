internal import Foundation
private import ToolSupport

enum NativeArchive {
    static func create(root: URL, output: URL, arguments: Arguments) throws {
        let target: String
        let runtime: URL
        let text: URL
        if arguments.platform == .android {
            target = "android-arm64"
            let artifacts = try NativeDependencies(toolchain: AndroidToolchain(root: root, arguments: arguments)).prepare(fromSource: true)
            runtime = artifacts.engine; text = artifacts.text
        } else {
            let apple: NativeSDK.AppleTarget?
            if arguments.platform == .ios {
                let sdk = arguments.value(.sdk) ?? "iphonesimulator"
                let architecture = arguments.value(.architecture) ?? "arm64"
                guard ["iphoneos", "iphonesimulator"].contains(sdk), ["arm64", "x86_64"].contains(architecture),
                      sdk != "iphoneos" || architecture == "arm64" else { throw ToolError.invalid("Invalid iOS native target") }
                apple = .init(sdk: sdk, architecture: architecture)
            } else { apple = nil }
            target = apple?.variant ?? CLIUpdate.platform
            try NativeSDK.prepare(root: root, apple: apple, fromSource: true)
            let base = NativeArtifacts.directory(at: root, target: target)
            runtime = base.appendingPathComponent("Runtime/vendor"); text = base.appendingPathComponent("vendor")
        }
        let staging = FileManager.default.temporaryDirectory.appendingPathComponent("guava-native-" + UUID().uuidString)
        defer { try? Files.remove(staging) }
        for path in NativeArtifacts.bundles(for: target) {
            let name = URL(fileURLWithPath: path).lastPathComponent + ".artifactbundle"
            try Files.copy((path.hasPrefix("Runtime/") ? runtime : text).appendingPathComponent(name),
                           to: staging.appendingPathComponent(path + ".artifactbundle"))
        }
        let release = try SDKRelease.read(at: root)
        try Files.text(release.version + "\n", to: staging.appendingPathComponent("VERSION"))
        try Files.directory(output)
        let name = "guava-native-\(target).tar.gz"
        let archive = output.appendingPathComponent(name)
        try Shell.run([TarArchive.executable, "-C", staging.path, "-czf", archive.path, "."])
        let repository = release.repository.replacingOccurrences(of: ".git", with: "")
        let metadata = NativeArtifacts.Archive(url: URL(string: "\(repository)/releases/download/v\(release.version)/\(name)")!,
                                              sha256: try SHA256.hex(file: archive))
        try Files.json(NativeArtifacts(version: release.version, platforms: [target: metadata]),
                       to: output.appendingPathComponent("native-\(target).json"))
        print(archive.path)
    }
}
