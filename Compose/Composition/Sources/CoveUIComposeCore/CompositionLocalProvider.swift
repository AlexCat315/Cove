import CoveUIScene

/// Marker for `ViewModifier`s whose effect must be applied directly to the
/// node produced by the wrapped content (the wrapper node), instead of being
/// pushed down into descendant layout/render targets via `modifierTargets`.
///
/// The canonical use is `CompositionLocal` provision: the value must live on
/// an ancestor of every consumer, so writing it into the wrapper node is
/// correct regardless of whether that node has a layout node or is a
/// transparent user-view anchor.
///
/// Modifiers conforming to this protocol are not consulted via
/// `apply(node:)` / `apply(layout:)` by the materialiser; they are routed to
/// `_applyScope(node:)` exclusively.
public protocol _ScopeApplyingModifier {
    func _applyScope(node: Node) -> Bool
}

public struct _ProvideCompositionLocalModifier<Value>: ViewModifier, _ScopeApplyingModifier {
    public typealias Body = _ViewModifier_Content<Self>

    let local: CompositionLocal<Value>
    let value: Value

    public init(local: CompositionLocal<Value>, value: Value) {
        self.local = local
        self.value = value
    }

    public func _applyScope(node: Node) -> Bool {
        if let previous = node.compositionValues[local.key],
           ViewInputComparison.equal(previous, value, environment: true) { return false }
        node.setCompositionValue(local, value)
        return true
    }
}

public extension View {
    /// Provide `value` for `local` to every descendant of this view.
    /// Nearer providers override farther ones; consumers fall back to
    /// `local.defaultValue` when no provider is reachable.
    func compositionLocal<Value>(_ local: CompositionLocal<Value>,
                                 _ value: Value) -> some View {
        modifier(_ProvideCompositionLocalModifier(local: local, value: value))
    }
}
