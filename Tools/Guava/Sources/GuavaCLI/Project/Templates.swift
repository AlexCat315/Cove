import Foundation

enum ApplicationTemplates {
    static var root: URL { Bundle.module.resourceURL!.appendingPathComponent("Templates/UI") }

    static func installAndroid(at project: URL, configuration: ProjectConfiguration) throws {
        // The template is the Gradle shell only. Java, JNI and the Swift
        // package stay in sdk/Hosts/Android and are referenced by settings.gradle.
        let source = root.appendingPathComponent("Platforms/Android")
        try Files.copy(source, to: project.appendingPathComponent("Platforms/Android"))
        try Files.json(configuration, to: project.appendingPathComponent("guava.json"))
        try Files.text("// Generated packaging workspace\n", to: project.appendingPathComponent("Package.swift"))
    }
}
