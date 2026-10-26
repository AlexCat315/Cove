public import Foundation

#if canImport(FoundationNetworking)
  internal import FoundationNetworking
#endif

/// Downloads a pinned artifact and verifies its SHA-256 before use. A cached
/// file that already matches the digest is reused without network access.
public enum Download {
  /// Resolves `remote` after redirects using a HEAD request, without a body.
  /// Used to read GitHub's `/releases/latest` pointer without the API rate
  /// limit that applies to api.github.com.
  public static func redirectedURL(from remote: URL) throws -> URL {
    guard remote.scheme == "https" else { throw ToolError.message("Metadata requires HTTPS") }
    var request = URLRequest(url: remote)
    request.httpMethod = "HEAD"
    let semaphore = DispatchSemaphore(value: 0)
    let result = UncheckedBox<Result<URL, Error>>()
    let task = URLSession.shared.dataTask(with: request) { _, response, error in
      defer { semaphore.signal() }
      if let error {
        result.set(.failure(error))
        return
      }
      guard let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode) else {
        result.set(.failure(ToolError.message("HTTP \(response.map { String(describing: $0) } ?? "?") for \(remote.absoluteString)")))
        return
      }
      result.set(.success(http.url ?? remote))
    }
    task.resume()
    semaphore.wait()
    switch result.get() {
    case .success(let url): return url
    case .failure(let error): throw error
    case nil: throw ToolError.message("Redirect did not complete: \(remote.absoluteString)")
    }
  }

  public static func metadata(from remote: URL, to destination: URL) throws {
    guard remote.scheme == "https" else { throw ToolError.message("Metadata requires HTTPS") }
    try Fs.directory(destination.deletingLastPathComponent())
    let downloaded = try fetch(remote, beside: destination)
    defer { try? Fs.remove(downloaded) }
    try Fs.replace(downloaded, at: destination)
  }

  public static func verify(file url: URL, sha256 expected: String) throws {
    let actual = try SHA256.hex(file: url)
    guard actual.caseInsensitiveCompare(expected) == .orderedSame else {
      throw ToolError.message(
        "SHA-256 mismatch for \(url.lastPathComponent): expected \(expected), got \(actual)")
    }
  }

  public static func file(from remote: URL, to destination: URL, sha256: String) throws {
    if FileManager.default.fileExists(atPath: destination.path) {
      if let actual = try? SHA256.hex(file: destination),
        actual.caseInsensitiveCompare(sha256) == .orderedSame
      {
        return
      }
    }
    try Fs.directory(destination.deletingLastPathComponent())
    Terminal.info("Downloading \(remote.absoluteString)")
    let downloaded = try fetch(remote, beside: destination)
    defer { try? Fs.remove(downloaded) }
    try verify(file: downloaded, sha256: sha256)
    try Fs.replace(downloaded, at: destination)
  }

  private static func fetch(_ remote: URL, beside destination: URL) throws -> URL {
    let semaphore = DispatchSemaphore(value: 0)
    let result = UncheckedBox<Result<URL, Error>>()
    let task = URLSession.shared.downloadTask(with: remote) { location, response, error in
      defer { semaphore.signal() }
      if let error {
        result.set(.failure(error))
        return
      }
      if let http = response as? HTTPURLResponse, !(200..<300).contains(http.statusCode) {
        result.set(
          .failure(ToolError.message("HTTP \(http.statusCode) for \(remote.absoluteString)")))
        return
      }
      guard let location else {
        result.set(.failure(ToolError.message("No data for \(remote.absoluteString)")))
        return
      }
      let staging = destination.deletingLastPathComponent().appendingPathComponent(
        ".guava-download-\(UUID().uuidString)")
      do {
        // URLSession's temporary directory may be on a different volume.
        // Copy while its callback owns the file, then publish the sibling staging file.
        try FileManager.default.copyItem(at: location, to: staging)
        result.set(.success(staging))
      } catch {
        try? FileManager.default.removeItem(at: staging)
        result.set(.failure(error))
      }
    }
    task.resume()
    semaphore.wait()
    switch result.get() {
    case .success(let url): return url
    case .failure(let error): throw error
    case nil: throw ToolError.message("Download did not complete: \(remote.absoluteString)")
    }
  }
}
