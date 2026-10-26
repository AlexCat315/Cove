public import Foundation
private import CoveFileSystem

public enum Fs {
  public static func directory(_ url: URL) throws {
    try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
  }

  public static func remove(_ url: URL) throws {
    if FileManager.default.fileExists(atPath: url.path) {
      try FileManager.default.removeItem(at: url)
    }
  }

  public static func reset(_ url: URL) throws {
    try remove(url)
    try directory(url)
  }

  public static func copy(_ source: URL, to destination: URL) throws {
    try directory(destination.deletingLastPathComponent())
    try remove(destination)
    try FileManager.default.copyItem(at: source, to: destination)
  }

  public static func copyTree(_ source: URL, to destination: URL) throws {
    try directory(destination.deletingLastPathComponent())
    try remove(destination)
    try FileManager.default.copyItem(at: source, to: destination)
  }

  public static func read(_ url: URL) throws -> Data { try Data(contentsOf: url) }
  public static func text(_ url: URL) throws -> String {
    try String(contentsOf: url, encoding: .utf8)
  }

  public static func write(_ text: String, to url: URL) throws {
    try directory(url.deletingLastPathComponent())
    try AtomicFile.write(Data(text.utf8), to: url)
  }

  public static func write(_ data: Data, to url: URL) throws {
    try directory(url.deletingLastPathComponent())
    try AtomicFile.write(data, to: url)
  }

  public static func json<T: Encodable>(_ value: T, to url: URL, pretty: Bool = true) throws {
    let encoder = JSONEncoder()
    encoder.outputFormatting =
      pretty
      ? [.prettyPrinted, .sortedKeys, .withoutEscapingSlashes]
      : [.sortedKeys, .withoutEscapingSlashes]
    var data = try encoder.encode(value)
    data.append(0x0A)
    try write(data, to: url)
  }

  public static func files(in directory: URL, includeDirectories: Bool = false) throws -> [URL] {
    var failure: Error?
    guard
      let enumerator = FileManager.default.enumerator(
        at: directory,
        includingPropertiesForKeys: [.isRegularFileKey, .isDirectoryKey],
        errorHandler: { _, error in
          failure = error
          return false
        })
    else {
      throw ToolError.message("Cannot read \(directory.path)")
    }
    var result: [URL] = []
    for case let entry as URL in enumerator {
      let values = try entry.resourceValues(forKeys: [.isRegularFileKey, .isDirectoryKey])
      if includeDirectories, values.isDirectory == true {
        result.append(entry)
      } else if values.isRegularFile == true {
        result.append(entry)
      }
    }
    if let failure { throw failure }
    return result.sorted { $0.path < $1.path }
  }

  /// Relative path using forward slashes, matching archive and manifest conventions.
  public static func relative(_ file: URL, to root: URL) -> String {
    PathIdentity.relativePath(file, to: root)
  }

  /// Atomically publishes a staged file, matching "write temp then replace" semantics.
  public static func replace(_ staged: URL, at destination: URL) throws {
    try directory(destination.deletingLastPathComponent())
    try FileReplacement.replace(destination, with: staged)
  }
}
