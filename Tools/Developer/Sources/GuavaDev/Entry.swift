private import Foundation
private import ToolSupport
private import VulkanBootstrap
#if !os(Windows)
private import DevTools
#endif
@main enum GuavaDev {
 static func main() {
  do {
   var arguments = Array(CommandLine.arguments.dropFirst())
   let command = arguments.isEmpty ? "--help" : arguments.removeFirst()
   switch command {
    case "vulkan": try VulkanBootstrap.run(arguments)
    #if !os(Windows)
    case "devtools": try DevToolsCommands.run(arguments)
    #endif
    case "--help": print("guava-dev vulkan | devtools")
    default: throw ToolError.message("Unknown SDK tool: \(command)")
   }
  } catch { Terminal.fail(error.localizedDescription) }
 }
}
