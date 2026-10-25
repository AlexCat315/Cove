import GuavaUI

/// Desktop and mobile hosts install this shared root once per application session.
/// Preview it as a phone on desktop with `GUAVA_PREVIEW_DEVICE=phone guava run`.
public func makeApplicationContent() -> some View { RootView() }

enum Tab: Hashable { case today, projects, settings }

private struct RootView: View {
    @State private var tab = Tab.today
    @State private var model = MobileModel()

    var body: some View {
        SafeArea([.top, .horizontal]) {
            TabView(selection: $tab, tabs: [
                TabItem("Today", id: Tab.today, configure: { $0.icon = UIIcons.Shape.home }) {
                    TodayScreen(model: $model)
                },
                TabItem("Projects", id: Tab.projects, configure: { $0.icon = UIIcons.File.folder }) {
                    ProjectsScreen(model: $model)
                },
                TabItem("Settings", id: Tab.settings, configure: { $0.icon = UIIcons.Action.settings }) {
                    SettingsScreen(model: $model)
                },
            ]).flex(1, shrink: 1, basis: 0).frame(minWidth: 0, minHeight: 0)
        }
        .flex(1, shrink: 1, basis: 0).frame(minWidth: 0, minHeight: 0)
        .background(.background)
    }
}
