public import Foundation
private import CGuavaSystem

/// Filesystem comparisons resolve existing ancestors, including paths not created yet.
public enum PathIdentity {
    public static func canonicalURL(_ url: URL) -> URL {
        resolvedURL(url) ?? normalizedURL(url)
    }

    public static func sameLocation(_ lhs: URL, _ rhs: URL) -> Bool {
        guard let lhs = resolvedURL(lhs), let rhs = resolvedURL(rhs) else { return false }
        let leftExists = FileManager.default.fileExists(atPath: lhs.path)
        let rightExists = FileManager.default.fileExists(atPath: rhs.path)
        guard leftExists == rightExists else { return false }
        if leftExists {
            return lhs.path.withCString { left in rhs.path.withCString { guava_same_file(left, $0) == 1 } }
        }
        return comparableComponents(lhs) == comparableComponents(rhs)
    }

    public static func contains(_ candidate: URL, in root: URL, includingRoot: Bool = true) -> Bool {
        guard let candidate = resolvedURL(candidate), let root = resolvedURL(root) else { return false }
        if FileManager.default.fileExists(atPath: root.path) {
            let directories = ancestors(of: candidate)
            return directories.dropFirst(includingRoot ? 0 : 1).contains { sameLocation($0, root) }
        }
        let parts = comparableComponents(candidate), base = comparableComponents(root)
        return parts.count >= base.count && (includingRoot || parts.count > base.count)
            && parts.prefix(base.count).elementsEqual(base)
    }

    /// Includes the starting directory and terminates at POSIX, drive and UNC roots.
    public static func ancestors(of directory: URL) -> [URL] {
        var current = normalizedURL(directory)
        var visited = Set<String>()
        var result: [URL] = []
        while visited.insert(current.path).inserted {
            result.append(current)
            #if os(Windows)
            guard !isWindowsRoot(current.path) else { break }
            #endif
            let parent = normalizedURL(current.deletingLastPathComponent())
            guard !parent.path.isEmpty, parent.path != current.path else { break }
            current = parent
        }
        return result
    }

    public static func relativePath(_ file: URL, to root: URL) -> String {
        let parts = components(file.standardizedFileURL), base = components(root.standardizedFileURL)
        let comparable = comparableComponents(file.standardizedFileURL)
        let comparableBase = comparableComponents(root.standardizedFileURL)
        var common = 0
        while common < min(parts.count, base.count), comparable[common] == comparableBase[common] { common += 1 }
        return (Array(repeating: "..", count: base.count - common) + Array(parts.dropFirst(common))).joined(separator: "/")
    }

    private static func resolvedURL(_ url: URL) -> URL? {
        guard url.isFileURL else { return nil }
        var ancestor = normalizedURL(url)
        var suffix: [String] = []
        var visitedLinks = Set<String>()
        while !FileManager.default.fileExists(atPath: ancestor.path) {
            if let destination = try? FileManager.default.destinationOfSymbolicLink(atPath: ancestor.path) {
                guard visitedLinks.count < 40, visitedLinks.insert(ancestor.path).inserted else { return nil }
                ancestor = normalizedURL(URL(fileURLWithPath: destination, relativeTo: ancestor.deletingLastPathComponent()).absoluteURL)
                continue
            }
            #if os(Windows)
            guard !isWindowsRoot(ancestor.path) else { break }
            #endif
            let parent = normalizedURL(ancestor.deletingLastPathComponent())
            guard parent.path != ancestor.path, !ancestor.lastPathComponent.isEmpty else { break }
            suffix.append(ancestor.lastPathComponent)
            ancestor = parent
        }
        var result = normalizedURL(ancestor.resolvingSymlinksInPath())
        for component in suffix.reversed() { result.appendPathComponent(component) }
        return normalizedURL(result)
    }

    private static func normalizedURL(_ url: URL) -> URL {
        #if os(Windows)
        // Foundation standardization can collapse //?/ before it reaches the filesystem,
        // and deleting a drive root can turn C:/ into a relative C:. Normalize first.
        var path = url.path.replacingOccurrences(of: "\\", with: "/")
        if let host = url.host, !host.isEmpty, host != "localhost", !path.hasPrefix("//") {
            path = "//" + host + "/" + path.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
        }
        for prefix in ["//?/", "/?/"] where path.hasPrefix(prefix) {
            path = String(path.dropFirst(prefix.count))
            if path.lowercased().hasPrefix("unc/") { path = "//" + path.dropFirst(4) }
            break
        }
        if isWindowsRoot(path) {
            if !path.hasSuffix("/") { path += "/" }
            return URL(fileURLWithPath: path, isDirectory: true)
        }
        return URL(fileURLWithPath: path, isDirectory: url.hasDirectoryPath).standardizedFileURL
        #else
        return url.standardizedFileURL
        #endif
    }

    #if os(Windows)
    private static func isWindowsRoot(_ path: String) -> Bool {
        let parts = path.replacingOccurrences(of: "\\", with: "/").split(separator: "/")
        if path.hasPrefix("//") { return parts.count <= 2 }
        return parts.count == 1 && parts[0].count == 2 && parts[0].last == ":"
    }
    #endif

    public static func components(_ url: URL) -> [String] {
        #if os(Windows)
        canonicalURL(url).path.replacingOccurrences(of: "\\", with: "/").split(separator: "/").map(String.init)
        #else
        canonicalURL(url).path.split(separator: "/").map(String.init)
        #endif
    }

    public static func comparableComponents(_ url: URL) -> [String] {
        #if os(Windows)
        components(url).map { $0.lowercased() }
        #else
        components(url)
        #endif
    }
}
