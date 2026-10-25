internal import Foundation

final class ProcessIOState: @unchecked Sendable {
    private let lock = NSLock()
    private var pendingInput = 0
    private var pipesClosed = 0
    private var exitStatus: Int32?
    private var stopped = false
    private var inputClosed = false
    private var timer: DispatchSourceTimer?

    func installTimer(_ timer: DispatchSourceTimer) {
        lock.withLock { if stopped { timer.cancel() } else { self.timer = timer } }
    }
    func cancelTimer() { lock.withLock { timer?.cancel(); timer = nil } }

    func enqueue(_ count: Int, maximum: Int) throws {
        try lock.withLock {
            guard !stopped, !inputClosed, exitStatus == nil else { throw ProcessError.stopped }
            guard count <= maximum - pendingInput else { throw ProcessError.inputLimit }
            pendingInput += count
        }
    }
    func wrote(_ count: Int) { lock.withLock { pendingInput -= count } }
    func closeInput() { lock.withLock { inputClosed = true } }
    func stop() -> Bool { lock.withLock { let first = !stopped; stopped = true; return first } }
    func closedPipe() -> Int32? { lock.withLock { pipesClosed += 1; return pipesClosed == 2 ? exitStatus : nil } }
    func terminated(_ status: Int32) -> Bool { lock.withLock { exitStatus = status; return pipesClosed == 2 } }
}
