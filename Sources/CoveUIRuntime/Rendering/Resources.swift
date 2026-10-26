import CovePlatformCore

/// Shared by UI and backdrop shaders in desktop apps and unpacked Android APKs.
enum ShaderResourcesBundle {
    static let bundle = PackageResourceBundle.required(named: "CoveUI_CoveUIRuntime")
}
