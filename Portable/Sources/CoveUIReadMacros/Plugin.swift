internal import SwiftCompilerPlugin
internal import SwiftSyntaxMacros

@main
struct CoveUIReadPlugin: CompilerPlugin {
    let providingMacros: [Macro.Type] = [TrackStateReadsMacro.self]
}
