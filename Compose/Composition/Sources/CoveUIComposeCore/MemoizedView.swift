/// Explicit parent-input equality. Include every input that changes content or
/// callbacks in `key`. Local state and environment invalidations still run.
public struct MemoizedView<Key: Equatable, Content: View>: View, _MemoizedView {
    public let key: Key
    private let content: () -> Content

    public init(key: Key, @ViewBuilder content: @escaping () -> Content) {
        self.key = key
        self.content = content
    }

    public var body: Content { content() }

    public func _hasSameInputs(as other: any View) -> Bool {
        guard let other = other as? Self else { return false }
        return key == other.key
    }
}

/// Composition's explicit reuse seam; ordinary views keep conservative input comparison.
public protocol _MemoizedView: View {
    func _hasSameInputs(as other: any View) -> Bool
}
