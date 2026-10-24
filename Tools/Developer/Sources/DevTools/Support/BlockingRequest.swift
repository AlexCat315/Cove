internal import Foundation
internal import ToolSupport
private import Synchronization

/// The command-line boundary waits for one asynchronous protocol operation.
enum BlockingRequest {
    static func run<Value: Sendable>(_ operation: @escaping @Sendable () async throws -> Value) throws -> Value {
        let completed = DispatchSemaphore(value: 0)
        let result = Mutex<Result<Value, any Error>?>(nil)
        Task.detached {
            let outcome: Result<Value, any Error>
            do { outcome = .success(try await operation()) } catch { outcome = .failure(error) }
            result.withLock { $0 = outcome }
            completed.signal()
        }
        completed.wait()
        return try result.withLock { try $0!.get() }
    }
}
