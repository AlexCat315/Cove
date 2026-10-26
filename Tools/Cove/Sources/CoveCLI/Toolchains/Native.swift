internal import ToolSupport
internal import Foundation

enum NativeSDK {
    struct AppleTarget {
        let sdk: String
        let architecture: String
        var variant: String { "ios-\(architecture)" + (sdk == "iphonesimulator" ? "-simulator" : "") }
    }

    static func prepare(root: URL, apple: AppleTarget? = nil, fromSource: Bool = false) throws {
        let target = apple?.variant ?? CLIUpdate.platform
        if !fromSource, try NativeArtifacts.install(at: root, target: target) { return }
        guard NativeArtifacts.hasSourceInputs(at: root) else {
            throw ToolError.invalid("Missing native libraries for \(target); reinstall a complete SDK release. Building native libraries requires a source checkout.")
        }
        var environment = ProcessInfo.processInfo.environment
        #if os(Windows)
        try WindowsBuildEnvironment.activate(&environment)
        #endif
        for domain in ["Runtime", "Text"] {
            let source = root.appendingPathComponent(domain == "Runtime" ? "Runtime/third-party" : "third-party")
            let base = domain == "Runtime" ? root.appendingPathComponent("Runtime") : root
            let build = base.appendingPathComponent("build/\(apple?.variant ?? "native")")
            let artifacts = NativeArtifacts.directory(at: root, target: target)
            let vendor = artifacts.appendingPathComponent(domain == "Runtime" ? "Runtime/vendor" : "vendor")
            let names = domain == "Runtime" ? ["SDL3", "lunasvg", "plutovg", "webp", "sharpyuv"]
                : ["CFreeType", "CHarfBuzz"] + (apple == nil ? ["GhosttyVt"] : [])
            if names.allSatisfy({ FileManager.default.fileExists(atPath: vendor.appendingPathComponent("\($0).artifactbundle/info.json").path) }) { continue }
            var configure = ["cmake", "-S", source.path, "-B", build.path,
                             "-DCMAKE_BUILD_TYPE=Release", "-DCOVE_VENDOR_DIR=\(vendor.path)"]
            if let apple {
                let sysroot = try Shell.run(["xcrun", "--sdk", apple.sdk, "--show-sdk-path"], capture: true)
                configure += ["-G", "Ninja", "-DCMAKE_SYSTEM_NAME=iOS", "-DCMAKE_OSX_SYSROOT=\(sysroot)",
                              "-DCMAKE_OSX_ARCHITECTURES=\(apple.architecture)", "-DCMAKE_OSX_DEPLOYMENT_TARGET=26.0"]
            }
            try Shell.run(configure, environment: environment)
            try Shell.run(["cmake", "--build", build.path, "--config", "Release", "--parallel", "4"], environment: environment)
            if domain == "Runtime" {
                try Shell.run(["cmake", "--build", build.path, "--config", "Release", "--target", "stage_sdl3", "stage_image_decode", "--parallel", "4"], environment: environment)
            }
            try Shell.run(["cmake", "--install", build.path, "--config", "Release", "--prefix", build.appendingPathComponent("install").path], environment: environment)
        }
    }
}
