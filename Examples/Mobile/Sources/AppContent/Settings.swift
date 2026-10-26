import CoveUI

struct SettingsScreen: View {
    let model: Binding<MobileModel>
    @State private var choosingAppearance = false

    var body: some View {
        Box(direction: .column, alignItems: .stretch, spacing: 0) {
            ScrollView(.vertical) {
                Box(direction: .column, alignItems: .stretch, spacing: 16) {
                    Text("Settings").font(.display).padding(EdgeInsets(top: 12, bottom: 8))
                    ListGroup("General") {
                        ListRow("Notifications", trailing: { Toggle(isOn: model.field(\.notifications)) })
                        ListRow("Sync over cellular", trailing: { Toggle(isOn: model.field(\.syncOverCellular)) })
                        Button { choosingAppearance = true } label: {
                            ListRow("Appearance", configure: { $0.showsDisclosure = true }, trailing: {
                                Text(model.wrappedValue.appearance).foregroundColor(.onSurfaceMuted)
                            })
                        }.buttonStyle(.plain).frame(width: .percent(100))
                    }
                    ListGroup("Accessibility") {
                        ListRow("Reduce motion", trailing: { Toggle(isOn: model.field(\.reduceMotion)) })
                    }
                    ListGroup {
                        ListRow("Sign out", configure: { $0.role = .destructive })
                    }
                }.padding(horizontal: 20, vertical: 8).flex(0, shrink: 0).frame(width: .percent(100), minWidth: 0)
            }.flex(1, shrink: 1, basis: 0).frame(minHeight: 0)
            // The sheet presents in a portal, so it takes no room here.
            BottomSheet(isPresented: $choosingAppearance, configure: { $0.detents = [260] }) {
                Box(direction: .column, alignItems: .stretch, spacing: 8) {
                    Text("Appearance").font(.headline)
                    for choice in ["System", "Light", "Dark"] {
                        AnyView(choiceButton(choice))
                    }
                }.padding(20)
            }
        }
    }

    @ViewBuilder
    private func choiceButton(_ choice: String) -> some View {
        let pick = { model.wrappedValue.appearance = choice; choosingAppearance = false }
        if model.wrappedValue.appearance == choice {
            Button(choice, action: pick).buttonStyle(.primary)
        } else {
            Button(choice, action: pick).buttonStyle(.secondary)
        }
    }
}
