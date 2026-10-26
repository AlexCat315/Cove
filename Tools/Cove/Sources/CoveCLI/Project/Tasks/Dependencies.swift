internal import Foundation
private import ToolSupport

/// SwiftPM owns resolution and lockfiles. Cove only decides when declared branches refresh.
enum WorkspaceDependencies {
    private struct Lockfile: Decodable {
        struct Pin: Decodable {
            struct State: Decodable { let branch: String?; let revision: String? }
            let identity: String
            let location: String
            let state: State
        }
        let pins: [Pin]
    }

    static func refresh(_ workspace: ProjectWorkspace, sdk: URL,
                        environment: [String: String] = ProcessInfo.processInfo.environment) throws {
        guard workspace.configuration.dependencyUpdates == .branches,
              environment["CI", default: ""].isEmpty else { return }
        var heads: [String: String] = [:]
        let buildEnvironment = try SwiftPackage.environment(root: sdk)
        for path in workspace.configuration.packages {
            let package = try workspace.package(path)
            guard let data = try? Data(contentsOf: package.appendingPathComponent("Package.resolved")),
                  let lock = try? JSONDecoder().decode(Lockfile.self, from: data) else { continue }
            var changed: [String] = []
            for pin in lock.pins {
                guard let branch = pin.state.branch else { continue }
                let ref = "refs/heads/" + branch
                let key = pin.location + "\n" + ref
                if heads[key] == nil {
                    guard let output = try? Shell.run(["git", "-c", "http.lowSpeedLimit=1000", "-c", "http.lowSpeedTime=5",
                                                      "ls-remote", pin.location, ref], capture: true, quiet: true),
                          let head = output.split(whereSeparator: { $0.isWhitespace }).first else { continue }
                    heads[key] = String(head)
                }
                if heads[key] == pin.state.revision { continue }
                let repositories = package.appendingPathComponent(".build/repositories")
                let cached = (try? Files.children(repositories)) ?? []
                for repository in cached where repository.lastPathComponent.lowercased().hasPrefix(pin.identity.lowercased() + "-") {
                    try Shell.run(["git", "-C", repository.path, "fetch", "origin"], environment: buildEnvironment)
                }
                changed.append(pin.identity)
            }
            if !changed.isEmpty {
                try Shell.run([CoveSDK.swift, "package", "--package-path", package.path, "--skip-update", "update"] + changed,
                              environment: buildEnvironment)
            }
        }
    }
}
