public import Foundation

/// The program a `TerminalSession` runs on its pseudo terminal.
public struct TerminalCommand: Sendable, Equatable {
    /// Absolute path, or a name resolved through `PATH`.
    public var executable: String
    public var arguments: [String] = []
    /// Entries layered over the inherited environment. `TERM` and
    /// `COLORTERM` describe the emulator and are set unless overridden here.
    public var environment: [String: String] = [:]
    /// `nil` inherits the host process's working directory.
    public var workingDirectory: URL?

    public init(executable: String, arguments: [String] = [], workingDirectory: URL? = nil) {
        self.executable = executable
        self.arguments = arguments
        self.workingDirectory = workingDirectory
    }

    /// The user's login shell (`$SHELL -l`, falling back to `/bin/sh`); Windows
    /// PowerShell on Windows.
    public static var loginShell: TerminalCommand {
        #if os(Windows)
        return TerminalCommand(executable: "powershell.exe", arguments: ["-NoLogo"])
        #else
        let shell = ProcessInfo.processInfo.environment["SHELL"].flatMap { $0.isEmpty ? nil : $0 } ?? "/bin/sh"
        return TerminalCommand(executable: shell, arguments: ["-l"])
        #endif
    }
}

extension TerminalCommand {
    /// Environment handed to the child: inherited, emulator identity, overrides.
    func resolvedEnvironment(inherited: [String: String] = ProcessInfo.processInfo.environment) -> [String: String] {
        var result = inherited
        result["TERM"] = "xterm-256color"
        result["COLORTERM"] = "truecolor"
        result["TERM_PROGRAM"] = "CoveUI"
        if result["LANG"] == nil, result["LC_ALL"] == nil, result["LC_CTYPE"] == nil {
            result["LANG"] = "en_US.UTF-8"
        }
        result.merge(environment) { _, override in override }
        return result
    }

    /// Absolute executable path; bare names are looked up in `PATH` (and, on
    /// Windows, completed with the `PATHEXT` extensions).
    func resolvedExecutable(path: String?, pathExtensions: String? = ProcessInfo.processInfo.environment["PATHEXT"]) -> String? {
        let manager = FileManager.default
        #if os(Windows)
        let separator: Character = ";", slashes: Set<Character> = ["/", "\\"]
        let extensions = executable.contains(".") ? [""]
            : [""] + (pathExtensions ?? ".COM;.EXE;.BAT;.CMD").split(separator: ";").map(String.init)
        // CreateProcess and cmd re-parse the first token of the command line;
        // forward slashes in the executable path can be mistaken for a switch
        // and corrupt cmd's /c parsing. Report Windows paths with backslashes.
        let windowsPath = { (value: String) -> String in value.replacingOccurrences(of: "/", with: "\\") }
        func runnable(_ candidate: String) -> Bool { manager.fileExists(atPath: candidate) }
        #else
        let separator: Character = ":", slashes: Set<Character> = ["/"]
        let extensions = [""]
        // POSIX paths already use /; leave them untouched.
        let windowsPath = { (value: String) -> String in value }
        func runnable(_ candidate: String) -> Bool { manager.isExecutableFile(atPath: candidate) }
        #endif
        if executable.contains(where: slashes.contains) {
            return runnable(executable) ? windowsPath(executable) : nil
        }
        for directory in (path ?? "/usr/bin:/bin").split(separator: separator) {
            for suffix in extensions {
                let candidate = windowsPath("\(directory)/\(executable)\(suffix)")
                if runnable(candidate) { return candidate }
            }
        }
        return nil
    }
}
