import Foundation
import Testing
import CoveUIRuntime
import CoveUICompose
import CoveUIDevToolsScene
@testable import GalleryContent
@testable import CoveUI

/// Acceptance for the app-root selection region: `Runtime` installs
/// `.textSelection()` around every screen, so mounting the real gallery the way
/// the app does must produce exactly the same geometry as mounting it bare.
/// A region that reports a different size collapses the whole window.
@Suite("Gallery root selection region", .serialized)
@MainActor
struct SelectionShellTests {

    private struct Row {
        let name: String
        let frame: CGRect
    }

    private func layout(selectable: Bool, phone: Bool = false) -> [Row] {
        let oldText = TextEnvironmentHolder.current
        TextEnvironmentHolder.current = TextEnvironment.bootstrapped(atlasTextureID: 1, primaryFontName: SystemFontDefaults.primaryFontName)
        defer { TextEnvironmentHolder.current = oldText }
        return AnimatorScheduler.$current.withValue(AnimatorScheduler()) {
            let graph = ViewGraph(tree: NodeTree(), recomposer: Recomposer())
            var environment = UIEnvironment()
            if phone {
                environment.input = [.touch, .keyboard]
                environment.idiom = .phone
                environment.viewport.width = 390
                environment.viewport.height = 820
            }
            let content = AdaptiveApplicationContent(content: GalleryView(initialPage: .button),
                                                     environment: State(wrappedValue: environment))
            graph.install(root: selectable ? AnyView(content.textSelection()) : AnyView(content))
            graph.computeLayout(width: 1280, height: 820)
            let rows = graph.layoutSnapshot().compactMap { entry -> Row? in
                guard let name = entry.debugName else { return nil }
                return Row(name: name, frame: entry.absoluteFrame)
            }
            graph.install(root: EmptyView())
            return rows
        }
    }

    private func report(_ rows: [Row], label: String) -> String {
        let interesting = rows.filter {
            $0.name == "gallery-shell" || $0.name == "gallery-content" || $0.name.hasPrefix("story-section-")
        }
        return "\(label): " + interesting.map { "\($0.name)=\($0.frame)" }.joined(separator: " | ")
    }

    private func shell(_ rows: [Row]) -> CGRect? {
        rows.first { $0.name == "gallery-shell" }?.frame
    }

