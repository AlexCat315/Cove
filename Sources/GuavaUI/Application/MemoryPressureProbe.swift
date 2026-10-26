import Foundation
#if os(Windows)
private import WinSDK
#endif

/// Platform pressure is sampled off the UI thread; only transitions enqueue a
/// trim. Linux includes the container limit rather than relying on host RAM alone.
final class MemoryPressureProbe: @unchecked Sendable {
    private let lock = NSLock()
    private var previous = false
    #if os(Windows)
    private var notification = CreateMemoryResourceNotification(LowMemoryResourceNotification)
    #endif

    func changedToLowMemory() -> Bool {
        lock.withLock {
            let low: Bool
            #if os(Windows)
            var condition: WinSDK.BOOL = 0
            low = notification.map { QueryMemoryResourceNotification($0, &condition) != 0 && condition != 0 } ?? false
            #elseif os(Linux)
            let host = Self.availableRatio() < 0.1
            let container = Self.containerRatio() > 0.9
            low = host || container
            #else
            low = false
            #endif
            defer { previous = low }
            return low && !previous
        }
    }

    func close() {
        lock.withLock {
            #if os(Windows)
            if let notification { _ = CloseHandle(notification) }
            notification = nil
            #endif
        }
    }

    #if os(Linux)
    private static func availableRatio() -> Double {
        guard let text = try? String(contentsOfFile: "/proc/meminfo", encoding: .utf8) else { return 1 }
        var total: Double = 0, available: Double = 0
        for line in text.split(separator: "\n") {
            let fields = line.split(whereSeparator: \.isWhitespace)
            guard fields.count >= 2, let value = Double(fields[1]) else { continue }
            if fields[0] == "MemTotal:" { total = value }
            if fields[0] == "MemAvailable:" { available = value }
        }
        return total > 0 ? available / total : 1
    }

    private static func containerRatio() -> Double {
        guard let membership = try? String(contentsOfFile: "/proc/self/cgroup", encoding: .utf8),
              let entry = membership.split(separator: "\n").first(where: { $0.hasPrefix("0::") }) else { return 0 }
        let suffix = String(entry.dropFirst(3))
        for path in ["/sys/fs/cgroup" + suffix, "/sys/fs/cgroup"] {
            guard let value = try? String(contentsOfFile: path + "/memory.current", encoding: .utf8),
                  let maximum = try? String(contentsOfFile: path + "/memory.max", encoding: .utf8),
                  let used = Double(value.trimmingCharacters(in: .whitespacesAndNewlines)),
                  let limit = Double(maximum.trimmingCharacters(in: .whitespacesAndNewlines)), limit > 0 else { continue }
            return used / limit
        }
        return 0
    }
    #endif
}
