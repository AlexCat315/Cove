internal import Foundation

enum CLIRuntimeLibraries {
    static func stage(executable: URL, into payload: URL) throws {
        #if os(Windows)
        let environment = ProcessInfo.processInfo.environment
        let searchPath = environment.first { $0.key.caseInsensitiveCompare("PATH") == .orderedSame }?.value ?? ""
        let paths = [executable.deletingLastPathComponent().path] + searchPath.components(separatedBy: ";")
        var copied: Set<String> = []
        let pattern = #"^(swift|BlocksRuntime|dispatch|Foundation|CoreFoundation|_Foundation|icu|libxml2|curl|zlib|vcruntime|msvcp|concrt).*\.dll$"#
        for path in paths where !path.isEmpty {
            guard let files = try? Files.children(URL(fileURLWithPath: path)) else { continue }
            for file in files where file.lastPathComponent.range(of: pattern, options: [.regularExpression, .caseInsensitive]) != nil {
                if copied.insert(file.lastPathComponent.lowercased()).inserted {
                    try Files.copy(file, to: payload.appendingPathComponent(file.lastPathComponent))
                }
            }
        }
        #elseif os(Linux)
        let listing = try Shell.run(["ldd", executable.path], capture: true, quiet: true)
        guard !listing.contains("not found") else { throw ToolError.invalid("CLI has unresolved runtime libraries") }
        let libraries = payload.appendingPathComponent("lib")
        try Files.directory(libraries)
        let system = ["libc.so", "libm.so", "libpthread.so", "libdl.so", "librt.so", "ld-linux"]
        for line in listing.split(whereSeparator: \.isNewline) {
            guard let component = line.split(whereSeparator: \.isWhitespace).first(where: { $0.hasPrefix("/") }) else { continue }
            let file = URL(fileURLWithPath: String(component))
            if system.contains(where: { file.lastPathComponent.hasPrefix($0) }) { continue }
            let destination = libraries.appendingPathComponent(file.lastPathComponent)
            if !FileManager.default.fileExists(atPath: destination.path) {
                try Files.copy(file.resolvingSymlinksInPath(), to: destination)
                try Shell.run(["patchelf", "--set-rpath", "$ORIGIN", destination.path])
            }
        }
        try Shell.run(["patchelf", "--set-rpath", "$ORIGIN/lib", payload.appendingPathComponent(executable.lastPathComponent).path])
        #else
        let listing = try Shell.run(["otool", "-L", executable.path], capture: true, quiet: true)
        for line in listing.split(whereSeparator: \.isNewline).dropFirst() {
            guard let name = line.split(whereSeparator: \.isWhitespace).first else { continue }
            guard name.hasPrefix("/usr/lib/") || name.hasPrefix("/System/Library/") else {
                throw ToolError.invalid("CLI requires an unbundled library: \(name)")
            }
        }
        #endif
    }
}
