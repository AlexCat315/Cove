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
        let keyPath: AnyKeyPath
        var observed: Any
        /// Typed equality predicate for the field's dynamic type, built once and
        /// memoized per type. `nil` means the field is a reference (or otherwise
        /// non-comparable) and must always invalidate.
        var equal: ((Any, Any) -> Bool)?
    }
    /// A single element of an array-valued field. A write that changes only other
    /// indices — or other fields — does not invalidate the dependent scope.
    struct ElementRead {
        let name: String
        let keyPath: AnyKeyPath
        let index: Int
        var observed: Any
        var equal: ((Any, Any) -> Bool)?
    }
    struct Reader {
        let invalidate: (String?) -> Void
        var whole = false
        var fields: [AnyKeyPath: FieldRead] = [:]
        var elements: [ElementRead] = []
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
                // Reuse the existing entry rather than allocating a new closure
                // on every body re-evaluation; only the captured observed value
                // needs refreshing.
                reader.fields[keyPath] = FieldRead(name: name, keyPath: keyPath, observed: observed, equal: makeEqual(observed))
                readers[scope.id] = reader
            }
        }
        return observed
    }

    func readSignal<Field>(_ keyPath: KeyPath<Value, Field>, name: String) -> Field {
        read(keyPath, name: name)
    }

    /// Track a single element of an array-valued field. The returned value is the
    /// element itself (so `state.items[i]` keeps its concrete type), and the
    /// dependent scope is invalidated only when `items[index]` actually changes.
    func readElement<Element>(_ keyPath: WritableKeyPath<Value, [Element]>, at index: Int, name: String) -> Element {
        let collection = value[keyPath: keyPath]
        let observed: Any = collection.indices.contains(index) ? collection[index] : NSNull()
        if let scope = readScope {
            register(scope)
            readersLock.withLock {
                var reader = readers[scope.id] ?? Reader(invalidate: scope.invalidate)
                reader.elements.append(ElementRead(name: name, keyPath: keyPath as AnyKeyPath,
                                                  index: index, observed: observed, equal: makeEqual(observed)))
                readers[scope.id] = reader
            }
        }
        // Mirror the original `state.items[i]` semantics: out-of-range reads trap,
        // exactly as an untracked subscript would.
        return collection[index]
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
            if reader.whole { reader.invalidate(nil); continue }
            var invalidated = false
            for fr in reader.fields.values {
                if let newField = (newValue as Any)[keyPath: fr.keyPath] {
                    // `equal` is a typed predicate built once at read time, so the
                    // hot write path does a single key-path subscript plus a direct
                    // `==` — no reflection or existential opening per field.
                    let equal = fr.equal.map { $0(fr.observed, newField) } ?? false
                    if !equal {
                        reader.invalidate(fr.name)
                        invalidated = true
                        break
                    }
                }
            }
            if !invalidated {
                for er in reader.elements {
                    guard let newCollection = (newValue as Any)[keyPath: er.keyPath] as? [Any],
                          newCollection.indices.contains(er.index) else {
                        // Array shrank (or element vanished) → treat as a change.
                        reader.invalidate(er.name)
                        invalidated = true
                        break
                    }
                    let newElement = newCollection[er.index]
                    let equal = er.equal.map { $0(er.observed, newElement) } ?? false
                    if !equal {
                        reader.invalidate(er.name)
                        invalidated = true
                        break
                    }
                }
            }
            if invalidated { continue }
        }
        // Existing views keep their explicit state-write invalidation contract.
        // A tracked view is invalidated by exactly the values its body read.
        if !preciseOwner || snapshot.isEmpty || ownerScope.map({ !StateReadTracking.contains($0) }) != false {
            onChange?()
        }
    }
}

private enum StateFieldTypes {
    static let scalars = Set([Bool.self, Int.self, Int8.self, Int16.self, Int32.self, Int64.self,
        UInt.self, UInt8.self, UInt16.self, UInt32.self, UInt64.self, Float.self, Double.self,
        String.self, Character.self, UUID.self, Date.self, CGFloat.self].map(ObjectIdentifier.init))
}

/// A reference hidden inside Equatable can mutate both the retained read and
/// the replacement at once. Opaque/custom reflection must never prove equality.
/// The per-type result is memoized so hot `set` paths avoid re-reflecting the
/// same dynamic types on every write.
nonisolated(unsafe) private var valueTypeCache: [ObjectIdentifier: Bool] = [:]
private let valueTypeCacheLock = NSLock()

private func isValueType(_ value: Any) -> Bool {
    let id = ObjectIdentifier(type(of: value))
    if let cached = valueTypeCacheLock.withLock({ valueTypeCache[id] }) { return cached }
    let result = _isValueType(of: value)
    valueTypeCacheLock.withLock { valueTypeCache[id] = result }
    return result
}

private func _isValueType(of value: Any) -> Bool {
    if StateFieldTypes.scalars.contains(ObjectIdentifier(type(of: value))) { return true }
    let mirror = Mirror(reflecting: value)
    guard mirror.displayStyle != .class else { return false }
    // Optionals are value-semantic: recurse into the wrapped value. `Optional`
    // is itself `CustomReflectable`, so it must be handled before the
    // custom-reflection rejection below (which exists to treat opaque,
    // reference-hiding custom reflections as non-value types).
    if mirror.displayStyle == .optional {
        return mirror.children.allSatisfy { isValueType($0.value) }
    }
    if value is any CustomReflectable {
        let name = String(reflecting: type(of: value))
        guard ["Swift.Array<", "Swift.Dictionary<", "Swift.Set<", "Swift.SIMD"].contains(where: name.hasPrefix) else { return false }
    }
    return mirror.children.allSatisfy { isValueType($0.value) }
}

/// Build a typed equality predicate for `sample`'s dynamic type. The predicate
/// captures the concrete `Equatable` conformance so the comparison on the hot
/// write path is a direct `==` (no existential opening, no reflection). Reference
/// types (and anything `isValueType` rejects) yield `nil`, signalling that the
/// field must always invalidate. Results are memoized per type.
nonisolated(unsafe) private var comparatorCache: [ObjectIdentifier: ((Any, Any) -> Bool)?] = [:]
private let comparatorCacheLock = NSLock()

private func makeEqual(_ sample: Any) -> ((Any, Any) -> Bool)? {
    let id = ObjectIdentifier(type(of: sample))
    if let cached = comparatorCacheLock.withLock({ comparatorCache[id] }) { return cached }
    let result: ((Any, Any) -> Bool)?
    if isValueType(sample), let equatable = sample as? any Equatable {
        func build<E: Equatable>(_ proto: E) -> (Any, Any) -> Bool {
            { lhs, rhs in
                guard let lhs = lhs as? E, let rhs = rhs as? E else { return false }
                return lhs == rhs
            }
        }
        result = build(equatable)
    } else {
        result = nil
    }
    comparatorCacheLock.withLock { comparatorCache[id] = result }
    return result
}
