import CoveUICore
import Foundation
#if canImport(Observation)
private import Observation
#endif

private final class ObservationReadTracking: @unchecked Sendable {
    private let lock = NSLock()
    private var onChange: (() -> Void)?
    init(onChange: @escaping () -> Void) { self.onChange = onChange }
    func cancel() { lock.withLock { onChange = nil } }
    func changed() {
        let callback = lock.withLock {
            let callback = onChange
            onChange = nil
            return callback
        }
        callback?()
    }
}

public struct ObservableStateScope {
    fileprivate let observation: ObservationReadTracking
    public let id: ObjectIdentifier
    public let invalidate: () -> Void
    public let invalidateTracked: ((RecompositionReason) -> Void)?

    public init(id: ObjectIdentifier,
                invalidate: @escaping () -> Void,
                invalidateTracked: ((RecompositionReason) -> Void)? = nil) {
        self.id = id
        self.invalidate = invalidate
        self.invalidateTracked = invalidateTracked
        observation = ObservationReadTracking(onChange: invalidate)
    }
}

public enum ObservableStateTracking {
    public typealias DependencyCleaner = (ObjectIdentifier) -> Void

    private final class StackBox {
        var stack: [ObservableStateScope] = []
    }

    private static let stackStorageKey = "cove.ui.ObservableStateTracking.stack"
    private static let cleanersLock = NSLock()
    nonisolated(unsafe) private static var cleanersByScope: [ObjectIdentifier: [ObjectIdentifier: DependencyCleaner]] = [:]

    public static var current: ObservableStateScope? {
        stackBox.stack.last
    }

    #if os(WASI)
    nonisolated(unsafe) private static let stackBox = StackBox()
    #else
    private static var stackBox: StackBox {
        let dict = Thread.current.threadDictionary
        if let existing = dict[stackStorageKey] as? StackBox {
            return existing
        }
        let fresh = StackBox()
        dict[stackStorageKey] = fresh
        return fresh
    }

    #endif

    public static func registerDependencyCleaner(for scopeID: ObjectIdentifier,
                                                 ownerID: ObjectIdentifier,
                                                 _ cleaner: @escaping DependencyCleaner) {
        cleanersLock.withLock {
            cleanersByScope[scopeID, default: [:]][ownerID] = cleaner
        }
    }

    @discardableResult
    public static func withScope<R>(id: ObjectIdentifier,
                                    invalidate: @escaping () -> Void,
                                    invalidateTracked: ((RecompositionReason) -> Void)? = nil,
                                    _ body: () throws -> R) rethrows -> R {
        clearDependencies(for: id)
        let stack = stackBox
        let scope = ObservableStateScope(id: id, invalidate: invalidate, invalidateTracked: invalidateTracked)
        registerDependencyCleaner(for: id, ownerID: ObjectIdentifier(scope.observation)) { _ in scope.observation.cancel() }
        stack.stack.append(scope)
        defer { _ = stack.stack.popLast() }
        return try body()
    }

    /// Track a single evaluation boundary. Do not wrap recursive reconciliation:
    /// otherwise parents would also observe every property read by their children.
    public static func observeReads<R>(_ body: () -> R) -> R {
        #if canImport(Observation)
        guard let observation = current?.observation else { return body() }
        return withObservationTracking(body, onChange: { [weak observation] in observation?.changed() })
        #else
        return body()
        #endif
    }

    public static func removeScope(id: ObjectIdentifier) {
        clearDependencies(for: id)
    }

    private static func clearDependencies(for scopeID: ObjectIdentifier) {
        let cleaners = cleanersLock.withLock {
            cleanersByScope.removeValue(forKey: scopeID).map { Array($0.values) } ?? []
        }
        for cleaner in cleaners {
            cleaner(scopeID)
        }
    }
}

public final class ObservableStateRegistrar: @unchecked Sendable {
    private struct Observer {
        var id: ObjectIdentifier
        var invalidate: () -> Void
        var invalidateTracked: ((RecompositionReason) -> Void)?
    }

    private let lock = NSLock()
    private var observersByKey: [AnyHashable: [ObjectIdentifier: Observer]] = [:]
    private var keysByScope: [ObjectIdentifier: Set<AnyHashable>] = [:]

    public init() {}

    public func access(_ key: AnyHashable) {
        guard let scope = ObservableStateTracking.current else { return }
        let ownerID = ObjectIdentifier(self)
        ObservableStateTracking.registerDependencyCleaner(for: scope.id,
                                                          ownerID: ownerID) { [weak self] scopeID in
            self?.removeDependencies(for: scopeID)
        }
        lock.withLock {
            observersByKey[key, default: [:]][scope.id] = Observer(id: scope.id,
                                                                    invalidate: scope.invalidate, invalidateTracked: scope.invalidateTracked)
            keysByScope[scope.id, default: []].insert(key)
        }
    }

    public func invalidate(_ key: AnyHashable) {
        let observers: [Observer] = lock.withLock {
            guard let keyed = observersByKey[key] else { return [] }
            return Array(keyed.values)
        }
        for observer in observers {
            if let tracked = observer.invalidateTracked { tracked(.init(kind: "observable", detail: String(describing: key))) }
            else { observer.invalidate() }
        }
    }

    public func invalidateAll() {
        let observers = lock.withLock {
            var byScope: [ObjectIdentifier: Observer] = [:]
            for keyedObservers in observersByKey.values {
                for observer in keyedObservers.values {
                    byScope[observer.id] = observer
                }
            }
            return Array(byScope.values)
        }
        for observer in observers {
            if let tracked = observer.invalidateTracked { tracked(.init(kind: "observable", detail: "all fields")) }
            else { observer.invalidate() }
        }
    }

    public func removeDependencies(for scopeID: ObjectIdentifier) {
        lock.withLock {
            guard let keys = keysByScope.removeValue(forKey: scopeID) else { return }
            for key in keys {
                observersByKey[key]?.removeValue(forKey: scopeID)
                if observersByKey[key]?.isEmpty == true {
                    observersByKey.removeValue(forKey: key)
                }
            }
        }
    }
}
