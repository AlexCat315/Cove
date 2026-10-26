import Foundation
private import GuavaFileSystem

/// Frontend discovery is independent of application compilation and platform hosting.
enum Inspector {
    static func present(_ session: ManagedSession, root: URL, sessionDirectory: URL, open: Bool) throws {
        let page = root.appendingPathComponent("DevTools/Client/index.html")
        var components = URLComponents(url: page, resolvingAgainstBaseURL: false)!
        components.queryItems = [URLQueryItem(name: "endpoint", value: session.endpoint)]
        let target = components.url!.absoluteString
        // open(1)/xdg-open/rundll32 drop the query string when launching a file://
        // URL, so the endpoint never reaches the client. Land first on a session-local
        // page that forwards from inside the browser, where the query is preserved.
        let bootstrap = sessionDirectory.appendingPathComponent("inspector.html")
        try Files.text(bootstrapHTML(target: target), to: bootstrap)
        try FilePermissions.restrictToCurrentUser(bootstrap)
        print("DevTools session: \(session.runtime.id) (\(session.application.name), \(session.configuration.rawValue))")
        print("Inspector: \(bootstrap.absoluteString)")
        guard open else { return }
        #if os(macOS)
        try Shell.run(["open", bootstrap.absoluteString])
        #elseif os(Windows)
        try Shell.run(["rundll32.exe", "url.dll,FileProtocolHandler", bootstrap.absoluteString])
        #else
        try Shell.run(["xdg-open", bootstrap.absoluteString])
        #endif
    }

    /// The bootstrap embeds the endpoint as a JSON string literal; the token
    /// stays in the 0700 session directory and the file is removed with it.
    private static func bootstrapHTML(target: String) throws -> String {
        let literal = String(data: try JSONEncoder().encode(target), encoding: .utf8)!
        return """
        <!doctype html>
        <html lang="en">
        <head>
          <meta charset="utf-8">
          <title>GuavaUI DevTools</title>
        </head>
        <body>
          <p>Opening GuavaUI DevTools… <a href="\(target)">Open manually</a></p>
          <noscript><p>Enable JavaScript or open the GuavaUI DevTools Inspector directly.</p></noscript>
          <script>location.replace(\(literal));</script>
        </body>
        </html>
        """
    }

    /// Opens the Inspector, or runs one client action (query, click, input,
    /// capture, timings) against the same resolved live session.
    static func attach(_ arguments: Arguments, project: AppProject, root: URL) throws {
        let (session, file) = try live(arguments, project: project)
        guard let client = arguments.client else {
            try present(session, root: root, sessionDirectory: file.deletingLastPathComponent(),
                        open: !arguments.flags.contains(.noOpen))
            return
        }
        try DeveloperTool.run(["devtools", client.operation, "@" + file.path] + client.arguments, root: root)
    }

    private static func live(_ arguments: Arguments, project: AppProject) throws -> (ManagedSession, URL) {
        let directory = project.buildDirectory.appendingPathComponent("devtools")
        guard FileManager.default.fileExists(atPath: directory.path) else { throw ToolError.invalid("No development session; start guava run --devtools or --profile") }
        let requested = arguments.value(.session)
        var sessions: [(ManagedSession, URL)] = []
        for child in try Files.children(directory) {
            let file = child.appendingPathComponent("session.json")
            guard let data = try? Data(contentsOf: file),
                  let session = try? JSONDecoder().decode(ManagedSession.self, from: data),
                  requested == nil || session.runtime.id == requested,
                  arguments.requestedPlatform == nil || session.platform == arguments.requestedPlatform else { continue }
            if (try? isAlive(session)) == true { sessions.append((session, file)) }
        }
        guard sessions.count == 1, let found = sessions.first else {
            throw ToolError.invalid(sessions.isEmpty ? "No live DevTools session for this application" : "Multiple live sessions: \(sessions.map { $0.0.runtime.id }.joined(separator: ", ")); select --session ID")
        }
        return found
    }

    static func isAlive(_ session: ManagedSession) throws -> Bool {
        if let device = session.device {
            let pid = try Shell.run([device.adb, "-s", device.serial, "shell", "pidof", session.application.id], capture: true, quiet: true)
            return pid.split(separator: " ").contains(String(session.runtime.pid)[...])
        }
        #if os(Windows)
        let listing = try Shell.run(["tasklist.exe", "/FI", "PID eq \(session.runtime.pid)", "/NH"], capture: true, quiet: true)
        return listing.split(whereSeparator: \.isWhitespace).contains(String(session.runtime.pid)[...])
        #else
        return processIsAlive(session.runtime.pid)
        #endif
    }
}
