private import Foundation

/// Limits apply independently to shaping and layout. Bytes estimate retained
/// strings and result buffers, rather than the process's physical footprint.
public struct TextCacheLimits: Sendable, Equatable {
    public var maximumEntries: Int
    public var maximumBytes: Int

    public init(maximumEntries: Int = 512, maximumBytes: Int = 8 * 1_024 * 1_024) {
        self.maximumEntries = max(0, maximumEntries)
        self.maximumBytes = max(0, maximumBytes)
    }
}

public struct TextCacheSnapshot: Sendable, Equatable {
    public let entries: Int
    public let estimatedBytes: Int
}

/// A bounded LRU. Expensive shaping/layout happens outside the lock; a trim
/// also discards builds that started before it, so pressure cannot refill it.
final class TextCache<Key: Hashable, Value> {
    private struct Entry {
        let value: Value
        let bytes: Int
        var use: UInt64
    }
    private let lock = NSLock()
    private let limits: TextCacheLimits
    private var entries: [Key: Entry] = [:]
    private var bytes = 0
    private var clock: UInt64 = 0
    private var generation: UInt64 = 0

    init(limits: TextCacheLimits) { self.limits = limits }

    var snapshot: TextCacheSnapshot {
        lock.withLock { TextCacheSnapshot(entries: entries.count, estimatedBytes: bytes) }
    }

    func clear() {
        lock.withLock {
            generation &+= 1
            entries.removeAll()
            bytes = 0
        }
    }

    func value(for key: Key, cost: (Value) -> Int, build: () -> Value) -> Value {
        let lookup: (Value?, UInt64) = lock.withLock {
            if var entry = entries[key] {
                clock &+= 1
                entry.use = clock
                entries[key] = entry
                return (entry.value, generation)
            }
            return (nil, generation)
        }
        if let cached = lookup.0 { return cached }
        let result = build()
        let resultBytes = max(0, cost(result))
        return lock.withLock {
            if let cached = entries[key] { return cached.value }
            guard generation == lookup.1, limits.maximumEntries > 0,
                  resultBytes <= limits.maximumBytes else { return result }
            while entries.count >= limits.maximumEntries || bytes > limits.maximumBytes - resultBytes {
                guard let oldest = entries.min(by: { $0.value.use < $1.value.use }) else { break }
                bytes -= oldest.value.bytes
                entries.removeValue(forKey: oldest.key)
            }
            clock &+= 1
            entries[key] = Entry(value: result, bytes: resultBytes, use: clock)
            bytes += resultBytes
            return result
        }
    }
}
