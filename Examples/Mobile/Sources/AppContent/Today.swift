import GuavaUI

struct TodayScreen: View {
    let model: Binding<MobileModel>

    var body: some View {
        ScrollView(.vertical) {
            Column(alignment: .leading, spacing: 16) {
                Text("Today").font(.display).padding(EdgeInsets(top: 12))
                summary
                Text("Continue").font(.headline).padding(EdgeInsets(top: 8))
                for project in model.wrappedValue.projects.prefix(2) {
                    AnyView(ProjectCard(project: project))
                }
            }.padding(horizontal: 20, vertical: 8).flex(0, shrink: 0).frame(width: .percent(100), minWidth: 0)
        }.flex(1, shrink: 1, basis: 0).frame(minHeight: 0)
    }

    private var summary: some View {
        Row(alignment: .center, spacing: 12) {
            metric("4", "Projects")
            metric("12", "Renders")
            metric("3", "Reviews")
        }.frame(width: .percent(100))
    }

    private func metric(_ value: String, _ label: String) -> some View {
        Column(alignment: .leading, spacing: 2) {
            Text(value).font(.title)
            Text(label).font(.caption).foregroundColor(.onSurfaceMuted)
        }.padding(16).flex(1, shrink: 1, basis: 0).frame(minWidth: 0)
            .background(.surfaceRaised).cornerRadius(16)
    }
}

struct ProjectCard: View {
    let project: Project

    var body: some View {
        Column(alignment: .leading, spacing: 10) {
            Row(alignment: .center, spacing: 8) {
                Text(project.name).font(.bodyStrong).flex(1, shrink: 1).frame(minWidth: 0)
                Badge(project.kind)
            }
            ProgressView(value: project.progress)
            Text("Edited \(project.updated)").font(.caption).foregroundColor(.onSurfaceMuted)
        }.padding(16).frame(width: .percent(100), minWidth: 0).background(.surfaceRaised).cornerRadius(16)
    }
}
