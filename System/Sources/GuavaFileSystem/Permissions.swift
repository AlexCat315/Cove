public import Foundation
private import CGuavaSystem

public enum FilePermissions {
    public static func createPrivateDirectory(at url: URL, withIntermediateDirectories: Bool = true) throws {
        #if os(Windows)
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: withIntermediateDirectories)
        #else
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: withIntermediateDirectories,
                                                attributes: [.posixPermissions: 0o700])
        #endif
        try restrictToCurrentUser(url)
    }

    /// Replaces inherited access with access for the current user (0700/0600 on POSIX).
    public static func restrictToCurrentUser(_ url: URL) throws {
        let values = try url.resourceValues(forKeys: [.isDirectoryKey, .isSymbolicLinkKey])
        guard values.isSymbolicLink != true else { throw CocoaError(.fileWriteNoPermission) }
        let code = url.path.withCString { guava_restrict_path($0, values.isDirectory == true ? 1 : 0) }
        guard code == 0 else {
            #if os(Windows)
            throw NSError(domain: "NSWin32ErrorDomain", code: Int(code), userInfo: [NSFilePathErrorKey: url.path])
            #else
            throw NSError(domain: NSPOSIXErrorDomain, code: Int(code), userInfo: [NSFilePathErrorKey: url.path])
            #endif
        }
    }
}
