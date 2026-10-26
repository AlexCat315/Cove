public import Foundation
private import CoveFileSystem

/// Locates the SDK checkout every tool operates on, independent of the caller's
/// working directory, and exposes the small set of well-known roots.
public enum Repository {
  public static var root: URL {
    if let override = ProcessInfo.processInfo.environment["COVE_ROOT"], !override.isEmpty {
      return URL(fileURLWithPath: override).standardizedFileURL
    }
    let current = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
    for directory in PathIdentity.ancestors(of: current) {
      if FileManager.default.fileExists(atPath: directory.appendingPathComponent("Tools/Developer/Package.swift").path) {
        return directory
      }
    }
    Terminal.fail("Run cove-dev from inside a source checkout or set COVE_ROOT")
  }

  public static func path(_ relative: String) -> URL {
    root.appendingPathComponent(relative)
  }

  public static func require(_ relative: String) throws -> URL {
    let url = path(relative)
    guard FileManager.default.fileExists(atPath: url.path) else {
      throw ToolError.message("Missing required path: \(relative)")
    }
    return url
  }
}
