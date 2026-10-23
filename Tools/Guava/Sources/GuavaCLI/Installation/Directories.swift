internal import Foundation

enum Installation {
    static var directory: URL {
        let environment = ProcessInfo.processInfo.environment
        if let path = environment["GUAVA_HOME"] { return URL(fileURLWithPath: path) }
        #if os(Windows)
        return URL(fileURLWithPath: environment["LOCALAPPDATA"] ?? FileManager.default.homeDirectoryForCurrentUser.path)
            .appendingPathComponent("Guava")
        #elseif os(Linux)
        return environment["XDG_DATA_HOME"].map { URL(fileURLWithPath: $0).appendingPathComponent("guava") }
            ?? FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".local/share/guava")
        #else
        return FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".guava")
        #endif
    }
}