    @Test("The shell keeps its size with the app-root selection region")
    func shellKeepsSize() {
        let bare = layout(selectable: false)
        let wrapped = layout(selectable: true)
        #expect(shell(bare)?.height == 820, "bare: \(report(bare, label: "bare"))")
        #expect(shell(wrapped)?.height == 820, "wrapped: \(report(wrapped, label: "wrapped"))")
        #expect(shell(bare) == shell(wrapped),
                "bare \(String(describing: shell(bare))) vs wrapped \(String(describing: shell(wrapped)))")
    }

    @Test("Story sections stay ordered and separated with the region")
    func sectionsStaySeparated() {
        let wrapped = layout(selectable: true)
        let sections = wrapped.filter { $0.name.hasPrefix("story-section-") }
        #expect(sections.count == 6, "wrapped: \(report(wrapped, label: "wrapped"))")
        for (first, second) in zip(sections, sections.dropFirst()) {
            #expect(first.frame.maxY <= second.frame.minY,
                    "overlap \(first.name) \(first.frame) / \(second.name) \(second.frame)")
        }
    }

    /// The phone surface takes the other branch: the region adds a menu sheet
    /// beside its content, and that must not steal height from the screen.
    @Test("A phone surface keeps the shell size with the region and its sheet")
    func phoneShellKeepsSize() {
        let bare = layout(selectable: false, phone: true)
        let wrapped = layout(selectable: true, phone: true)
        #expect(shell(bare)?.height == 820, "bare: \(report(bare, label: "bare"))")
        #expect(shell(wrapped)?.height == 820, "wrapped: \(report(wrapped, label: "wrapped"))")
        #expect(shell(bare) == shell(wrapped),
                "bare \(String(describing: shell(bare))) vs wrapped \(String(describing: shell(wrapped)))")
    }

    /// Selection only exists where a label is hit-testable, so the real screen
    /// has to end up with selectable labels — not just a correct layout.
    @Test("Dashboard labels become selectable inside the region")
    func dashboardLabelsAreSelectable() {
        let oldText = TextEnvironmentHolder.current
        TextEnvironmentHolder.current = TextEnvironment.bootstrapped(atlasTextureID: 1, primaryFontName: SystemFontDefaults.primaryFontName)
        let oldRegistry = InteractionRegistryHolder.current
        let oldFocus = FocusChainHolder.current
        let oldCapture = PointerCaptureHolder.current
        InteractionRegistryHolder.current = InteractionRegistry()
        FocusChainHolder.current = FocusChain()
        PointerCaptureHolder.current = PointerCapture()
        defer {
            TextEnvironmentHolder.current = oldText
            InteractionRegistryHolder.current = oldRegistry
            FocusChainHolder.current = oldFocus
            PointerCaptureHolder.current = oldCapture
        }
        AnimatorScheduler.$current.withValue(AnimatorScheduler()) {
            let graph = ViewGraph(tree: NodeTree(), recomposer: Recomposer())
            let content = AdaptiveApplicationContent(content: GalleryView(initialPage: .introduction),
                                                     environment: State(wrappedValue: UIEnvironment()))
            graph.install(root: AnyView(content.textSelection()))
            for _ in 0..<3 { graph.recomposer.commitAll(); graph.computeLayout(width: 1280, height: 820) }
            defer { graph.install(root: EmptyView()) }

            var labels: [(String, Bool, Bool)] = []
            func walk(_ node: Node) {
                if node.accessibility?.role == .staticText, let label = node.accessibility?.label, !label.isEmpty {
                    labels.append((label, node.isHitTestable, node.isFocusable))
                }
                node.children.forEach(walk)
            }
            if let root = graph.tree.root { walk(root) }
            let selectable = labels.filter(\.1)
            let report = labels.prefix(12).map { "\($0.0.prefix(24)) hit=\($0.1) focus=\($0.2)" }
                .joined(separator: " | ")
            #expect(labels.count > 20, "labels: \(labels.count) \(report)")
            #expect(selectable.count > 10, "selectable: \(selectable.count)/\(labels.count) \(report)")
        }
    }

    /// Pointer routes between a label and the root, so a failure says which
    /// ancestor claimed the gesture instead of just "nothing was copied".
    private func chain(_ node: Node) -> String {
        var lines: [String] = []
        var current: Node? = node
        while let step = current {
            let route = InteractionRegistryHolder.current?.handlers(for: step).pointerRoute
            if let route {
                let name = (step.attachments[LayoutDebugAttachmentKey.debugName] as? String)
                    ?? step.viewTag.map { String($0.prefix(90)) } ?? "-"
                lines.append("  \(name) frame=\(step.frame) route=\(route.role)/\(route.debugName)")
            }
            current = step.parent
        }
        return lines.isEmpty ? "  (no pointer routes above the label)" : lines.joined(separator: "\n")
    }

    /// Drags across a real heading with the real dispatcher: this is what the
    /// pointer actually does in the running app.
    @Test("Dragging a dashboard heading selects and copies it")
    func dragCopiesHeading() {
        let oldText = TextEnvironmentHolder.current
        TextEnvironmentHolder.current = TextEnvironment.bootstrapped(atlasTextureID: 1, primaryFontName: SystemFontDefaults.primaryFontName)
        let oldRegistry = InteractionRegistryHolder.current
        let oldFocus = FocusChainHolder.current
        let oldCapture = PointerCaptureHolder.current
        let oldClipboard = ClipboardHolder.write
        let registry = InteractionRegistry(), focus = FocusChain(), capture = PointerCapture()
        InteractionRegistryHolder.current = registry
        FocusChainHolder.current = focus
        PointerCaptureHolder.current = capture
        defer {
            TextEnvironmentHolder.current = oldText
            InteractionRegistryHolder.current = oldRegistry
            FocusChainHolder.current = oldFocus
            PointerCaptureHolder.current = oldCapture
            ClipboardHolder.write = oldClipboard
        }
        AnimatorScheduler.$current.withValue(AnimatorScheduler()) {
            let graph = ViewGraph(tree: NodeTree(), recomposer: Recomposer())
            let dispatcher = EventDispatcher(tree: graph.tree, interactions: registry,
                                             capture: capture, focusChain: focus)
            let content = AdaptiveApplicationContent(content: GalleryView(initialPage: .introduction),
                                                     environment: State(wrappedValue: UIEnvironment()))
            graph.install(root: AnyView(content.textSelection()))
            for _ in 0..<3 { graph.recomposer.commitAll(); graph.computeLayout(width: 1280, height: 820) }
            defer { graph.install(root: EmptyView()) }

            var heading: Node?
            func walk(_ node: Node) {
                if heading == nil, node.accessibility?.role == .staticText,
                   let label = node.accessibility?.label, label.count > 8 {
                    heading = node
                }
                node.children.forEach(walk)
            }
            if let root = graph.tree.root { walk(root) }
            guard let target = heading else { return #expect(Bool(false), "no heading") }
            let frame = target.absoluteFrame
            let from = CGPoint(x: frame.minX + 1, y: frame.midY)
            let to = CGPoint(x: frame.maxX - 1, y: frame.midY)
            dispatcher.dispatch(.mouseButtonDown(MouseButtonEvent(button: .left, x: Float(from.x),
                                                                  y: Float(from.y), clicks: 1)))
            for step in 1...4 {
                let t = Float(step) / 4
                dispatcher.dispatch(.mouseMotion(MouseMotionEvent(x: Float(from.x + (to.x - from.x) * CGFloat(t)),
                                                                  y: Float(to.y),
                                                                  deltaX: 1, deltaY: 0)))
            }
            dispatcher.dispatch(.mouseButtonUp(MouseButtonEvent(button: .left, x: Float(to.x),
                                                                y: Float(to.y), clicks: 1)))
            var copied: String?
            ClipboardHolder.write = { copied = $0 }
            dispatcher.dispatch(.keyDown(KeyEvent(scancode: Scancode.c, keycode: 0,
                                                  modifiers: .gui, isRepeat: false)))
            #expect(copied?.isEmpty == false,
                    "hit=\(target.isHitTestable) focus=\(target.isFocusable) frame=\(frame) label=\(target.accessibility?.label ?? "-")\n\(chain(target))")
        }
    }

    /// The whole catalog, one page at a time, mounted the way the app mounts it.
    ///
    /// Only *leaf* labels count: `Badge`, `Tag`, `KeyCap` and `StatusBar` publish
    /// combined semantics on their container node, which is deliberately not
    /// hit-testable — the real `Text` inside them is what a press has to reach.
    /// A leaf that cannot be pressed is only acceptable when a control owns it
    /// (a label inside a button belongs to that button). Anything else means a
    /// gesture container swallowed the page, which is exactly the bug that made
    /// the whole window unselectable.
    @Test("Every catalog page makes its labels pressable")
    func everyPagePressesItsLabels() {
        let oldText = TextEnvironmentHolder.current
        TextEnvironmentHolder.current = TextEnvironment.bootstrapped(atlasTextureID: 1, primaryFontName: SystemFontDefaults.primaryFontName)
        let oldRegistry = InteractionRegistryHolder.current
        let oldFocus = FocusChainHolder.current
        let oldCapture = PointerCaptureHolder.current
        InteractionRegistryHolder.current = InteractionRegistry()
        FocusChainHolder.current = FocusChain()
        PointerCaptureHolder.current = PointerCapture()
        defer {
            TextEnvironmentHolder.current = oldText
            InteractionRegistryHolder.current = oldRegistry
            FocusChainHolder.current = oldFocus
            PointerCaptureHolder.current = oldCapture
        }
        var offenders: [String] = []
        var pages = 0, total = 0
        AnimatorScheduler.$current.withValue(AnimatorScheduler()) {
            let graph = ViewGraph(tree: NodeTree(), recomposer: Recomposer())
            for page in GalleryPage.allCases {
                let content = AdaptiveApplicationContent(content: GalleryView(initialPage: page),
                                                         environment: State(wrappedValue: UIEnvironment()))
                graph.install(root: AnyView(content.textSelection()))
                for _ in 0..<2 { graph.recomposer.commitAll(); graph.computeLayout(width: 1280, height: 820) }
                pages += 1
                var labels = 0
                var blocked: [Node] = []
                // Returns whether the subtree publishes any label at all, so a
                // container that combines its children's text is not counted as
                // a label itself.
                @discardableResult
                func walk(_ node: Node) -> Bool {
                    let isText = node.accessibility?.role == .staticText
                        && (node.accessibility?.label.isEmpty == false)
                    var below = false
                    for child in node.children where walk(child) { below = true }
                    if isText, !below {
                        labels += 1
                        if !node.isHitTestable {
                            var owner: Node?
                            var ancestor = node.parent
                            while let current = ancestor {
                                let route = InteractionRegistryHolder.current?
                                    .handlers(for: current).pointerRoute
                                if let route, route.priority <= .normal, route.role != .scroll {
                                    owner = current
                                    break
                                }
                                ancestor = current.parent
                            }
                            if owner == nil { blocked.append(node) }
                        }
                    }
                    return isText || below
                }
                if let root = graph.tree.root { walk(root) }
                total += labels
                if !blocked.isEmpty, let first = blocked.first {
                    offenders.append("\(page.rawValue): \(blocked.count)/\(labels) inert — "
                        + "\"\(first.accessibility?.label ?? "-")\" frame=\(first.absoluteFrame)\n\(chain(first))")
                }
                graph.install(root: EmptyView())
            }
        }
        let report = offenders.prefix(6).joined(separator: "\n\n")
        #expect(pages == GalleryPage.allCases.count, "mounted \(pages) pages")
        #expect(total > 1000, "only \(total) leaf labels across the catalog")
        #expect(offenders.isEmpty, "pages with inert labels: \(offenders.count)\n\(report)")
    }

    /// The end-to-end form of the same claim: drag across a visible label with
    /// the real dispatcher and press `Cmd+C`. Flags can be right while the
    /// gesture path is still broken, so this drives the actual press → motion →
    /// release → copy sequence.
    ///
    /// One page per distinct page-level container rather than the whole catalog:
    /// dragging costs ~7 s a page, and the container is what decides whether a
    /// press survives — `everyPagePressesItsLabels` already covers every page.
    @Test("A dragged label copies on each page shell")
    func draggedLabelCopiesOnEachShell() {
        let pages: [GalleryPage] = [
            .introduction, .button, .form, .tabs, .list, .table, .dataTable, .tree,
            .sidebar, .panel, .splitView, .scrollView, .workspace, .richText, .markdown,
        ]
        let oldText = TextEnvironmentHolder.current
        TextEnvironmentHolder.current = TextEnvironment.bootstrapped(atlasTextureID: 1, primaryFontName: SystemFontDefaults.primaryFontName)
        let oldRegistry = InteractionRegistryHolder.current
        let oldFocus = FocusChainHolder.current
        let oldCapture = PointerCaptureHolder.current
        let oldClipboard = ClipboardHolder.write
        let registry = InteractionRegistry(), focus = FocusChain(), capture = PointerCapture()
        InteractionRegistryHolder.current = registry
        FocusChainHolder.current = focus
        PointerCaptureHolder.current = capture
        defer {
            TextEnvironmentHolder.current = oldText
            InteractionRegistryHolder.current = oldRegistry
            FocusChainHolder.current = oldFocus
            PointerCaptureHolder.current = oldCapture
            ClipboardHolder.write = oldClipboard
        }
        var failures: [String] = []
        var copiedPages = 0
        AnimatorScheduler.$current.withValue(AnimatorScheduler()) {
            let graph = ViewGraph(tree: NodeTree(), recomposer: Recomposer())
            let dispatcher = EventDispatcher(tree: graph.tree, interactions: registry,
                                             capture: capture, focusChain: focus)
            let window = CGRect(x: 0, y: 0, width: 1280, height: 820)
            for page in pages {
                let content = AdaptiveApplicationContent(content: GalleryView(initialPage: page),
                                                         environment: State(wrappedValue: UIEnvironment()))
                graph.install(root: AnyView(content.textSelection()))
                for _ in 0..<2 { graph.recomposer.commitAll(); graph.computeLayout(width: 1280, height: 820) }
                // Candidates: visible labels the pointer can actually reach.
                var candidates: [(String, CGRect)] = []
                func walk(_ node: Node) {
                    if node.accessibility?.role == .staticText,
                       let label = node.accessibility?.label, label.count > 3,
                       node.isHitTestable, node.isFocusable {
                        let frame = node.absoluteFrame
                        if frame.width > 24, window.contains(CGPoint(x: frame.midX, y: frame.midY)) {
                            candidates.append((label, frame))
                        }
                    }
                    node.children.forEach(walk)
                }
                if let root = graph.tree.root { walk(root) }
                var copied: String?
                ClipboardHolder.write = { copied = $0 }
                for (_, frame) in candidates.prefix(3) {
                    let from = CGPoint(x: frame.minX + 1, y: frame.midY)
                    let to = CGPoint(x: frame.maxX - 1, y: frame.midY)
                    dispatcher.dispatch(.mouseButtonDown(MouseButtonEvent(button: .left, x: Float(from.x),
                                                                          y: Float(from.y), clicks: 1)))
                    for step in 1...3 {
                        let t = Float(step) / 3
                        dispatcher.dispatch(.mouseMotion(MouseMotionEvent(
                            x: Float(from.x + (to.x - from.x) * CGFloat(t)), y: Float(to.y),
                            deltaX: 1, deltaY: 0)))
                    }
                    dispatcher.dispatch(.mouseButtonUp(MouseButtonEvent(button: .left, x: Float(to.x),
                                                                        y: Float(to.y), clicks: 1)))
                    dispatcher.dispatch(.keyDown(KeyEvent(scancode: Scancode.c, keycode: 0,
                                                          modifiers: .gui, isRepeat: false)))
                    if copied?.isEmpty == false { break }
                }
                if copied?.isEmpty == false {
                    copiedPages += 1
                } else {
                    failures.append("\(page.rawValue): candidates=\(candidates.count) "
                        + "copied=\(copied.map { "\"\($0)\"" } ?? "nil")"
                        + (candidates.first.map { " first=\"\($0.0)\" frame=\($0.1)" } ?? ""))
                }
                graph.install(root: EmptyView())
            }
        }
        let report = failures.prefix(8).joined(separator: "\n")
        #expect(failures.isEmpty, "pages that did not copy: \(failures.count)\n\(report)")
        #expect(copiedPages == pages.count, "copied \(copiedPages)/\(pages.count) pages")
    }

    /// The acceptance the DevTools client itself performs: mount the real app
    /// root, then read it back through the diagnostic tools (`get_diagnostics`,
    /// `query_ui_nodes`) that DevTools exposes to AI clients.
    @Test("DevTools diagnostics report a healthy tree with the region installed")
    func devToolsDiagnostics() async throws {
        let oldText = TextEnvironmentHolder.current
        TextEnvironmentHolder.current = TextEnvironment.bootstrapped(atlasTextureID: 1, primaryFontName: SystemFontDefaults.primaryFontName)
        defer { TextEnvironmentHolder.current = oldText }
        try await AnimatorScheduler.$current.withValue(AnimatorScheduler()) {
            let graph = ViewGraph(tree: NodeTree(), recomposer: Recomposer())
            let content = AdaptiveApplicationContent(content: GalleryView(initialPage: .button),
                                                     environment: State(wrappedValue: UIEnvironment()))
            graph.install(root: AnyView(content.textSelection()))
            graph.computeLayout(width: 1280, height: 820)
            defer { graph.install(root: EmptyView()) }

            let diagnostics = UIDiagnostics(tree: graph.tree, state: graph.stateRegistry)
            let toolbox = try diagnostics.toolbox()

            let overview = try await toolbox.execute(name: "get_diagnostics",
                                                     input: Data("{}".utf8))
            let overviewJSON = try JSONSerialization.jsonObject(with: overview.json) as? [String: Any]
            let data = try #require(overviewJSON?["data"] as? [String: Any], "\(String(decoding: overview.json, as: UTF8.self))")
            #expect(data["hasRoot"] as? Bool == true)
            #expect((data["nodeCount"] as? Int ?? 0) > 100)
            let issues = try #require(data["layoutIssues"] as? [String: Any])
            #expect(issues["total"] as? Int == 0,
                    "layout issues: \(String(decoding: overview.json, as: UTF8.self))")

            let shell = try await toolbox.execute(name: "query_ui_nodes",
                                                  input: Data(#"{"text":"gallery-shell","compact":true}"#.utf8))
            let shellJSON = String(decoding: shell.json, as: UTF8.self)
            #expect(shellJSON.contains("gallery-shell"), "\(shellJSON)")
            #expect(!shell.isError, "\(shellJSON)")
        }
    }
}
