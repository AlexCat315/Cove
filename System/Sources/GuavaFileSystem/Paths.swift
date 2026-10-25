public import Foundation
private import CGuavaSystem

/// Filesystem comparisons resolve existing ancestors, including paths not created yet.
public enum PathIdentity {
    public static func canonicalURL(_ url: URL) -> URL {
        resolvedURL(url) ?? url.standardizedFileURL
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
        var current = directory.standardizedFileURL
        var visited = Set<String>()
        var result: [URL] = []
        while visited.insert(current.path).inserted {
            result.append(current)
            let parent = current.deletingLastPathComponent().standardizedFileURL
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
        var ancestor = url.standardizedFileURL
        var suffix: [String] = []
        var visitedLinks = Set<String>()
        while !FileManager.default.fileExists(atPath: ancestor.path) {
            if let destination = try? FileManager.default.destinationOfSymbolicLink(atPath: ancestor.path) {
                guard visitedLinks.count < 40, visitedLinks.insert(ancestor.path).inserted else { return nil }
                ancestor = URL(fileURLWithPath: destination, relativeTo: ancestor.deletingLastPathComponent())
                    .absoluteURL.standardizedFileURL
                continue
            }
            let parent = ancestor.deletingLastPathComponent()
            guard parent.path != ancestor.path, !ancestor.lastPathComponent.isEmpty else { break }
            suffix.append(ancestor.lastPathComponent)
            ancestor = parent
        }
        let resolved = ancestor.resolvingSymlinksInPath().standardizedFileURL
        var path = resolved.path
        #if os(Windows)
        path = path.replacingOccurrences(of: "\\", with: "/")
        if path.lowercased().hasPrefix("//?/unc/") { path = "//" + path.dropFirst(8) }
        else if path.hasPrefix("//?/") { path = String(path.dropFirst(4)) }
        #endif
        var result = URL(fileURLWithPath: path, isDirectory: resolved.hasDirectoryPath)
        for component in suffix.reversed() { result.appendPathComponent(component) }
        return result.standardizedFileURL
    }

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
