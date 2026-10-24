#if os(Windows)
import Foundation
import Testing
@testable import GuavaUITerminal

@Suite("Windows terminal session", .serialized)
@MainActor
struct WindowsSessionTests {
    @Test("ConPTY delivers command output and the process exit code")
    func outputAndExit() async throws {
        let session = TerminalSession(command: TerminalCommand(executable: "cmd.exe",
            arguments: ["/d", "/s", "/c", "echo hello-guava&exit /b 3"]), scrollbackLines: 100) {
                DispatchQueue.main.async(execute: DispatchWorkItem(block: $0))
            }
        try session.start()
        let deadline = ContinuousClock.now + .seconds(10)
        while session.status == .running, ContinuousClock.now < deadline {
            try await Task.sleep(for: .milliseconds(10))
        }
        #expect(session.status == .exited(code: 3))
        let outputDeadline = ContinuousClock.now + .seconds(10)
        var screen = ""
        while ContinuousClock.now < outputDeadline {
            let frame = session.frame
            screen = (0..<frame.rows).map { frame.text(ofRow: $0) }.joined(separator: "\n")
            if screen.contains("hello-guava") { break }
            try await Task.sleep(for: .milliseconds(10))
        }
        #expect(screen.contains("hello-guava"))
    }
}
#endif
