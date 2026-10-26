internal import Foundation

enum Installation {
    static var commands: URL {
        #if os(Windows)
        directory.appendingPathComponent("bin")
        #else
        ProcessInfo.processInfo.environment["COVE_BIN_DIR"].map { URL(fileURLWithPath: $0) }
            ?? FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".local/bin")
        #endif
    }
    static var directory: URL {
        let environment = ProcessInfo.processInfo.environment
        if let path = environment["COVE_HOME"] { return URL(fileURLWithPath: path) }
        #if os(Windows)
        return URL(fileURLWithPath: environment["LOCALAPPDATA"] ?? FileManager.default.homeDirectoryForCurrentUser.path)
            .appendingPathComponent("Cove")
        #elseif os(Linux)
        return environment["XDG_DATA_HOME"].map { URL(fileURLWithPath: $0).appendingPathComponent("cove") }
            ?? FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".local/share/cove")
        #else
        return FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".cove")
        #endif
    }
}
