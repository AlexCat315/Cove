import Foundation
import CovePlatformCore
import CoveUIScene
import CoveUIText
import CoveUIImaging
#if canImport(CoreGraphics)
import CoreGraphics
#endif
#if canImport(ImageIO)
import ImageIO
#endif

/// A menu snapshot plus the actions of its items, so the chosen item's action
/// can be invoked after the native menu returns.
struct NativeMenuBuild {
    let snapshot: NativeMenuSnapshot
    private let actions: [Int: () -> Void]

    init(snapshot: NativeMenuSnapshot, actions: [Int: () -> Void]) {
        self.snapshot = snapshot; self.actions = actions
    }

    func run(id: Int) { actions[id]?() }
}

/// Converts the in-app menu model into a platform-neutral snapshot.
enum NativeMenuBuilder {
    static func build(_ entries: [MenuEntry]) -> NativeMenuBuild {
        var nextID = 0
        var actions: [Int: () -> Void] = [:]

        func snapshot(_ entries: [MenuEntry]) -> NativeMenuSnapshot {
            NativeMenuSnapshot(items: entries.map { entry in
                let id = nextID
                nextID += 1
                switch entry {
                case .separator:
                    return NativeMenuItemSnapshot(id: id, kind: .separator, title: "")

                case .label(_, let title):
                    // A non-selectable group header: a disabled regular item.
                    return NativeMenuItemSnapshot(id: id, kind: .regular,
                                                  title: title, isEnabled: false)

                case .submenu(let branch):
                    let sub = snapshot(branch.entries)
                    return NativeMenuItemSnapshot(id: id, kind: .submenu(sub),
                                                  title: branch.title,
                                                  isEnabled: branch.isEnabled)

                case .item(let item):
                    actions[id] = item.action
                    return NativeMenuItemSnapshot(
                        id: id,
                        kind: .regular,
                        title: item.title,
                        isEnabled: item.isEnabled,
                        isChecked: item.isSelected,
                        shortcut: Self.parseShortcut(item.shortcut),
                        icon: Self.makeIcon(item.content.icon))
                }
            })
        }

        return NativeMenuBuild(snapshot: snapshot(entries), actions: actions)
    }

    /// Parses a platform display string (the value stored in `MenuItem.shortcut`)
    /// into a neutral shortcut. macOS uses the glyphs ⌘⌃⌥⇧; other platforms use
    /// the words Ctrl / Meta / Alt / Shift joined by "+".
    static func parseShortcut(_ display: String?) -> NativeShortcut? {
        guard let display, !display.isEmpty else { return nil }
        var ctrl = false, alt = false, shift = false, meta = false
        var rest = display
        if rest.contains("⌘") { meta = true; rest = rest.replacingOccurrences(of: "⌘", with: "") }
        if rest.contains("⌃") { ctrl = true; rest = rest.replacingOccurrences(of: "⌃", with: "") }
        if rest.contains("⌥") { alt = true; rest = rest.replacingOccurrences(of: "⌥", with: "") }
        if rest.contains("⇧") { shift = true; rest = rest.replacingOccurrences(of: "⇧", with: "") }
        if rest.contains("Ctrl") { ctrl = true; rest = rest.replacingOccurrences(of: "Ctrl", with: "") }
        if rest.contains("Meta") { meta = true; rest = rest.replacingOccurrences(of: "Meta", with: "") }
        if rest.contains("Alt") { alt = true; rest = rest.replacingOccurrences(of: "Alt", with: "") }
        if rest.contains("Shift") { shift = true; rest = rest.replacingOccurrences(of: "Shift", with: "") }
        let key = rest.replacingOccurrences(of: "+", with: "")
            .trimmingCharacters(in: .whitespaces)
        guard !key.isEmpty else { return nil }
        return NativeShortcut(key: key, ctrl: ctrl, alt: alt, shift: shift, meta: meta)
    }

    /// Rasterizes a bundle glyph (an alpha-mask SVG) into a black-on-transparent
    /// template PNG so the native menu can tint it alongside the item text.
    static func makeIcon(_ resource: BundleImageResource?, points: Float = 16) -> NativeMenuIcon? {
        #if canImport(CoreGraphics)
        guard let resource, let url = resource.url else { return nil }
        let pixels = Int(points * 2)
        guard let decoded = try? ImageDecoder.decode(url: url, targetSize: (width: pixels, height: pixels)),
              decoded.width == pixels, decoded.height == pixels else { return nil }
        // The glyph is an alpha mask: force the fill black and keep the coverage.
        var rgba = decoded.pixels
        for i in stride(from: 0, to: rgba.count, by: 4) {
            rgba[i] = 0; rgba[i + 1] = 0; rgba[i + 2] = 0
        }
        guard let png = Self.encodePNG(rgba: rgba, width: pixels, height: pixels) else { return nil }
        return NativeMenuIcon(png: png, isTemplate: true)
        #else
        return nil
        #endif
    }

    #if canImport(CoreGraphics)
    private static func encodePNG(rgba: [UInt8], width: Int, height: Int) -> Data? {
        let bytesPerRow = width * 4
        let colorSpace = CGColorSpaceCreateDeviceRGB()
        let bitmapInfo = CGImageAlphaInfo.premultipliedLast.rawValue
        guard let provider = CGDataProvider(data: Data(rgba) as CFData),
              let cg = CGImage(width: width, height: height,
                               bitsPerComponent: 8, bitsPerPixel: 32,
                               bytesPerRow: bytesPerRow, space: colorSpace,
                               bitmapInfo: CGBitmapInfo(rawValue: bitmapInfo),
                               provider: provider, decode: nil,
                               shouldInterpolate: false, intent: .defaultIntent),
              let output = CFDataCreateMutable(nil, 0) else { return nil }
        guard let destination = CGImageDestinationCreateWithData(output, "public.png" as CFString, 1, nil) else { return nil }
        CGImageDestinationAddImage(destination, cg, nil)
        guard CGImageDestinationFinalize(destination) else { return nil }
        return output as Data
    }
    #endif
}
