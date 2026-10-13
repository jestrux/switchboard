import SwiftUI

/// One window: a sidebar scoped to all projects or a single one, beside the terminal pane.
struct ContentView: View {
    @EnvironmentObject var store: EngineStore
    @Environment(\.openWindow) private var openWindow
    @Environment(\.dismissWindow) private var dismissWindow
    @StateObject private var context: WindowContext
    @State private var columnVisibility = NavigationSplitViewVisibility.all

    init(scope: WindowScope) {
        _context = StateObject(wrappedValue: WindowContext(scope: scope))
    }

    private var projectSlug: String? {
        if case .project(let slug) = context.scope { return slug }
        return nil
    }

    private var scopedProject: ProjectInfo? { projectSlug.flatMap(store.project) }

    var body: some View {
        Group {
            if let slug = projectSlug, scopedProject == nil {
                projectGone(slug)
            } else {
                splitView
            }
        }
        .focusedSceneValue(\.windowFocus, WindowFocus(context: context, selection: context.selection))
        .environmentObject(context)
        .onAppear {
            store.registerWindow(context)
            context.reconcile(with: store)
            if context.scope == .all { store.offerToKillLeftovers() }
        }
        .onChange(of: store.projects) { _, _ in context.reconcile(with: store) }
        .onChange(of: store.lastInstalled) { _, slug in
            // The window that added a project selects it; only the main window qualifies.
            guard context.scope == .all, let slug, let p = store.project(slug), let c = p.allCommands.first else { return }
            context.selection = CommandRef(project: slug, address: c.address)
        }
        .onDisappear { store.releaseAll(from: context) }
    }

    private var splitView: some View {
        NavigationSplitView(columnVisibility: $columnVisibility) {
            SidebarView()
                .navigationSplitViewColumnWidth(min: 220, ideal: 260, max: 420)
        } detail: {
            if let ref = context.selection {
                TerminalPane(ref: ref)
                    .id(ref)
            } else {
                ContentUnavailableView {
                    Label(scopedProject?.name ?? "Switchboard", systemImage: "switch.2")
                } description: {
                    Text(store.projects.isEmpty
                         ? "Add a project to get started: drop a folder with config.yaml onto the sidebar."
                         : "Select a command to see its terminal.")
                } actions: {
                    if context.scope == .all { Button("Add Project…") { store.addProjectViaPanel() } }
                }
            }
        }
        .toolbar { toolbar }
        .frame(minWidth: 820, minHeight: 480)
    }

    /// A project window whose project was deleted or renamed away.
    private func projectGone(_ slug: String) -> some View {
        ContentUnavailableView {
            Label("Project removed", systemImage: "folder.badge.questionmark")
        } description: {
            Text("“\(slug)” is no longer in the Projects folder.")
        } actions: {
            Button("Close Window") { dismissWindow() }
            Button("Show All Projects") { openWindow(id: "main") }
        }
        .frame(minWidth: 480, minHeight: 320)
    }

    @ToolbarContentBuilder
    private var toolbar: some ToolbarContent {
        ToolbarItemGroup(placement: .navigation) {
            if context.scope == .all {
                Button { store.addProjectViaPanel() } label: { Label("Add Project…", systemImage: "plus") }
                    .help("Add Project… (⌘O)")
            } else {
                Button { openWindow(id: "main") } label: { Label("All Projects", systemImage: "square.grid.2x2") }
                    .help("Show the main window with every project (⌘0)")
            }
        }
        ToolbarItemGroup(placement: .primaryAction) {
            if let ref = context.selection {
                let state = store.state(ref)
                let cmd = store.command(ref)
                if state.isAlive {
                    Button { store.stop(ref) } label: { Label("Stop", systemImage: "stop.fill") }
                        .help("Stop (⌘.)")
                    Button { store.restart(ref) } label: { Label("Restart", systemImage: "arrow.clockwise") }
                        .help("Restart (⌘R)")
                } else {
                    Button { store.start(ref) } label: { Label("Start", systemImage: "play.fill") }
                        .help("Start")
                        .disabled(cmd?.cwdExists == false)
                }
                Divider()
                Button { store.editConfig(ref.project) } label: { Label("Edit Config", systemImage: "doc.text") }
                    .help("Open config.yaml in VS Code")
                Button { store.revealInFinder(ref.project) } label: { Label("Reveal in Finder", systemImage: "folder") }
                    .help("Reveal config.yaml in Finder")
            }
        }
    }
}
