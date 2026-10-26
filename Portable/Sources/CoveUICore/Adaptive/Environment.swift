import Foundation

/// Transient surface capabilities. Layout follows available space and input,
/// so a tablet with a keyboard can use the same controls as a desktop window.
public struct UIEnvironment: Sendable, Hashable {
    public struct Viewport: Sendable, Hashable {
        public static let compactMaximumWidth: Float = 600
        public static let expandedMinimumWidth: Float = 1000
        public static let compactMaximumHeight: Float = 600
        public static let expandedMinimumHeight: Float = 1000
        public var width: Float = 1280
        public var height: Float = 720
        public var scale: Float = 1
        public init() {}
        public var widthClass: WidthClass {
            width < Self.compactMaximumWidth ? .compact
                : width < Self.expandedMinimumWidth ? .medium
                : .expanded
        }
        public var heightClass: HeightClass {
            height < Self.compactMaximumHeight ? .compact
                : height < Self.expandedMinimumHeight ? .medium
                : .expanded
        }
        /// Square viewports count as landscape; width wins ties.
        public var orientation: Orientation {
            width >= height ? .landscape : .portrait
        }
    }
    public struct Insets: Sendable, Hashable {
        public var top: Float = 0
        public var bottom: Float = 0
        public var leading: Float = 0
        public var trailing: Float = 0
        public init() {}
        public var normalized: Self {
            var result = self
            result.top = top.isFinite ? max(0, top) : 0
            result.bottom = bottom.isFinite ? max(0, bottom) : 0
            result.leading = leading.isFinite ? max(0, leading) : 0
            result.trailing = trailing.isFinite ? max(0, trailing) : 0
            return result
        }
    }
    /// The device class the surface is being shown on. This is an explicit
    /// host fact, never inferred from width: a landscape phone is still a
    /// phone, and a tablet with a keyboard is still a tablet.
    public enum Idiom: Sendable, Hashable, CaseIterable {
        case phone, tablet, desktop
    }
    /// Software keyboard geometry reported by the platform. Transient session
    /// state, separate from the persistent safe area of the device.
    public struct Keyboard: Sendable, Hashable {
        /// Height in logical points covered by the keyboard and its accessory.
        public var height: Float = 0
        public init(height: Float = 0) { self.height = height.isFinite ? max(0, height) : 0 }
        public var isVisible: Bool { height.isFinite && height > 0 }
    }
    public struct Input: OptionSet, Sendable, Hashable {
        public let rawValue: UInt8
        public init(rawValue: UInt8) { self.rawValue = rawValue }
        public static let touch = Input(rawValue: 1)
        public static let pointer = Input(rawValue: 2)
        public static let keyboard = Input(rawValue: 4)
        public static let pen = Input(rawValue: 8)
    }
    public enum WidthClass: Sendable, Hashable { case compact, medium, expanded }
    public enum HeightClass: Sendable, Hashable { case compact, medium, expanded }
    public enum Orientation: Sendable, Hashable { case portrait, landscape }
    public var viewport = Viewport()
    public var safeArea = Insets()
    public var input: Input = [.pointer, .keyboard]
    public var idiom: Idiom = .desktop
    /// Native accessibility point-size mapping, independent of display density.
    /// Hosts report the curve; pages may override it closer to their content.
    public var textScale = TextScale()
    public var keyboard = Keyboard()
    public var reducesMotion = false
    public init() {}
    public var minimumControlExtent: Float { input.contains(.touch) ? 44 : 28 }
    public var supportsHover: Bool { input.contains(.pointer) }
}
