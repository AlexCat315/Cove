internal import Foundation
internal import GuavaAndroidContract

enum AndroidArtifact {
    static func export(_ arguments: Arguments, root: URL) throws {
        let toolchain = try AndroidToolchain(root: root, arguments: arguments)
        let options = try BuildOptions(arguments)
        let identity = ApplicationIdentity(id: try arguments.required(.id), name: try arguments.required(.name))
        try identity.validate()
        let output = URL(fileURLWithPath: try arguments.required(.output))
        let products = try toolchain.compile(product: "GuavaAndroidApp", configuration: options.configuration,
                                             gameplay: arguments.url(.gameplayPackage))
        try APKPackage(toolchain: toolchain, identity: identity, mode: options.configuration, signing: options.signing)
            .build(products: products, launch: SceneLaunch(mode: .game, project: "GuavaProject"), output: output,
                   project: arguments.url(.projectAssets))
    }
}
