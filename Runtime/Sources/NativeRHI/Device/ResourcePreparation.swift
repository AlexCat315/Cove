import Foundation

public struct ResourcePreparationProgress: Sendable, Equatable {
    public enum Phase: String, Sendable { case preparing, finished, cancelled, failed }
    public let phase: Phase
    public let sequence: UInt64
    public let label: String
    public let currentStep: String?
    public let completedSteps: Int
    public let totalSteps: Int?
    public let completedCompilations: Int
    public let currentCompilation: String?
    public let failure: String?
    /// Work-item progress, not an estimate of remaining driver compilation time.
    public var fraction: Double? {
        guard let totalSteps, totalSteps > 0 else { return phase == .finished ? 1 : nil }
        return Double(completedSteps) / Double(totalSteps)
    }
}

/// One preparation runs serially on its worker. Only cancellation crosses
/// threads; progress snapshots are immutable and callbacks are Sendable.
public final class ResourcePreparationContext: @unchecked Sendable {
    private let cancellation = NSLock()
    private var cancelled = false
    private let label: String
    private let total: Int?
    private let observer: @Sendable (ResourcePreparationProgress) -> Void
    private var stepLabel: String?
    private var completed = 0
    private var compilations = 0
    private var compilation: String?
    private var sequence: UInt64 = 0
    private struct OwnedKey: Hashable { let device: ObjectIdentifier; let id: UInt32 }
    private struct Owned { var count: Int; let order: Int; let cleanup: () -> Void }
    private var owned: [OwnedKey: Owned] = [:]
    private var ownershipOrder = 0
    private static let key = "com.cove.rhi.resource-preparation"
    static var current: ResourcePreparationContext? { Thread.current.threadDictionary[key] as? ResourcePreparationContext }

    init(label: String, total: Int?, observer: @escaping @Sendable (ResourcePreparationProgress) -> Void) {
        self.label = label; self.total = total; self.observer = observer
    }
    func cancel() { cancellation.lock(); cancelled = true; cancellation.unlock() }
    public func checkCancellation() throws {
        cancellation.lock(); let value = cancelled; cancellation.unlock()
        if value { throw CancellationError() }
    }
    public func step<T>(_ label: String, _ operation: () throws -> T) throws -> T {
        guard Self.current === self else { throw RHIError.invalidArgument("preparation steps must run inside their preparation operation") }
        try checkCancellation()
        if let total, completed >= total { throw RHIError.invalidArgument("preparation exceeded its declared step count") }
        stepLabel = label; emit()
        let value = try operation()
        try checkCancellation()
        completed += 1; emit()
        return value
    }
    func compilationStarted(_ label: String) throws { try checkCancellation(); compilation = label; emit() }
    func compilationFinished() { compilations += 1; compilation = nil; emit() }
    func acquired(_ device: Device, id: UInt32, cleanup: @escaping () -> Void) {
        let key = OwnedKey(device: ObjectIdentifier(device), id: id)
        if owned[key] != nil { owned[key]!.count += 1 }
        else { ownershipOrder += 1; owned[key] = Owned(count: 1, order: ownershipOrder, cleanup: cleanup) }
    }
    func released(_ device: Device, id: UInt32) {
        let key = OwnedKey(device: ObjectIdentifier(device), id: id)
        guard let value = owned[key] else { return }
        if value.count == 1 { owned[key] = nil } else { owned[key]!.count -= 1 }
    }
    private func discardOwned() {
        let values = owned.values.sorted { $0.order > $1.order }
        owned.removeAll()
        for value in values { for _ in 0..<value.count { value.cleanup() } }
    }
    func emit(_ phase: ResourcePreparationProgress.Phase = .preparing, failure: String? = nil) {
        sequence &+= 1
        observer(ResourcePreparationProgress(phase: phase, sequence: sequence, label: label, currentStep: stepLabel,
            completedSteps: completed, totalSteps: total, completedCompilations: compilations,
            currentCompilation: compilation, failure: failure))
    }
    func run<T>(_ operation: () throws -> sending T) throws -> sending T {
        let previous = Thread.current.threadDictionary[Self.key]
        Thread.current.threadDictionary[Self.key] = self
        defer { Thread.current.threadDictionary[Self.key] = previous }
        do {
            try checkCancellation(); emit()
            let value = try operation()
            try checkCancellation()
            if let total, completed != total { throw RHIError.invalidArgument("preparation did not complete its declared steps") }
            owned.removeAll() // Ownership transfers with the successfully prepared result.
            stepLabel = nil; compilation = nil; emit(.finished)
            return value
        } catch {
            discardOwned()
            emit(error is CancellationError ? .cancelled : .failed, failure: error is CancellationError ? nil : String(describing: error))
            throw error
        }
    }
}

extension Device {
    /// Builds owned resources off the caller's actor. The returned value is
    /// transferred once; callers need not mark mutable renderers Sendable.
    public func prepareResources<T>(label: String, totalSteps: Int? = nil,
        onProgress: @escaping @Sendable (ResourcePreparationProgress) -> Void = { _ in },
        operation: @escaping @Sendable (ResourcePreparationContext) throws -> sending T) async throws -> sending T {
        guard supportsBackgroundCompilation else { throw RHIError.unsupportedFeature("backend does not support background compilation") }
        if let totalSteps, totalSteps < 0 { throw RHIError.invalidArgument("negative preparation step count") }
        let context = ResourcePreparationContext(label: label, total: totalSteps, observer: onProgress)
        return try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { continuation in
                preparationQueue.async {
                    do { continuation.resume(returning: try context.run { try operation(context) }) }
                    catch { continuation.resume(throwing: error) }
                }
            }
        } onCancel: { context.cancel() }
    }
}
