internal import Foundation

struct InstallationReceipt: Codable {
    let version: String
    let directory: URL
    let commands: URL

    static let filename = "installation.json"
    static var executableDirectory: URL {
        (Bundle.main.executableURL ?? URL(fileURLWithPath: CommandLine.arguments[0]))
            .resolvingSymlinksInPath().deletingLastPathComponent()
    }

    static func read(at directory: URL) throws -> Self {
        let handle = try FileHandle(forReadingFrom: directory.appendingPathComponent(filename))
        defer { try? handle.close() }
        return try JSONDecoder().decode(Self.self, from: handle.readToEnd() ?? Data())
    }
}
