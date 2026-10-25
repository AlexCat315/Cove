import Foundation
import Dispatch
import GuavaLogging

/// Machine-readable stdout is collected verbatim. Human output is teed to its original stream and the journal.
final class ProcessStream: @unchecked Sendable {
    private let destination: FileHandle?
    private let collect: Bool
    private let stream: String
    private let command: String
    private let context: Logger.Metadata
    private var collected = Data()
    private var buffered = Data()

    init(command: String, stream: String, destination: FileHandle?, collect: Bool = false) {
        self.command = URL(fileURLWithPath: command).lastPathComponent; self.stream = stream
        self.destination = destination; self.collect = collect; context = LogContext.current
    }
    func consume(_ data: Data, pid: Int32) {
        if collect { collected.append(data) }
        try? destination?.write(contentsOf: data)
        guard !collect else { return }
        LogContext.withValues(context) {
            buffered.append(data)
            while let end = buffered.firstIndex(of: 10) {
                record(buffered[..<end], pid: pid); buffered.removeSubrange(...end)
            }
            if buffered.count > 32768 { record(buffered, pid: pid); buffered.removeAll(keepingCapacity: true) }
        }
    }
    func finish(pid: Int32) -> Data {
        if !buffered.isEmpty { LogContext.withValues(context) { record(buffered, pid: pid) } }
        buffered.removeAll(keepingCapacity: true)
        return collected
    }
    private func record(_ data: some DataProtocol, pid: Int32) {
        let line = String(decoding: data, as: UTF8.self).trimmingCharacters(in: .newlines)
        guard !line.isEmpty else { return }
        LogSystem.logger("com.guava.cli.process").log(level: stream == "stderr" ? .warning : .info,
            "\(line)", metadata: ["command": "\(command)", "child_pid": "\(pid)", "stream": "\(stream)"], source: "process.\(stream)")
    }
}
