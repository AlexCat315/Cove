import CoveUIScene
import CoveUIText
import CoveUIImaging

enum TouchPresentation {
    static func usesSheet(_ surface: UIEnvironment) -> Bool {
        surface.idiom == .phone || (surface.input.contains(.touch) && !surface.supportsHover)
    }
    static let menu = CompositionLocal<Bool>(defaultValue: false)
}

/// A trigger's overlay: a bottom sheet on touch surfaces, anchored to the
/// trigger for pointer input.
struct AdaptiveOverlayHost<Content: View>: View {
    let isPresented: Binding<Bool>
    var isEnabled = true
    var width: Float?
    var placement: PopoverPlacement = .bottomLeading
    var sizing: PopoverSizing = .content
    var surface: PopoverSurface = .bubble
    var overflow: PopoverOverflow = .constrain
    var onKey: ((KeyEvent, EventPhase) -> EventResult)?
    let content: Content

    var body: some View {
        SurfaceReader { environment in
            if TouchPresentation.usesSheet(environment) {
                TouchOverlay(isPresented: isPresented, content: content, onKey: onKey)
            } else {
                AnchoredOverlayHost(isPresented: isEnabled && isPresented.wrappedValue, width: width,
                                    placement: placement, sizing: sizing, surface: surface, overflow: overflow,
                                    onDismiss: { isPresented.wrappedValue = false }, keyHandler: onKey) {
                    content
                }
            }
        }
    }
}

struct TouchOverlay<Content: View>: View {
    let isPresented: Binding<Bool>
    let content: Content
    var onKey: ((KeyEvent, EventPhase) -> EventResult)?

    var body: some View {
        SurfaceReader { surface in
            let safe = surface.safeArea.normalized
            let bottom = max(safe.bottom, max(0, surface.keyboard.height))
            let available = max(0, surface.viewport.height - safe.top - bottom - 12)
            ResolvedLayoutDirection { direction in
                Modal(isPresented: isPresented, configure: {
                    $0.geometry.placement = .bottom; $0.geometry.inset = 0
                    $0.geometry.width = surface.viewport.widthClass == .compact ? .greatestFiniteMagnitude : 560
                    $0.geometry.height = max(64, available + bottom)
                    $0.geometry.cornerRadius = 18
                    $0.fitsContentHeight = true; $0.backdrop = .dimmed
                }) {
                    _MenuKeyHost(onKey: { event in onKey?(event, .target) == .handled }) {
                        Box(direction: .column, alignItems: .stretch, spacing: 0) {
                            SheetDragHandle(onTranslation: { _ in }, onEnd: { distance, velocity in
                                if distance > 80 || (distance > 0 && velocity > 900) { isPresented.wrappedValue = false }
                            }).accessibilityHidden()
                            ScrollView(.vertical, scrollbarGutter: .overlay) {
                                content.frame(width: .percent(100), minWidth: 0)
                                    .compositionLocal(TouchPresentation.menu, true)
                            }.frame(maxHeight: max(0, available - 56))
                        }
                        .padding(EdgeInsets(leading: max(12, safe.leading), bottom: bottom + 12, trailing: max(12, safe.trailing)))
                        .layoutDirection(direction == .rtl ? .rtl : .ltr)
                        .debugName("touch-overlay-sheet")
                    }
                }
            }
        }
    }
}
