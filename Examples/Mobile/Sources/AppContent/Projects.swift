import GuavaUI

struct ProjectsScreen: View {
    let model: Binding<MobileModel>
    @State private var path: [Project] = []
    @State private var query: TextBuffer = ""

    var body: some View {
        NavigationStack(path: $path) {
            Box(direction: .column, alignItems: .stretch, spacing: 0) {
                Text("Projects").font(.display).padding(EdgeInsets(top: 12, leading: 20, bottom: 8, trailing: 20))
                TextField("Search", text: $query) { $0.behavior.clearable = true }
                    .padding(horizontal: 20, vertical: 4)
                SectionedList([ListSection("recent", title: "Recent", items: matches)],
                              onActivate: { path.append($0) }) { project, _ in
                    ListRow(project.name, configure: {
                        $0.subtitle = "\(project.kind) · \(project.updated)"
                        $0.showsDisclosure = true
                    }, leading: {
                        Avatar(project.name)
                    }, trailing: { EmptyView() })
                }.flex().frame(minHeight: 0)
            }.flex().frame(minWidth: 0, minHeight: 0)
        } destination: { project in
            ProjectDetail(project: project) { path.removeLast() }
        }
    }

    private var matches: [Project] {
        let text = query.stringValue.trimmingCharacters(in: .whitespaces)
        return text.isEmpty ? model.wrappedValue.projects : model.wrappedValue.projects.filter { $0.name.localizedCaseInsensitiveContains(text) }
    }
}

private struct ProjectDetail: View {
    let project: Project
    let back: () -> Void
    @State private var sharing = false

    var body: some View {
        Box(direction: .column, alignItems: .stretch, spacing: 0) {
            AppBar(project.name, back: MenuItem(title: "Projects", action: back))
            ScrollView(.vertical) {
                Column(alignment: .leading, spacing: 16) {
                    ProjectCard(project: project)
                    Button("Open in Studio") {}.frame(width: .percent(100))
                    Button("Share") { sharing = true }.buttonStyle(.secondary).frame(width: .percent(100))
                }.padding(20).flex(0, shrink: 0).frame(width: .percent(100), minWidth: 0)
            }.flex().frame(minHeight: 0)
            Dialog(isPresented: $sharing, title: "Share \(project.name)") {
                Text("guava.studio/p/\(project.id)").foregroundColor(.onSurfaceMuted)
            } footer: {
                Button("Copy link") { sharing = false }.frame(width: .percent(100))
            }
        }.flex().frame(minWidth: 0, minHeight: 0)
    }
}
