import CoveUICompose
import CoveUIRuntime

struct AdaptiveApplicationContent<Root: View>: View {
    let content: Root
    @State var environment: UIEnvironment

    init(content: Root, environment: State<UIEnvironment>) {
        self.content = content
        self._environment = environment
    }
    var body: some View {
        SurfaceRoot(environment) { content }
    }
}
