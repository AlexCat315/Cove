private import Foundation
private import CoveProcess
#if canImport(Darwin)
private import Darwin
#elseif canImport(Glibc)
private import Glibc
#endif

let mode = CommandLine.arguments.dropFirst().first ?? "output"
switch mode {
case "output":
    try FileHandle.standardOutput.write(contentsOf: Data("stdout".utf8))
    try FileHandle.standardError.write(contentsOf: Data("stderr".utf8))
case "interactive":
    try FileHandle.standardOutput.write(contentsOf: Data("ready".utf8))
    while true {
        let data = try PipeRead.available(from: .standardInput)
        if data.isEmpty { break }
        try FileHandle.standardOutput.write(contentsOf: data)
    }
case "descendant":
    _ = try PipeRead.available(from: .standardInput)
    let child = Process()
    child.executableURL = URL(fileURLWithPath: CommandLine.arguments[0])
    child.arguments = ["sleep"]
    child.standardOutput = FileHandle.standardOutput
    child.standardError = FileHandle.standardError
    try child.run()
    try FileHandle.standardError.write(contentsOf: Data("\(child.processIdentifier)".utf8))
case "sleep":
    #if canImport(Darwin) || canImport(Glibc)
    signal(SIGTERM, SIG_IGN)
    #endif
    Thread.sleep(forTimeInterval: 60)
case "environment":
    try FileHandle.standardOutput.write(contentsOf: Data((ProcessInfo.processInfo.environment["COVE_FIXTURE_VALUE"] ?? "unset").utf8))
case "arguments":
    try FileHandle.standardOutput.write(contentsOf: JSONEncoder().encode(Array(CommandLine.arguments.dropFirst(2))))
default:
    fatalError("Unknown fixture mode")
}
