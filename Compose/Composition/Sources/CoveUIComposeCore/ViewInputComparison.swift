import CoveUIScene
import Foundation

/// State belongs to a retained scope. Equal parent inputs must keep that scope's
/// original storage and callbacks rather than replace storage without replaying body.
private protocol PersistentViewState {}
extension State: PersistentViewState {}

enum ViewInputComparison {
    private static let scalarTypes = Set([Bool.self, Int.self, Int8.self, Int16.self, Int32.self, Int64.self,
        UInt.self, UInt8.self, UInt16.self, UInt32.self, UInt64.self, Float.self, Double.self,
        String.self, Character.self, UUID.self, Date.self, CGFloat.self].map(ObjectIdentifier.init))
    static func equal(_ lhs: Any, _ rhs: Any, environment: Bool = false) -> Bool {
        if !environment, let lhs = lhs as? any _MemoizedView, let rhs = rhs as? any View {
            return lhs._hasSameInputs(as: rhs)
        }
        var remaining = 2_048
        if !environment {
            guard type(of: lhs) == type(of: rhs) else { return false }
            let a = Array(Mirror(reflecting: lhs).children)
            let b = Array(Mirror(reflecting: rhs).children)
            // State-only hosts can reconcile external registries as part of a
            // parent update. Only skip a scope with actual comparable inputs.
            guard a.count == b.count, a.contains(where: { !($0.value is any PersistentViewState) }) else { return false }
            return zip(a, b).allSatisfy { left, right in
                left.label == right.label && (left.value is any PersistentViewState
                    || compare(left.value, right.value, environment: false, remaining: &remaining))
            }
        }
        return compare(lhs, rhs, environment: environment, remaining: &remaining)
    }

    private static func compare(_ lhs: Any, _ rhs: Any, environment: Bool, remaining: inout Int) -> Bool {
        remaining -= 1
        guard remaining >= 0, type(of: lhs) == type(of: rhs) else { return false }
        // Nested state belongs to a child scope, not this scope's input list.
        if !environment, lhs is any PersistentViewState { return false }
        if scalarTypes.contains(ObjectIdentifier(type(of: lhs))), let value = lhs as? any Equatable {
            return equate(value, rhs)
        }
        let a = Mirror(reflecting: lhs), b = Mirror(reflecting: rhs)
        if a.displayStyle == .class {
            // An environment binding retains its object identity; mutable view
            // reference inputs remain opaque and must reevaluate on parent updates.
            return environment && (lhs as AnyObject) === (rhs as AnyObject)
        }
        guard let style = a.displayStyle, style == b.displayStyle else { return false }
        if lhs is any CustomReflectable {
            // Reflection customized by application code cannot prove equality.
            let name = String(reflecting: type(of: lhs))
            guard ["Swift.Array<", "Swift.Dictionary<", "Swift.Set<", "Swift.SIMD"].contains(where: name.hasPrefix) else { return false }
        }
        let ac = Array(a.children), bc = Array(b.children)
        guard ac.count == bc.count else { return false }
        if ac.isEmpty {
            if let value = lhs as? any Equatable { return equate(value, rhs) }
            return style == .struct || style == .tuple || style == .optional || style == .collection
                || style == .dictionary || style == .set
        }
        for (left, right) in zip(ac, bc) {
            guard left.label == right.label,
                  compare(left.value, right.value, environment: environment, remaining: &remaining) else { return false }
        }
        return true
    }

    private static func equate<T: Equatable>(_ lhs: T, _ rhs: Any) -> Bool {
        guard let rhs = rhs as? T else { return false }
        return lhs == rhs
    }
}
