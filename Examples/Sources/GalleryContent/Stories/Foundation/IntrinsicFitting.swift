import CoveUI

struct ViewThatFitsStory: View {
    @State private var narrow = false

    var body: some View {
        StorySection("Intrinsic fitting", "The wide branch is measured at an unspecified width. It stays selected while its natural width and height fit; otherwise the compact branch replaces it while both subtrees retain their state.") {
            Button(narrow ? "Use wide proposal" : "Use narrow proposal") { narrow.toggle() }
                .buttonStyle(.secondary)
            ViewThatFits(fitting: .intrinsic) {
                Row(spacing: 8) {
                    Text("Intrinsic wide").font(.caption)
                    chip("Files")
                    chip("Preview")
                    chip("Settings")
                }
            } compact: {
                Column(spacing: 8) {
                    Text("Intrinsic compact").font(.caption)
                    chip("Files")
                    chip("Preview")
                    chip("Settings")
                }
            }
            .frame(width: narrow ? 180 : 520)
            .debugName("intrinsic-fit")
        }
    }

    private func chip(_ title: String) -> some View {
        Text(title).font(.caption)
            .padding(horizontal: 10, vertical: 6)
            .background(.surfaceVariant).cornerRadius(6)
    }
}
