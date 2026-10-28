import Foundation
#if canImport(Darwin)
import Darwin
#elseif canImport(Glibc)
import Glibc
#elseif canImport(Musl)
import Musl
#elseif canImport(WinSDK)
import WinSDK
#endif

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
    private static let lock = NSLock()
    nonisolated(unsafe) private static var cleaners: [ObjectIdentifier: [ObjectIdentifier: (ObjectIdentifier) -> Void]] = [:]
    nonisolated(unsafe) private static var installed: Set<ObjectIdentifier> = []
    /// The per-thread scope stack.
    ///
    /// Recording a `@State` read happens on every field read, so this must be a
    /// raw thread-local load rather than a Foundation `Thread.threadDictionary`
    /// access — that bridges to an `NSDictionary` and hashes a string on every
    /// call, which dominated this path.
    private static var stack: StackBox {
        if let existing = StackTLS.load() { return existing }
        let box = StackBox()
        StackTLS.store(box)
        return box
    }

    /// A native thread-local slot, one per platform.
    ///
    /// POSIX/Darwin use pthread TLS. Windows uses fiber-local storage, which is
    /// the only Windows thread-local facility that runs a destructor when a
    /// thread exits (plain `TlsAlloc` has none). Single-threaded WASI keeps a
    /// single process-wide slot instead.
    private enum StackTLS {
        #if os(WASI)
        nonisolated(unsafe) private static var slot: StackBox?
        static func load() -> StackBox? { slot }
        static func store(_ box: StackBox) { slot = box }
        #elseif canImport(Darwin) || canImport(Glibc) || canImport(Musl)
        private static let key: pthread_key_t = {
            var key = pthread_key_t()
            pthread_key_create(&key) { raw in
                Unmanaged<StackBox>.fromOpaque(raw).release()
            }
            return key
        }()
        static func load() -> StackBox? {
            guard let raw = pthread_getspecific(key) else { return nil }
            return Unmanaged<StackBox>.fromOpaque(raw).takeUnretainedValue()
        }
        static func store(_ box: StackBox) {
            pthread_setspecific(key, Unmanaged.passRetained(box).toOpaque())
        }
        #elseif canImport(WinSDK)
        private static let index: DWORD = FlsAlloc { raw in
            Unmanaged<StackBox>.fromOpaque(raw).release()
        }
        static func load() -> StackBox? {
            guard index != FLS_OUT_OF_INDEXES, let raw = FlsGetValue(index) else { return nil }
            return Unmanaged<StackBox>.fromOpaque(raw).takeUnretainedValue()
        }
        static func store(_ box: StackBox) {
            guard index != FLS_OUT_OF_INDEXES else { return }
            FlsSetValue(index, Unmanaged.passRetained(box).toOpaque())
        }
        #else
        // Unknown platform: portable (slower) Foundation fallback.
        private static let key = "cove.ui.StateReadTracking.stack"
        static func load() -> StackBox? { Thread.current.threadDictionary[key] as? StackBox }
        static func store(_ box: StackBox) { Thread.current.threadDictionary[key] = box }
        #endif
    }

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
        /// Dynamic type of `observed` at registration time. When a later body
        /// evaluation observes the same type, the memoized comparator below is
        /// reused without touching the global comparator cache.
        var typeId: ObjectIdentifier
        /// Typed equality predicate for the field's dynamic type, built once and
        /// memoized per type. `nil` means the field is a reference (or otherwise
        /// non-comparable) and must always invalidate.
        var equal: ((Any, Any) -> Bool)?
    }
    /// A single element of a collection-valued field (array or dictionary). A
    /// write that changes only other indices/keys — or other fields — does not
    /// invalidate the dependent scope.
    struct ElementRead {
        let name: String
        let keyPath: AnyKeyPath
        var observed: Any
        var typeId: ObjectIdentifier
        var equal: ((Any, Any) -> Bool)?
        let selector: Selector
        enum Selector: Hashable {
            case array(Int)
            case dictionary(AnyHashable)
        }
    }
    /// Identifies a tracked element within a scope. Keying `elements` by it keeps
    /// storage bounded by the set of distinct (collection, index/key) pairs a body
    /// actually reads, even when the same element is read repeatedly.
    struct ElementKey: Hashable {
        let keyPath: AnyKeyPath
        let selector: ElementRead.Selector
    }
    /// Reference type on purpose: mutating a struct copy pulled out of the
    /// `readers` dictionary would trigger a COW copy of its nested collections on
    /// every single field read.
    final class Reader {
        let invalidate: (String?) -> Void
        var whole = false
        var fields: [AnyKeyPath: FieldRead] = [:]
        var elements: [ElementKey: ElementRead] = [:]
        init(invalidate: @escaping (String?) -> Void) { self.invalidate = invalidate }
    }

    private var readScope: StateReadTracking.Scope? {
        // A non-precise owner invalidates unconditionally on write, so recorded
        // dependencies would never be read back. Skipping them removes every
        // tracking cost — lock, table write and value boxing — for the majority
        // of views, which never opt into precise tracking.
        guard preciseOwner else { return nil }
        if let current = StateReadTracking.current { return current }
        guard StateReadTracking.isDeferredEvaluation,
              let ownerScope, let onChange else { return nil }
        return .init(id: ownerScope, invalidate: { _ in onChange() })
    }

    /// Single mutation point for the reader table. Registration is scope-wide, so
    /// it is performed only when the reader entry is created — the first read a
    /// body makes against this state — instead of once per field read.
    private func track(_ scope: StateReadTracking.Scope, _ mutate: (inout Reader) -> Void) {
        var needsRegister = false
        readersLock.withLock {
            var reader: Reader
            if let existing = readers[scope.id] {
                reader = existing
            } else {
                reader = Reader(invalidate: scope.invalidate)
                needsRegister = true
            }
            mutate(&reader)
            readers[scope.id] = reader
        }
        // Kept outside `readersLock`: the global registration lock must never be
        // taken while holding it, to avoid a lock-order inversion with `clear`.
        if needsRegister { register(scope) }
    }

    func readWhole() {
        guard preciseOwner, let scope = readScope else { return }
        track(scope) { $0.whole = true }
    }

    func read<Field>(_ keyPath: KeyPath<Value, Field>, name: String) -> Field {
        let observed = value[keyPath: keyPath]
        if let scope = readScope {
            let typeId = ObjectIdentifier(type(of: observed))
            track(scope) { reader in
                if var existing = reader.fields[keyPath], existing.typeId == typeId {
                    // Same dynamic type: reuse the memoized comparator, refresh
                    // only the observed value.
                    existing.observed = observed
                    reader.fields[keyPath] = existing
                } else {
                    reader.fields[keyPath] = FieldRead(name: name, keyPath: keyPath,
                                                       observed: observed, typeId: typeId,
                                       equal: makeEqual(observed))
                }
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
            let typeId = ObjectIdentifier(type(of: observed))
            let key = ElementKey(keyPath: keyPath as AnyKeyPath, selector: .array(index))
            track(scope) { reader in
                if var existing = reader.elements[key], existing.typeId == typeId {
                    existing.observed = observed
                    reader.elements[key] = existing
                } else {
                    reader.elements[key] = ElementRead(name: name, keyPath: keyPath as AnyKeyPath,
                                                       observed: observed, typeId: typeId,
                                                       equal: makeEqual(observed),
                                                       selector: .array(index))
                }
            }
        }
        // Mirror the original `state.items[i]` semantics: out-of-range reads trap,
        // exactly as an untracked subscript would.
        return collection[index]
    }

    /// Track a single value of a dictionary-valued field. Returns the optional
    /// element (matching `Dictionary`'s own subscript) and invalidates only when
    /// `dict[key]` actually changes.
    func readElement<Key: Hashable, Element>(_ keyPath: WritableKeyPath<Value, [Key: Element]>, at key: Key, name: String) -> Element? {
        let dict = value[keyPath: keyPath]
        let observed: Any = dict[key].map { $0 as Any } ?? NSNull()
        if let scope = readScope {
            let typeId = ObjectIdentifier(type(of: observed))
            let elementKey = ElementKey(keyPath: keyPath as AnyKeyPath,
                                        selector: .dictionary(key as AnyHashable))
            track(scope) { reader in
                if var existing = reader.elements[elementKey], existing.typeId == typeId {
                    existing.observed = observed
                    reader.elements[elementKey] = existing
                } else {
                    reader.elements[elementKey] = ElementRead(name: name,
                                                              keyPath: keyPath as AnyKeyPath,
                                                              observed: observed, typeId: typeId,
                                                              equal: makeEqual(observed),
                                                              selector: .dictionary(key as AnyHashable))
                }
            }
        }
        return dict[key]
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
                for er in reader.elements.values {
                    guard let newContainer = (newValue as Any)[keyPath: er.keyPath] else {
                        reader.invalidate(er.name)
                        invalidated = true
                        break
                    }
                    var newElement: Any?
                    switch er.selector {
                    case .array(let i):
                        guard let array = newContainer as? [Any], array.indices.contains(i) else {
                            // Array shrank (or index vanished) → treat as a change.
                            reader.invalidate(er.name)
                            invalidated = true
                            break
                        }
                        newElement = array[i]
                    case .dictionary(let k):
                        guard let dict = newContainer as? [AnyHashable: Any] else {
                            reader.invalidate(er.name)
                            invalidated = true
                            break
                        }
                        newElement = dict[k]
                    }
                    guard let newElement else {
                        reader.invalidate(er.name)
                        invalidated = true
                        break
                    }
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
