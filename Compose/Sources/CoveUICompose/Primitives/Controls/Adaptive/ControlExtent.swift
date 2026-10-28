import Foundation
import CoveUIScene
import CoveUIText
import CoveUIImaging

extension Node {
    var supportsControlHover: Bool { compositionValue(of: SurfaceEnvironment.key).supportsHover }

    func applyTouchControlExtent(enabled: Bool = true) {
        let environment = compositionValue(of: SurfaceEnvironment.key)
        let extent = CGFloat(environment.minimumControlExtent)
        layoutNode?.setMinimumSizeFloor(enabled && environment.input.contains(.touch)
            ? CGSize(width: extent, height: extent) : nil)
    }
}
