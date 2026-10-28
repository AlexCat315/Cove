/// Process-wide content scale used by vector-image rasterizers (SVG/PDF).
/// Hosts publish the active drawable scale so SVG icons can be rasterized
/// at physical-pixel resolution and stay crisp on HiDPI displays.
public enum ContentScaleHolder {
    nonisolated(unsafe) public static var current: Float = 1
}

