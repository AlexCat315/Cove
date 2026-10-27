import Foundation

/// Composition scopes supply invalidation without introducing a Core → Scene dependency.
public enum StateReadTracking {
    public struct Scope {
        public let id: ObjectIdentifier
        public let invalidate: (String?) -> Void
        public init(id: ObjectIdentifier, invalidate: @escaping (String?) -> Void) {
            self.id = id
            self.invalidate = invalidate
        }
    }

    private final class StackBox {
        var stack: [Scope] = []
        var deferredDepth = 0
    }
    private static let storageKey = "cove.ui.StateReadTracking.stack"
    private static let lock = NSLock()
    nonisolated(unsafe) private static var cleaners: [ObjectIdentifier: [ObjectIdentifier: (ObjectIdentifier) -> Void]] = [:]
    nonisolated(unsafe) private static var installed: Set<ObjectIdentifier> = []
    #if os(WASI)
    nonisolated(unsafe) private static let stack = StackBox()
    #else
    private static var stack: StackBox {
        if let box = Thread.current.threadDictionary[storageKey] as? StackBox { return box }
        let box = StackBox()
        Thread.current.threadDictionary[storageKey] = box
        return box
    }
    #endif

    static var current: Scope? { stack.stack.last }
    static var isDeferredEvaluation: Bool { stack.deferredDepth > 0 }

    /// Layout callbacks and painters run after body evaluation. Reads there
    /// still subscribe to their state's retained owner; input actions do not.
    public static func withDeferredEvaluation<R>(_ body: () throws -> R) rethrows -> R {
        let box = stack
        box.deferredDepth += 1
        defer { box.deferredDepth -= 1 }
        return try body()
    }
    static func contains(_ id: ObjectIdentifier) -> Bool { lock.withLock { installed.contains(id) } }

    public static func withScope<R>(_ scope: Scope, _ body: () throws -> R) rethrows -> R {
        clear(scope.id)
        _ = lock.withLock { installed.insert(scope.id) }
        let box = stack
        box.stack.append(scope)
        defer { _ = box.stack.popLast() }
        return try body()
    }

    static func register(owner: ObjectIdentifier, scope: ObjectIdentifier,
                         clean: @escaping (ObjectIdentifier) -> Void) {
        lock.withLock { cleaners[scope, default: [:]][owner] = clean }
    }

    public static func removeScope(_ id: ObjectIdentifier) {
        _ = lock.withLock { installed.remove(id) }
        clear(id)
    }

    private static func clear(_ id: ObjectIdentifier) {
        let callbacks = lock.withLock { cleaners.removeValue(forKey: id).map { Array($0.values) } ?? [] }
        for callback in callbacks { callback(id) }
    }
}

extension StateStorage {
    struct FieldRead {
        let name: String
        let changed: (Value) -> Bool
    }
    struct Reader {
        let invalidate: (String?) -> Void
        var whole = false
        var fields: [AnyKeyPath: FieldRead] = [:]
    }

    private var readScope: StateReadTracking.Scope? {
        if let current = StateReadTracking.current { return current }
        guard preciseOwner, StateReadTracking.isDeferredEvaluation,
              let ownerScope, let onChange else { return nil }
        return .init(id: ownerScope, invalidate: { _ in onChange() })
    }

    func readWhole() {
        guard preciseOwner, let scope = readScope else { return }
        register(scope)
        readersLock.withLock {
            var reader = readers[scope.id] ?? Reader(invalidate: scope.invalidate)
            reader.whole = true
            readers[scope.id] = reader
        }
    }

    func read<Field>(_ keyPath: KeyPath<Value, Field>, name: String) -> Field {
        let observed = value[keyPath: keyPath]
        if let scope = readScope {
            register(scope)
            readersLock.withLock {
                var reader = readers[scope.id] ?? Reader(invalidate: scope.invalidate)
                reader.fields[keyPath] = FieldRead(name: name, changed: { new in
                    !stateFieldEqual(observed, new[keyPath: keyPath])
                })
                readers[scope.id] = reader
            }
        }
        return observed
    }

    private func register(_ scope: StateReadTracking.Scope) {
        StateReadTracking.register(owner: ObjectIdentifier(self), scope: scope.id) { [weak self] id in
            guard let self else { return }
            _ = self.readersLock.withLock { self.readers.removeValue(forKey: id) }
        }
    }

    func set(_ newValue: Value) {
        value = newValue
        let snapshot = readersLock.withLock { readers }
        for reader in snapshot.values {
            if reader.whole { reader.invalidate(nil) }
            else if let field = reader.fields.values.first(where: { $0.changed(newValue) }) {
                reader.invalidate(field.name)
            }
        }
        // Existing views keep their explicit state-write invalidation contract.
        // A tracked view is invalidated by exactly the values its body read.
        if !preciseOwner || snapshot.isEmpty || ownerScope.map({ !StateReadTracking.contains($0) }) != false {
            onChange?()
        }
    }
}

private func stateFieldEqual<T>(_ lhs: T, _ rhs: T) -> Bool {
    var budget = 2_048
    guard stateFieldIsValue(lhs, budget: &budget), stateFieldIsValue(rhs, budget: &budget),
          let equatable = lhs as? any Equatable else { return false }
    func equal<E: Equatable>(_ value: E, _ other: Any) -> Bool { (other as? E).map { value == $0 } ?? false }
    return equal(equatable, rhs)
}

private enum StateFieldTypes {
    static let scalars = Set([Bool.self, Int.self, Int8.self, Int16.self, Int32.self, Int64.self,
        UInt.self, UInt8.self, UInt16.self, UInt32.self, UInt64.self, Float.self, Double.self,
        String.self, Character.self, UUID.self, Date.self, CGFloat.self].map(ObjectIdentifier.init))
}

/// A reference hidden inside Equatable can mutate both the retained read and
/// the replacement at once. Opaque/custom reflection must never prove equality.
private func stateFieldIsValue(_ value: Any, budget: inout Int) -> Bool {
    budget -= 1
    guard budget >= 0 else { return false }
    if StateFieldTypes.scalars.contains(ObjectIdentifier(type(of: value))) { return true }
    let mirror = Mirror(reflecting: value)
    guard mirror.displayStyle != .class else { return false }
    if value is any CustomReflectable {
        let name = String(reflecting: type(of: value))
        guard ["Swift.Array<", "Swift.Dictionary<", "Swift.Set<", "Swift.SIMD"].contains(where: name.hasPrefix) else { return false }
    }
    return mirror.children.allSatisfy { stateFieldIsValue($0.value, budget: &budget) }
}
