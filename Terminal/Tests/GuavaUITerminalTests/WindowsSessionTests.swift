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
            arguments: ["/d", "/c", "echo hello-guava&exit /b 3"]), scrollbackLines: 100) {
                DispatchQueue.main.async(execute: $0)
            }
        try session.start()
        let deadline = ContinuousClock.now + .seconds(10)
        while session.status == .running, ContinuousClock.now < deadline {
            try await Task.sleep(for: .milliseconds(10))
        }
        #expect(session.status == .exited(code: 3))
        let frame = session.frame
        let screen = (0..<frame.rows).map { frame.text(ofRow: $0) }.joined(separator: "\n")
        #expect(screen.contains("hello-guava"))
    }
}
#endif
