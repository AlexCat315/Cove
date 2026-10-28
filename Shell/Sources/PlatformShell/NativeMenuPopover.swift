import CovePlatformCore
import Foundation

#if os(macOS)
import AppKit

/// Builds and pops up a native (AppKit) context menu from a platform-neutral
/// snapshot. The popup runs a modal session and returns the id of the chosen
/// item, or `nil` if the menu was dismissed without a choice.
@MainActor
public enum NativeMenuPopover {
    /// - Parameters:
    ///   - snapshot: the menu tree to show.
    ///   - point: anchor point in window logical coordinates (DIPs), top-left
    ///     origin.
    ///   - window: the window the point belongs to; the key window is used
    ///     when `nil`.
    /// - Returns: the id of the chosen item, or `nil`.
    public static func popUp(_ snapshot: NativeMenuSnapshot,
                             at point: CGPoint,
                             window: NSWindow? = nil) -> Int? {
        let target = MenuTarget()
        let menu = NSMenu()
        menu.autoenablesItems = false
        for item in snapshot.items {
            menu.addItem(build(item, target: target))
        }

        guard let contentView = (window ?? NSApp.keyWindow)?.contentView else {
            // No anchor view: pop up at the current mouse location on screen.
            menu.popUp(positioning: nil, at: NSEvent.mouseLocation, in: nil)
            return target.chosenID
        }

        // The point arrives top-left oriented (DIPs). A non-flipped AppKit
        // view is bottom-left oriented, so flip the y.
        var location = point
        if !contentView.isFlipped {
            location.y = contentView.bounds.height - point.y
        }
        menu.popUp(positioning: nil, at: location, in: contentView)
        return target.chosenID
    }

    private static func build(_ item: NativeMenuItemSnapshot,
                              target: MenuTarget) -> NSMenuItem {
        switch item.kind {
        case .separator:
            return NSMenuItem.separator()

        case .submenu(let branch):
            let menuItem = NSMenuItem(title: item.title, action: nil, keyEquivalent: "")
            let submenu = NSMenu()
            submenu.autoenablesItems = false
            for child in branch.items {
                submenu.addItem(build(child, target: target))
            }
            menuItem.submenu = submenu
            menuItem.isEnabled = item.isEnabled
            return menuItem

        case .regular:
            let key = item.shortcut?.key ?? ""
            let menuItem = NSMenuItem(title: item.title,
                                      action: #selector(MenuTarget.choose(_:)),
                                      keyEquivalent: key)
            menuItem.target = target
            menuItem.tag = item.id
            menuItem.isEnabled = item.isEnabled
            menuItem.state = item.isChecked ? .on : .off
            if let shortcut = item.shortcut {
                menuItem.keyEquivalentModifierMask = modifierMask(shortcut)
            }
            if let icon = item.icon, let image = NSImage(data: icon.png) {
                image.size = NSSize(width: 16, height: 16)
                image.isTemplate = icon.isTemplate
                menuItem.image = image
            }
            return menuItem
        }
    }

    private static func modifierMask(_ shortcut: NativeShortcut) -> NSEvent.ModifierFlags {
        var flags: NSEvent.ModifierFlags = []
        if shortcut.meta { flags.insert(.command) }
        if shortcut.ctrl { flags.insert(.control) }
        if shortcut.alt { flags.insert(.option) }
        if shortcut.shift { flags.insert(.shift) }
        return flags
    }
}

/// The target of every regular item; records the chosen item's tag.
private final class MenuTarget: NSObject {
    var chosenID: Int?
    @objc func choose(_ sender: NSMenuItem) {
        chosenID = sender.tag
    }
}
#endif
