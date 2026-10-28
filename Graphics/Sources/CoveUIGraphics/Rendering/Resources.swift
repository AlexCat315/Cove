import CoveUIScene
import CoveUIText
import CoveUIImaging
import CovePlatformCore

/// Shared by UI and backdrop shaders in desktop apps and unpacked Android APKs.
enum ShaderResourcesBundle {
    static let bundle = PackageResourceBundle.required(named: "CoveUIGraphics_CoveUIGraphics")
}
