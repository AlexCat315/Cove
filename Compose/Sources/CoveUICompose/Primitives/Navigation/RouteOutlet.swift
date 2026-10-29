import CoveUIScene

/// An active-route composition boundary. Only the selected destination is
/// constructed; changing its key releases the previous page's local state.
/// Keep the application shell outside this outlet and read selection here.
public struct RouteOutlet<Route: Hashable, Content: View>: View {
    public let selection: Binding<Route>
    private let destination: (Route) -> Content

    public init(selection: Binding<Route>, @ViewBuilder destination: @escaping (Route) -> Content) {
        self.selection = selection
        self.destination = destination
    }

    public var body: some View {
        let route = selection.wrappedValue
        destination(route).id(route)
    }
}
