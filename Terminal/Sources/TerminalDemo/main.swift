// A window with a live login shell, for trying CoveUITerminal by hand:
//
//     swift run --package-path CoveUI/Terminal TerminalDemo

import CoveUI
import CoveUITerminal

final class TerminalWindow: View {
    @State var title = ""
    @State var session = TerminalSession()

    var body: some View {
        session.onEvent = { [weak self] event in
            if case .titleChanged(let title) = event { self?.title = title }
        }
        return Column(spacing: 0) {
            Text(title.isEmpty ? "Terminal" : title)
                .font(.system(size: 13, weight: .semibold))
                .padding(EdgeInsets(top: 8, leading: 12, bottom: 8, trailing: 12))
            TerminalView(session: session, fontSize: 14)
        }
        .framePercent(width: 100, height: 100)
    }
}

try Application.run(config: ApplicationConfiguration(title: "CoveUI · Terminal") {
    $0.window.width = 900
    $0.window.height = 560
}) {
    TerminalWindow()
}
