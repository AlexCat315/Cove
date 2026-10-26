private import Foundation
private import Synchronization
private import Testing
private import GuavaFileSystem

@Test func independentFileHandlesSerializeSharedWrites() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    let value = root.appendingPathComponent("counter")
    try "0".write(to: value, atomically: true, encoding: .utf8)
    let failures = Mutex<[String]>([])
    DispatchQueue.concurrentPerform(iterations: 20) { _ in
        do {
            try FileLock.withLock(at: root.appendingPathComponent("shared.lock")) {
                let count = Int(try String(contentsOf: value, encoding: .utf8))!
                Thread.sleep(forTimeInterval: 0.001)
                try String(count + 1).write(to: value, atomically: true, encoding: .utf8)
            }
        } catch { failures.withLock { $0.append(String(describing: error)) } }
    }
    #expect(failures.withLock { $0.isEmpty })
    #expect(try String(contentsOf: value, encoding: .utf8) == "20")
}
