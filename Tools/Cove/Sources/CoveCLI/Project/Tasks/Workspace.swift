internal import Foundation

struct ProjectWorkspace {
    static let filename = "cove.workspace.json"

    struct Task: Decodable, Equatable {
        let package: String
        let product: String
        let entry: String
        let module: String
        let arguments: [String]
        let dependsOn: [String]
        let platforms: [AppPlatform]?

        enum CodingKeys: CodingKey { case package, product, entry, module, arguments, dependsOn, platforms }
        init(from decoder: Decoder) throws {
            let c = try decoder.container(keyedBy: CodingKeys.self)
            package = try c.decode(String.self, forKey: .package)
            product = try c.decode(String.self, forKey: .product)
            entry = try c.decode(String.self, forKey: .entry)
            module = try c.decodeIfPresent(String.self, forKey: .module) ?? product
            arguments = try c.decodeIfPresent([String].self, forKey: .arguments) ?? []
            dependsOn = try c.decodeIfPresent([String].self, forKey: .dependsOn) ?? []
            platforms = try c.decodeIfPresent([AppPlatform].self, forKey: .platforms)
        }
    }

    struct Configuration: Decodable {
        enum DependencyUpdates: String, Decodable { case locked, branches }
        let packages: [String]
        let dependencyUpdates: DependencyUpdates
        let beforeBuild: [String]
        let tasks: [String: Task]

        enum CodingKeys: CodingKey { case packages, dependencyUpdates, beforeBuild, tasks }
        init(from decoder: Decoder) throws {
            let c = try decoder.container(keyedBy: CodingKeys.self)
            packages = try c.decodeIfPresent([String].self, forKey: .packages) ?? []
            dependencyUpdates = try c.decodeIfPresent(DependencyUpdates.self, forKey: .dependencyUpdates) ?? .locked
            beforeBuild = try c.decodeIfPresent([String].self, forKey: .beforeBuild) ?? []
            tasks = try c.decodeIfPresent([String: Task].self, forKey: .tasks) ?? [:]
        }
    }

    let directory: URL
    let configuration: Configuration

    init(at directory: URL) throws {
        self.directory = directory.standardizedFileURL.resolvingSymlinksInPath()
        configuration = try JSONDecoder().decode(Configuration.self,
            from: Data(contentsOf: self.directory.appendingPathComponent(Self.filename)))
        try validate()
    }

    static func find(from directory: URL) throws -> ProjectWorkspace? {
        var current = directory.standardizedFileURL.resolvingSymlinksInPath()
        while true {
            if FileManager.default.fileExists(atPath: current.appendingPathComponent(filename).path) {
                return try ProjectWorkspace(at: current)
            }
            let parent = current.deletingLastPathComponent()
            if parent == current { return nil }
            current = parent
        }
    }

    func package(_ relative: String) throws -> URL {
        guard !relative.isEmpty, !(relative as NSString).isAbsolutePath else {
            throw ToolError.invalid("Task packages must be relative to the workspace")
        }
        let result = directory.appendingPathComponent(relative).standardizedFileURL.resolvingSymlinksInPath()
        guard result == directory || result.path.hasPrefix(directory.path + "/"),
              FileManager.default.fileExists(atPath: result.appendingPathComponent("Package.swift").path) else {
            throw ToolError.invalid("Workspace package must exist inside the project: \(relative)")
        }
        return result
    }

    func orderedTasks(_ names: [String]) throws -> [String] {
        var active = Set<String>(), complete = Set<String>(), ordered: [String] = []
        func visit(_ name: String) throws {
            if complete.contains(name) { return }
            guard let task = configuration.tasks[name] else { throw ToolError.invalid("Unknown project task: \(name)") }
            guard active.insert(name).inserted else { throw ToolError.invalid("Project task dependency cycle at \(name)") }
            for dependency in task.dependsOn { try visit(dependency) }
            active.remove(name)
            complete.insert(name)
            ordered.append(name)
        }
        for name in names { try visit(name) }
        return ordered
    }

    private func validate() throws {
        var targetNames = Set<String>()
        func matches(_ value: String, _ pattern: String) -> Bool {
            value.range(of: pattern, options: .regularExpression) != nil
        }
        for (name, task) in configuration.tasks {
            guard matches(name, #"^[A-Za-z][A-Za-z0-9_-]*$"#),
                  matches(task.product, #"^[A-Za-z_][A-Za-z0-9_-]*$"#),
                  matches(task.module, #"^[A-Za-z_][A-Za-z0-9_]*$"#),
                  matches(task.entry, #"^[A-Za-z_][A-Za-z0-9_]*\.[A-Za-z_][A-Za-z0-9_]*$"#),
                  targetNames.insert(name.replacingOccurrences(of: "-", with: "_")).inserted else {
                throw ToolError.invalid("Invalid project task declaration: \(name)")
            }
            _ = try package(task.package)
        }
        for path in configuration.packages { _ = try package(path) }
        _ = try orderedTasks(Array(configuration.tasks.keys) + configuration.beforeBuild)
    }
}
