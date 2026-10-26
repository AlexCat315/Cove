import Foundation
import Testing
@testable import CoveCLI

@Suite("Project tasks")
struct ProjectTaskTests {
    private func fixture(_ json: String) throws -> URL {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try Files.text("// fixture", to: root.appendingPathComponent("Tasks/Package.swift"))
        try Files.text(json, to: root.appendingPathComponent(ProjectWorkspace.filename))
        return root
    }

    @Test func taskArgumentsRemainLiteral() throws {
        let arguments = try Arguments(["task", "compile", "Some Project", "--", "--help", "@response", "a b"])
        #expect(arguments.operation == .task)
        #expect(arguments.taskName == "compile")
        #expect(arguments.subject == "Some Project")
        #expect(arguments.applicationArguments == ["--help", "@response", "a b"])
    }

    @Test func taskGraphRunsSharedDependenciesOnce() throws {
        let root = try fixture(#"{"tasks":{"prepare":{"package":"Tasks","product":"Tasks","entry":"Prepare.run"},"left":{"package":"Tasks","product":"Tasks","entry":"Left.run","dependsOn":["prepare"]},"right":{"package":"Tasks","product":"Tasks","entry":"Right.run","dependsOn":["prepare"]}}}"#)
        defer { try? Files.remove(root) }
        let workspace = try ProjectWorkspace(at: root)
        #expect(try workspace.orderedTasks(["left", "right"]) == ["prepare", "left", "right"])
        #expect(try ProjectWorkspace.find(from: root.appendingPathComponent("Tasks"))?.directory.path == root.resolvingSymlinksInPath().path)
    }

    @Test(arguments: [
        #"{"tasks":{"bad":{"package":"../outside","product":"Tasks","entry":"Task.run"}}}"#,
        #"{"tasks":{"bad":{"package":"Tasks","product":"Tasks","entry":"Task.run(); exit(0)"}}}"#,
        #"{"tasks":{"loop":{"package":"Tasks","product":"Tasks","entry":"Task.run","dependsOn":["loop"]}}}"#,
        #"{"beforeBuild":["missing"]}"#,
    ])
    func invalidDeclarationsFailBeforeExecution(_ json: String) throws {
        let root = try fixture(json)
        defer { try? Files.remove(root) }
        #expect(throws: (any Error).self) { try ProjectWorkspace(at: root) }
    }

    @Test func runnerGeneratesHostInOwnedCache() throws {
        let root = try fixture(#"{"tasks":{"prepare":{"package":"Tasks","product":"Tasks","entry":"Prepare.run"}}}"#)
        defer { try? Files.remove(root) }
        let workspace = try ProjectWorkspace(at: root)
        let runner = ProjectTaskRunner(arguments: try Arguments(["task", "prepare", root.path]), sdk: root)
        let generated = try runner.prepare(workspace)
        let source = try String(contentsOf: generated.appendingPathComponent("Sources/CoveTask_prepare/main.swift"), encoding: .utf8)
        #expect(source.contains("try Prepare.run(Array(CommandLine.arguments.dropFirst()))"))
        #expect(FileManager.default.fileExists(atPath: generated.appendingPathComponent("Package.swift").path))
        #expect(!FileManager.default.fileExists(atPath: root.appendingPathComponent("cove.cmd").path))
    }
}
