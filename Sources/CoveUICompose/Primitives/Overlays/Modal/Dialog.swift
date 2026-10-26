import CoveUIRuntime

public struct DialogOptions {
    public var modal = ModalOptions()
    public var description = ""
    public var showsCloseButton = true
    /// Touch phones present a centered dialog as a bottom sheet.
    var adaptsToTouch = true
    public init() {}
}

/// Structured modal header, scrollable content and a separate action area.
/// On touch phones it rises from the bottom edge and is dragged away by its
/// grabber.
public struct Dialog<Content: View, Footer: View>: View {
    public let isPresented: Binding<Bool>
    public let title: String
    public var options = DialogOptions()
    private let content: Content
    private let footer: Footer
    public init(isPresented: Binding<Bool>, title: String, configure: (inout DialogOptions) -> Void = { _ in },
                @ViewBuilder content: () -> Content, @ViewBuilder footer: () -> Footer) {
        self.isPresented = isPresented; self.title = title; self.content = content(); self.footer = footer(); configure(&options)
    }
    public var body: some View {
        SurfaceReader { surface in
            let sheet = options.adaptsToTouch && options.modal.geometry.placement == .center
                && TouchPresentation.usesSheet(surface)
            let fits = sheet || options.modal.fitsContentHeight
            let safe = surface.safeArea.normalized
            let bottom = sheet ? max(safe.bottom, max(0, surface.keyboard.height)) : 0
            Modal(isPresented: isPresented, configure: {
                $0 = options.modal
                guard sheet else { return }
                $0.geometry.placement = .bottom; $0.geometry.inset = 0; $0.geometry.cornerRadius = 18
                $0.geometry.width = surface.viewport.widthClass == .compact ? .greatestFiniteMagnitude : 560
                $0.geometry.height = max(64, surface.viewport.height - safe.top - 12)
                $0.fitsContentHeight = true
            }) {
                Box(direction: .column, alignItems: .stretch, spacing: 0) {
                    if sheet {
                        SheetDragHandle(onTranslation: { _ in }, onEnd: { distance, velocity in
                            guard options.modal.dismissal.closesOnBackdrop else { return }
                            if distance > 80 || (distance > 0 && velocity > 900) { isPresented.wrappedValue = false }
                        }).accessibilityHidden()
                    }
                    Row(alignment: .center, spacing: 12) {
                        Column(alignment: .leading, spacing: 5) {
                            Text(title).font(.headline).frame(minWidth: 0)
                            if !options.description.isEmpty { Text(options.description).font(.caption).foregroundColor(.onSurfaceMuted) }
                        }.flex(1, shrink: 1)
                        if options.showsCloseButton && !sheet {
                            Button(icon: .resource(UIIcons.Action.x), size: 14, tooltip: "Close dialog") { isPresented.wrappedValue = false }.buttonStyle(.ghost)
                        }
                    }.padding(sheet ? EdgeInsets(leading: 20, bottom: 4, trailing: 20) : EdgeInsets(all: 18)).flex(0, shrink: 0)
                    if !sheet { Divider() }
                    ScrollView(.vertical, scrollbarGutter: sheet ? .overlay : .stable) {
                        content.padding(sheet ? EdgeInsets(top: 8, leading: 20, bottom: 8, trailing: 20) : EdgeInsets(all: 18)).flex(0, shrink: 0)
                    }
                        .flex(fits ? 0 : 1, shrink: 1, basis: fits ? nil : 0).frame(minHeight: 0)
                    if !sheet { Divider() }
                    ActionGroupLayout(stacks: surface.viewport.widthClass == .compact) { footer }
                        .padding(sheet ? EdgeInsets(top: 8, leading: 20, bottom: bottom + 12, trailing: 20) : EdgeInsets(all: 14))
                        .flex(0, shrink: 0)
                }.accessibility { $0.role = .dialog; $0.label = title; $0.help = options.description }
            }
        }
    }
}
public extension Dialog where Footer == EmptyView {
    init(isPresented: Binding<Bool>, title: String, configure: (inout DialogOptions) -> Void = { _ in }, @ViewBuilder content: () -> Content) {
        self.init(isPresented: isPresented, title: title, configure: configure, content: content) { EmptyView() }
    }
}
public struct AlertDialogOptions {
    public var confirmTitle = "Confirm"
    public var cancelTitle = "Cancel"
    public var isDestructive = false
    public var isConfirming = false
    public var dismissAfterConfirm = true
    public init() {}
}
public struct AlertDialog: View {
    public let isPresented: Binding<Bool>
    public let title: String
    public let message: String
    public let onConfirm: () -> Void
    public var options = AlertDialogOptions()
    public init(isPresented: Binding<Bool>, title: String, message: String, configure: (inout AlertDialogOptions) -> Void = { _ in }, onConfirm: @escaping () -> Void) {
        self.isPresented = isPresented; self.title = title; self.message = message; self.onConfirm = onConfirm; configure(&options)
    }
    public var body: some View {
        TextScalerReader { scaler in
        Dialog(isPresented: isPresented, title: title, configure: {
            $0.modal.geometry.width = 420
            $0.modal.geometry.height = max(280, scaler.scale(200) + 80)
            $0.modal.fitsContentHeight = true
            $0.modal.dismissal.closesOnBackdrop = !options.isConfirming; $0.modal.dismissal.closesOnEscape = !options.isConfirming
            $0.showsCloseButton = false; $0.adaptsToTouch = false
        }, content: { Text(message).font(.body).foregroundColor(.onSurfaceVariant) }, footer: {
            Button(options.cancelTitle, isEnabled: !options.isConfirming) { isPresented.wrappedValue = false }.buttonStyle(.secondary)
            Button(options.confirmTitle, role: options.isDestructive ? .destructive : .normal, isLoading: options.isConfirming) {
                guard isPresented.wrappedValue, !options.isConfirming else { return }
                onConfirm(); if options.dismissAfterConfirm { isPresented.wrappedValue = false }
            }.buttonStyle(options.isDestructive ? AnyButtonStyle(DestructiveButtonStyle()) : AnyButtonStyle(PrimaryButtonStyle()))
        })
        }
    }
}
public struct SheetOptions {
    public var edge: ModalPlacement = .trailing
    public var extent: Float = 400
    /// Margin from the window edge; keeps the rounded sheet visually floating.
    public var inset: Float = 12
    public var cornerRadius: Float = 14
    public var dismissal = ModalDismissal()
    public init() {}
    mutating func validate() {
        extent = extent.isFinite ? max(64, extent) : 400
        inset = inset.isFinite ? max(0, inset) : 12
        cornerRadius = cornerRadius.isFinite ? max(0, cornerRadius) : 14
        if edge == .center { edge = .trailing }
    }
}
public struct Sheet<Content: View>: View {
    public let isPresented: Binding<Bool>
    public var options = SheetOptions()
    private let content: Content
    public init(isPresented: Binding<Bool>, configure: (inout SheetOptions) -> Void = { _ in }, @ViewBuilder content: () -> Content) {
        self.isPresented = isPresented; self.content = content(); configure(&options)
        options.validate()
    }
    public var body: some View {
        Modal(isPresented: isPresented, configure: {
            $0.geometry.placement = options.edge
            $0.geometry.inset = options.inset
            $0.geometry.cornerRadius = options.cornerRadius
            let horizontalEdge = options.edge == .top || options.edge == .bottom
            $0.geometry.width = horizontalEdge ? .greatestFiniteMagnitude : options.extent
            $0.geometry.height = horizontalEdge ? options.extent : .greatestFiniteMagnitude
            $0.dismissal = options.dismissal
        }) { content }
    }
}
