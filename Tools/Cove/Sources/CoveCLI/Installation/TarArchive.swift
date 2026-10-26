internal import Foundation
private import ToolSupport

enum TarArchive {
    static var executable: String {
        #if os(Windows)
        URL(fileURLWithPath: ProcessInfo.processInfo.environment["SystemRoot"] ?? "C:\\Windows")
            .appendingPathComponent("System32/tar.exe").path
        #else
        "tar"
        #endif
    }

    static func extract(_ archive: URL, to directory: URL) throws {
        try Files.directory(directory)
        let entries = try Shell.run([executable, "-tf", archive.path], capture: true, quiet: true)
        guard entries.split(whereSeparator: \.isNewline).allSatisfy({ entry in
            !entry.hasPrefix("/") && !entry.contains("\\") && !entry.contains(":")
                && !entry.split(separator: "/").contains("..")
        }) else { throw ToolError.invalid("Archive contains an invalid path") }
        try Shell.run([executable, "-xf", archive.path, "-C", directory.path])
    }
}
