private import Foundation
private import Testing
private import GuavaProcess

#if os(macOS) || os(Linux) || os(Windows)
@Suite("Cross-platform process contracts", .timeLimit(.minutes(1)))
struct ProcessContracts {
    @Test func concurrentSynchronousDiscovery() async throws {
        let request = try command("output")
        try await withThrowingTaskGroup(of: ProcessOutput.self) { group in
            for _ in 0..<min(32, max(8, ProcessInfo.processInfo.activeProcessorCount * 2)) {
                group.addTask { try ProcessRunner.runSynchronously(request) }
            }
            for try await output in group {
                #expect(output.status == 0 && output.stdoutText == "stdout" && output.stderrText == "stderr")
            }
        }
    }

    @Test func shortOutputIsDeliveredBeforeEOF() throws {
        let session = try ProcessSession(command: command("interactive"))
        defer { session.cancel() }
        var bytes = Data()
        let deadline = Date().addingTimeInterval(3)
        while bytes.count < 5, let event = try session.nextEvent(until: deadline) {
            if case let .stdout(data) = event { bytes.append(data) }
        }
        #expect(String(decoding: bytes, as: UTF8.self) == "ready")
        try session.write(Data("ping".utf8))
        while bytes.count < 9, let event = try session.nextEvent(until: deadline) {
            if case let .stdout(data) = event { bytes.append(data) }
        }
        #expect(String(decoding: bytes, as: UTF8.self) == "readyping")
        session.closeInput()
        while let _ = try session.nextEvent(until: deadline) {}
    }

    @Test func bothStreamsAndEnvironmentSurviveLaunch() throws {
        let output = try ProcessRunner.runSynchronously(command("output"))
        #expect(output.status == 0)
        #expect(String(decoding: output.stdout, as: UTF8.self) == "stdout")
        #expect(String(decoding: output.stderr, as: UTF8.self) == "stderr")
        var request = try command("environment")
        request.environment["GUAVA_FIXTURE_VALUE"] = "value"
        let result = try ProcessRunner.runSynchronously(request)
        #expect(String(decoding: result.stdout, as: UTF8.self) == "value")
    }

    @Test func deadlineCoversDescendantsHoldingPipesAfterParentExit() throws {
        var budget = ProcessBudget()
        budget.lifetimeMilliseconds = 200
        let session = try ProcessSession(command: command("descendant"), budget: budget)
        defer { session.cancel() }
        session.closeInput()
        let start = Date()
        #expect(throws: ProcessError.timedOut) {
            while let _ = try session.nextEvent(until: Date().addingTimeInterval(3)) {}
        }
        #expect(Date().timeIntervalSince(start) < 2)
    }

    @Test func cancellationWakesAnIdleReader() throws {
        let session = try ProcessSession(command: command("sleep"))
        session.cancel()
        #expect(throws: CancellationError.self) { while let _ = try session.nextEvent() {} }
    }

    @Test func nativeArgumentsKeepQuotesWhitespaceAndBackslashes() throws {
        var request = try command("arguments")
        let arguments = ["", "space value", "🥭 中文", "quoted\"value", "trailing\\", "&|%PATH%"]
        request.arguments += arguments
        let result = try ProcessRunner.runSynchronously(request)
        #expect(try JSONDecoder().decode([String].self, from: result.stdout) == arguments)
    }

    @Test func queuedInputDoesNotKeepAnAbandonedSessionAlive() throws {
        var budget = ProcessBudget()
        budget.lifetimeMilliseconds = nil
        var session: ProcessSession? = try ProcessSession(command: command("sleep"), budget: budget)
        weak var reference = session
        try session?.write(Data(repeating: 1, count: 1_048_576))
        session = nil
        #expect(reference == nil)
    }

    private func command(_ mode: String) throws -> ProcessCommand {
        #if os(Windows)
        let name = "GuavaProcessFixture.exe"
        #else
        let name = "GuavaProcessFixture"
        #endif
        let binaryDirectory = URL(fileURLWithPath: CommandLine.arguments[0]).deletingLastPathComponent()
        let candidates = [binaryDirectory.appendingPathComponent(name),
                          Bundle(for: ProcessTestBundle.self).bundleURL.deletingLastPathComponent().appendingPathComponent(name)]
        let executable = try #require(candidates.first { FileManager.default.isExecutableFile(atPath: $0.path) })
        var result = ProcessCommand(executable: executable, directory: FileManager.default.temporaryDirectory)
        result.arguments = [mode]
        result.environment = ProcessInfo.processInfo.environment
        return result
    }
}

private final class ProcessTestBundle: NSObject {}
#endif
