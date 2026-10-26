import Foundation

struct ProjectConfiguration: Codable, Equatable {
    struct Toolchain: Codable, Equatable {
        var sdkVersion: String
    }

    struct Targets: Codable, Equatable {
        var platforms: [AppPlatform] = [.mac, .win, .linux, .android, .ios]
        var desktopExecutable: String? = nil
        var desktopHelpers: [String] = []

        enum CodingKeys: CodingKey { case platforms, desktopExecutable, desktopHelpers }
        init() {}
        init(from decoder: Decoder) throws {
            self.init()
            let values = try decoder.container(keyedBy: CodingKeys.self)
            platforms = try values.decodeIfPresent([AppPlatform].self, forKey: .platforms) ?? platforms
            desktopExecutable = try values.decodeIfPresent(String.self, forKey: .desktopExecutable)
            desktopHelpers = try values.decodeIfPresent([String].self, forKey: .desktopHelpers) ?? []
        }

        func validate() throws {
            guard Set(desktopHelpers).count == desktopHelpers.count,
                  !desktopHelpers.contains(desktopExecutable ?? "Application"),
                  desktopHelpers.allSatisfy({ $0.range(of: #"^[A-Za-z][A-Za-z0-9_-]*$"#, options: .regularExpression) != nil }) else {
                throw ToolError.invalid("targets.desktopHelpers must contain unique executable product names")
            }
            guard !platforms.isEmpty, Set(platforms).count == platforms.count else {
                throw ToolError.invalid("targets.platforms must contain unique platform names")
            }
            guard desktopExecutable == nil || desktopExecutable!.range(of: #"^[A-Za-z][A-Za-z0-9_-]*$"#, options: .regularExpression) != nil else {
                throw ToolError.invalid("targets.desktopExecutable must be an executable product name")
            }
        }
    }

    var application: ApplicationIdentity
    var toolchain: Toolchain
    var targets = Targets()
    var version = ApplicationVersion()

    enum CodingKeys: CodingKey { case application, toolchain, targets, version }
    init(application: ApplicationIdentity, toolchain: Toolchain) {
        self.application = application
        self.toolchain = toolchain
    }
    init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        application = try values.decode(ApplicationIdentity.self, forKey: .application)
        toolchain = try values.decode(Toolchain.self, forKey: .toolchain)
        targets = try values.decodeIfPresent(Targets.self, forKey: .targets) ?? targets
        version = try values.decode(ApplicationVersion.self, forKey: .version)
    }

    func validate() throws {
        try application.validate()
        try targets.validate()
        try version.validate()
        guard SDKRelease.isVersionRequirement(toolchain.sdkVersion) else {
            throw ToolError.invalid("toolchain.sdkVersion must be an exact SDK version or \"latest\"")
        }
    }
}

struct AppProject {
    let directory: URL
    let configuration: ProjectConfiguration
    var desktopProduct: String { configuration.targets.desktopExecutable ?? "Application" }
    var buildDirectory: URL { directory.appendingPathComponent(".build") }

    init(at directory: URL) throws {
        self.directory = directory.standardizedFileURL
        let manifest = directory.appendingPathComponent("cove.json")
        guard FileManager.default.fileExists(atPath: manifest.path),
              FileManager.default.fileExists(atPath: directory.appendingPathComponent("Package.swift").path) else {
            throw ToolError.invalid("\(directory.path) must contain cove.json and Package.swift; use cove create app <directory>")
        }
        configuration = try JSONDecoder().decode(ProjectConfiguration.self, from: Data(contentsOf: manifest))
        try configuration.validate()
    }

    func require(_ platform: AppPlatform) throws {
        try platform.requireAvailable()
        guard configuration.targets.platforms.contains(platform) else {
            throw ToolError.invalid("This application's targets.platforms does not include \(platform.rawValue)")
        }
    }

    /// Re-pins toolchain.sdkVersion in place, preserving the file's formatting.
    func pin(sdkVersion version: String) throws {
        guard SDKRelease.isVersion(version) else { throw ToolError.invalid("Pin an exact SDK version, not \(version)") }
        let manifest = directory.appendingPathComponent("cove.json")
        let expression = try NSRegularExpression(pattern: #"("sdkVersion"\s*:\s*")[^"]*""#)
        let rewritten = NSMutableString(string: try String(contentsOf: manifest, encoding: .utf8))
        let matches = expression.replaceMatches(
            in: rewritten, range: NSRange(location: 0, length: rewritten.length),
            withTemplate: "$1" + NSRegularExpression.escapedTemplate(for: version) + "\"")
        guard matches == 1 else { throw ToolError.invalid("Could not find toolchain.sdkVersion in \(manifest.path)") }
        try Files.text(rewritten as String, to: manifest)
        let updated = try JSONDecoder().decode(ProjectConfiguration.self, from: Data((rewritten as String).utf8))
        guard updated.toolchain.sdkVersion == version else {
            throw ToolError.invalid("Failed to pin SDK \(version) in \(manifest.path)")
        }
    }

    func clean() throws {
        var builds = [buildDirectory]
        let legacy = directory.appendingPathComponent(".cove/build")
        let legacyIsLink = (try? legacy.resourceValues(forKeys: [.isSymbolicLinkKey]).isSymbolicLink) == true
            || (try? legacy.deletingLastPathComponent().resourceValues(forKeys: [.isSymbolicLinkKey]).isSymbolicLink) == true
        if !legacyIsLink { builds.append(legacy) }
        for build in builds {
            guard (try? build.resourceValues(forKeys: [.isSymbolicLinkKey]).isSymbolicLink) != true,
                  (try? build.deletingLastPathComponent().resourceValues(forKeys: [.isSymbolicLinkKey]).isSymbolicLink) != true else {
                throw ToolError.invalid("Refusing to clean a symlink at \(build.path)")
            }
            let sessions = build.appendingPathComponent("devtools")
            if FileManager.default.fileExists(atPath: sessions.path) {
                for directory in try Files.children(sessions) {
                    guard let data = try? Data(contentsOf: directory.appendingPathComponent("session.json")),
                          let session = try? JSONDecoder().decode(ManagedSession.self, from: data) else { continue }
                    if (try? Inspector.isAlive(session)) == true {
                        throw ToolError.invalid("Stop development session \(session.runtime.id) before cleaning this application's build cache")
                    }
                }
            }
        }
        for build in builds { try Files.remove(build) }
    }
}
