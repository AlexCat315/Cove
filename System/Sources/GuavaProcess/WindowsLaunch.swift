#if os(Windows)
internal import Foundation

struct WindowsLaunch {
    let executable: String
    let command: String
    let environment: String

    init(_ request: ProcessCommand) throws {
        var values: [String: String] = [:]
        for (key, value) in request.environment { values[key.uppercased()] = value }
        environment = values.sorted { $0.key < $1.key }.map { $0.key + "=" + $0.value }.joined(separator: "\0") + "\0\0"
        let path = request.executable.path
        if ["bat", "cmd"].contains(request.executable.pathExtension.lowercased()) {
            // Batch files have shell syntax, unlike native argv. Reject syntax-changing input.
            let unsafe = CharacterSet(charactersIn: "\"\r\n%!?^&|<>")
            guard ([path] + request.arguments).allSatisfy({ $0.rangeOfCharacter(from: unsafe) == nil }) else {
                throw ProcessError.invalidCommand
            }
            executable = ProcessEnvironment.value("ComSpec", in: request.environment)
                ?? (ProcessEnvironment.value("SystemRoot", in: request.environment) ?? "C:\\Windows") + "\\System32\\cmd.exe"
            command = Self.quote(executable) + " /d /s /v:off /c \"" + ([path] + request.arguments).map { "\"" + $0 + "\"" }.joined(separator: " ") + "\""
        } else {
            executable = path
            command = ([path] + request.arguments).map(Self.quote).joined(separator: " ")
        }
    }

    /// Microsoft CRT argv quoting; native arguments never pass through a shell.
    private static func quote(_ argument: String) -> String {
        var result = "\"", slashes = 0
        for character in argument.unicodeScalars {
            if character == "\\" { slashes += 1; continue }
            if character == "\"" {
                result += String(repeating: "\\", count: slashes * 2 + 1) + "\""
            } else {
                result += String(repeating: "\\", count: slashes) + String(character)
            }
            slashes = 0
        }
        return result + String(repeating: "\\", count: slashes * 2) + "\""
    }
}
#endif
