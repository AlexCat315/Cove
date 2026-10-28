import Foundation

/// A platform-neutral snapshot of a menu tree, ready for a native backend to
/// build a system context menu from. Higher-level UI code (the Compose layer)
/// converts its own menu model into this snapshot; the platform shell builds
/// the native menu (NSMenu / Win32 / GTK) and reports back which item was
/// chosen, by its stable id.
public struct NativeMenuSnapshot: Sendable, Equatable {
    public var items: [NativeMenuItemSnapshot]
    public init(items: [NativeMenuItemSnapshot]) { self.items = items }
}

public struct NativeMenuItemSnapshot: Sendable, Equatable {
    public enum Kind: Sendable, Equatable {
        case regular
        case separator
        case submenu(NativeMenuSnapshot)
    }

    /// Stable identifier; the native backend reports this back when chosen.
    public var id: Int
    public var kind: Kind
    public var title: String
    public var isEnabled: Bool
    /// Check / radio state shown beside the label.
    public var isChecked: Bool
    /// Optional keyboard shortcut.
    public var shortcut: NativeShortcut?
    /// Optional leading icon.
    public var icon: NativeMenuIcon?

    public init(id: Int, kind: Kind, title: String, isEnabled: Bool = true,
                isChecked: Bool = false, shortcut: NativeShortcut? = nil,
                icon: NativeMenuIcon? = nil) {
        self.id = id; self.kind = kind; self.title = title
        self.isEnabled = isEnabled; self.isChecked = isChecked
        self.shortcut = shortcut; self.icon = icon
    }
}

/// A platform-neutral keyboard shortcut. Backends map the flags to their own
/// modifier mask (e.g. AppKit maps `meta` to Command).
public struct NativeShortcut: Sendable, Equatable {
    public let key: String
    public let ctrl: Bool
    public let alt: Bool
    public let shift: Bool
    public let meta: Bool
    public init(key: String, ctrl: Bool = false, alt: Bool = false,
                shift: Bool = false, meta: Bool = false) {
        self.key = key; self.ctrl = ctrl; self.alt = alt
        self.shift = shift; self.meta = meta
    }
}

/// A leading icon for a native menu item.
public struct NativeMenuIcon: Sendable, Equatable {
    /// PNG bytes of the image.
    public let png: Data
    /// A template (black + alpha) image that the backend tints to the menu text.
    public let isTemplate: Bool
    public init(png: Data, isTemplate: Bool = false) {
        self.png = png; self.isTemplate = isTemplate
    }
}
