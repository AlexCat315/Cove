import CoveUIRuntime

/// Shared control density. Touch chrome resolves its own geometry from this scale. Explicit per-control sizes take precedence;
/// otherwise the nearest `.controlSize` provider sizes a whole toolbar or form.
public enum ControlSize: Sendable, Hashable, CaseIterable {
    case mini, small, regular, large

    public var buttonHeight: Float {
        switch self {
        case .mini: 22
        case .small: 28
        case .regular: 32
        case .large: 40
        }
    }

    public var horizontalPadding: Float {
        switch self {
        case .mini: 6
        case .small: 9
        case .regular: 12
        case .large: 16
        }
    }

    var touchHeight: Float {
        switch self {
        case .mini, .small: 44
        case .regular: 44
        case .large: 56
        }
    }
    var touchPadding: Float {
        switch self {
        case .mini: 12
        case .small: 14
        case .regular: 18
        case .large: 24
        }
    }

    var textFieldSize: TextField.Size {
        switch self {
        case .mini, .small: .small
        case .regular: .regular
        case .large: .large
        }
    }
}

public enum ControlSizeEnvironment {
    public static let key = CompositionLocal<ControlSize>(defaultValue: .regular)
}

public extension View {
    func controlSize(_ size: ControlSize) -> some View {
        compositionLocal(ControlSizeEnvironment.key, size)
    }
}
