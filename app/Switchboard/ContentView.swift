import SwiftUI

struct ContentView: View {
    @EnvironmentObject var store: EngineStore
    @State private var columnVisibility = NavigationSplitViewVisibility.all

    var body: some View {
        NavigationSplitView(columnVisibility: $columnVisibility) {
            SidebarView()
                .navigationSplitViewColumnWidth(min: 220, ideal: 260, max: 420)
        } detail: {
            if let ref = store.selection {
                TerminalPane(ref: ref)
                    .id(ref)
            } else {
                ContentUnavailableView {
                    Label("Switchboard", systemImage: "switch.2")
                } description: {
                    Text(store.projects.isEmpty
                         ? "Add a project to get started: drop a folder with config.yaml onto the sidebar."
                         : "Select a command to see its terminal.")
                } actions: {
                    Button("Add Project…") { store.addProjectViaPanel() }
                }
            }
        }
        .toolbar { toolbar }
        .frame(minWidth: 820, minHeight: 480)
        .task { store.offerToKillLeftovers() }
    }

    @ToolbarContentBuilder
    private var toolbar: some ToolbarContent {
        ToolbarItemGroup(placement: .navigation) {
            Button { store.addProjectViaPanel() } label: { Label("Add Project…", systemImage: "plus") }
                .help("Add Project… (⌘O)")
        }
        ToolbarItemGroup(placement: .primaryAction) {
            if let ref = store.selection {
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
