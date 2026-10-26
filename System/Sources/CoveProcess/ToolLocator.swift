public import Foundation

public enum ProcessEnvironment {
    public static func setting(_ value: String, for name: String, in environment: [String: String]) -> [String: String] {
        var result = environment
        #if os(Windows)
        for key in result.keys where key.caseInsensitiveCompare(name) == .orderedSame { result.removeValue(forKey: key) }
        #endif
        result[name] = value
        return result
    }

    public static func value(_ name: String, in environment: [String: String]) -> String? {
        if let value = environment[name] { return value }
        #if os(Windows)
        return environment.first { $0.key.caseInsensitiveCompare(name) == .orderedSame }?.value
        #else
        return nil
        #endif
    }
}

public enum ToolLocator {
    public static func executable(_ name: String,
                                  environment: [String: String] = ProcessInfo.processInfo.environment) -> URL? {
        guard !name.isEmpty, !name.contains("\0") else { return nil }
        if name.contains("/") || name.contains("\\") {
            let url = URL(fileURLWithPath: name)
            return isRunnable(url) ? url : nil
        }
        #if os(Windows)
        let separator: Character = ";"
        let extensions = [""] + (ProcessEnvironment.value("PATHEXT", in: environment) ?? ".EXE;.CMD;.BAT")
            .split(separator: ";").map(String.init)
        #else
        let separator: Character = ":"
        let extensions = [""]
        #endif
        for directory in (ProcessEnvironment.value("PATH", in: environment) ?? "").split(separator: separator) {
            for suffix in extensions {
                let file = URL(fileURLWithPath: String(directory)).appendingPathComponent(name + suffix)
                if isRunnable(file) { return file }
            }
        }
        return nil
    }

    private static func isRunnable(_ url: URL) -> Bool {
        var directory: ObjCBool = false
        return FileManager.default.fileExists(atPath: url.path, isDirectory: &directory)
            && !directory.boolValue && FileManager.default.isExecutableFile(atPath: url.path)
    }
}
