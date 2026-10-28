import Foundation

/// Bumped when any property that decides sequential focus / subtree input
/// eligibility changes. Focus-dependent containers cache their scan against
/// this counter instead of walking the tree on every layout pass.
public enum FocusRevision {
    nonisolated(unsafe) public private(set) static var current: UInt64 = 0
    static func bump() { current &+= 1 }
}

public extension Node {
    /// Programmatic and pointer focus remain available outside sequential Tab traversal.
    var isTabStop: Bool {
        get { attachments["__focus.tabStop"] as? Bool ?? true }
        set {
            guard isTabStop != newValue else { return }
            attachments["__focus.tabStop"] = newValue
            FocusRevision.bump()
        }
    }
}
