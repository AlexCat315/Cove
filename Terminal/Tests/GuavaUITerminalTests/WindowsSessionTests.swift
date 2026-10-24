#if os(Windows)
import Foundation
import Testing
@testable import GuavaUITerminal

@Suite("Windows terminal session", .serialized)
@MainActor
struct WindowsSessionTests {
    private func run(
        _ executable: String, _ arguments: [String], waitForOutput: Bool = true
    ) async throws -> (status: TerminalSession.Status, screen: String) {
        let session = TerminalSession(command: TerminalCommand(executable: executable,
            arguments: arguments), scrollbackLines: 100) {
                DispatchQueue.main.async(execute: DispatchWorkItem(block: $0))
            }
        try session.start()
        let deadline = ContinuousClock.now + .seconds(10)
        while session.status == .running, ContinuousClock.now < deadline {
            try await Task.sleep(for: .milliseconds(10))
        }
        let outputDeadline = ContinuousClock.now + .seconds(10)
        var screen = ""
        while waitForOutput, ContinuousClock.now < outputDeadline {
            let frame = session.frame
            screen = (0..<frame.rows).map { frame.text(ofRow: $0) }.joined(separator: "\n")
            if !screen.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { break }
            try await Task.sleep(for: .milliseconds(10))
        }
        return (session.status, screen)
    }

    @Test("ConPTY runs a program and reports its exit code")
    func exitCode() async throws {
        let result = try await run("powershell.exe", ["-NoProfile", "-Command", "exit 5"], waitForOutput: false)
        #expect(result.status == .exited(code: 5))
    }

    @Test("ConPTY delivers program output")
    func output() async throws {
        let result = try await run("powershell.exe", ["-NoProfile", "-Command", "Write-Output hello-guava"])
        #expect(result.status == .exited(code: 0))
        #expect(result.screen.contains("hello-guava"))
    }

    @Test("ConPTY forwards cmd switches and their command")
    func cmdCommandLine() async throws {
        let result = try await run("cmd.exe", ["/d", "/c", "echo", "hello-guava"])
        #expect(result.status == .exited(code: 0))
        #expect(result.screen.contains("hello-guava"))
    }
}
#endif
