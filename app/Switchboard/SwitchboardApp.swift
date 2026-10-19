import SwiftUI

@main
struct SwitchboardApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate
    @StateObject private var store = EngineStore.shared
    @Environment(\.openWindow) private var openWindow
    @Environment(\.dismissWindow) private var dismissWindow

    @FocusedValue(\.windowFocus) private var focus

    private var selection: CommandRef? { focus?.selection }
    private var selectedIsAlive: Bool { selection.map { store.state($0).isAlive } ?? false }

    var body: some Scene {
        Window("Switchboard", id: "main") {
            ContentView(scope: .all)
                .environmentObject(store)
        }
        .windowStyle(.hiddenTitleBar)
        .defaultSize(width: 1180, height: 720)
        .commands { appCommands }

        // One window per project; opening the same project again focuses its window.
        WindowGroup("Project", id: "project", for: String.self) { $slug in
            if let slug {
                ContentView(scope: .project(slug))
                    .environmentObject(store)
            }
        }
        .windowStyle(.hiddenTitleBar)
        .defaultSize(width: 980, height: 680)

        Settings {
            SettingsView()
        }
    }

    @CommandsBuilder
    private var appCommands: some Commands {
        CommandGroup(after: .appSettings) {
            Button("Install switchboard Command…") { store.installTools() }
        }
        CommandGroup(replacing: .newItem) {
            Button("Add Project…") { store.addProjectViaPanel() }
                .keyboardShortcut("o", modifiers: [.command])
            #if DEBUG
            Button("Install Sample Project (Kinpay)") { store.installSampleProject() }
            #endif
        }
        CommandMenu("Command") {
            Button("New Terminal") {
                guard let ctx = focus?.context else { return }
                let project: String? = {
                    if let s = selection { return s.project }
                    if case .project(let slug) = ctx.scope { return slug }
                    return ctx.railProject
                }()
                guard let project else { return }
                let group = selection.flatMap { store.command($0)?.groupId }
                if let ref = store.newTerminal(project: project, group: group) { ctx.selection = ref }
            }
            .keyboardShortcut("t", modifiers: [.command])
            .disabled(focus == nil)
            Divider()
            Button("Start") { if let s = selection { store.start(s) } }
                .disabled(selection == nil || selectedIsAlive)
            Button("Stop") { if let s = selection { store.stop(s) } }
                .keyboardShortcut(".", modifiers: [.command])
                .disabled(!selectedIsAlive && !(selection.map { store.state($0).restartAt != nil } ?? false))
            Button("Restart") { if let s = selection { store.restart(s) } }
                .keyboardShortcut("r", modifiers: [.command])
                .disabled(selection == nil)
            Divider()
            Button("Clear Terminal") {
                if let s = selection { TerminalRegistry.shared.existingHost(for: s)?.clear() }
            }
            .keyboardShortcut("k", modifiers: [.command])
            .disabled(selection == nil)
            Divider()
            Button("Start All in Project") { if let s = selection { store.startAll(project: s.project) } }
                .disabled(selection == nil)
            Button("Stop All in Project") { if let s = selection { store.stopAll(project: s.project) } }
                .disabled(selection == nil)
            Button("Stop Everything") { store.engine.stopEverything() }
                .disabled(store.totalRunning == 0)
        }
        CommandMenu("View") {
            Button("Split Right") { focus?.context.split(axis: .horizontal) }
                .keyboardShortcut("d", modifiers: [.command])
            Button("Split Down") { focus?.context.split(axis: .vertical) }
                .keyboardShortcut("d", modifiers: [.command, .shift])
            Button("Maximize Pane") { focus?.context.toggleMaximize() }
                .keyboardShortcut(.return, modifiers: [.command, .shift])
                .disabled(!(focus?.canClosePane ?? false))
            Button("Close Pane") { focus?.context.closePane() }
                .keyboardShortcut("w", modifiers: [.command, .shift])
                .disabled(!(focus?.canClosePane ?? false))
            Divider()
            Button("Next Pane") { focus?.context.focusNextPane() }
                .keyboardShortcut("]", modifiers: [.command])
                .disabled(!(focus?.canClosePane ?? false))
            Button("Previous Pane") { focus?.context.focusPreviousPane() }
                .keyboardShortcut("[", modifiers: [.command])
                .disabled(!(focus?.canClosePane ?? false))
            Divider()
            Button("Reset Layout") { focus?.context.resetLayout() }
                .disabled(!(focus?.canClosePane ?? false))
        }
        CommandMenu("Project") {
            Button("Open in New Window") {
                if let s = selection { openWindow(id: "project", value: s.project) }
            }
            .keyboardShortcut("n", modifiers: [.command, .shift])
            .disabled(selection == nil || focus?.context.scope != .all)
            Button("Move to Board") {
                if case .project(let slug)? = focus?.context.scope { dismissWindow(id: "project", value: slug) }
            }
            .disabled(focus.map { if case .project = $0.context.scope { return false } else { return true } } ?? true)
            Button("Show Board") { openWindow(id: "main") }
                .keyboardShortcut("0", modifiers: [.command])
            Divider()
            Button("Edit Config") { if let s = selection { store.editConfig(s.project) } }
                .keyboardShortcut("e", modifiers: [.command])
                .disabled(selection == nil)
            Button("Reveal in Finder") { if let s = selection { store.revealInFinder(s.project) } }
                .disabled(selection == nil)
            if let proj = selection.flatMap({ store.project($0.project) }), proj.home == nil, proj.workingFolders.count > 1 {
                Menu("Keep Settings in Repo") { HomeFolderMenuItems(project: proj).environmentObject(store) }
            } else {
                Button("Keep Settings in Repo…") { if let s = selection { store.keepSettingsInRepo(s.project) } }
                    .disabled(selection == nil || selection.flatMap { store.project($0.project)?.home } != nil)
            }
            Divider()
            Button("Reload Projects") { store.engine.reload() }
            Button("Open Projects Folder") {
                NSWorkspace.shared.open(URL(fileURLWithPath: store.engine.projectsDir()))
            }
        }
    }
}

/// Stops every session before the process exits, and keeps running when the window closes
/// so sessions survive until the user quits.
final class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        LinkHandling.install()
        LoginItem.enableOnFirstLaunch()
    }

    /// Folders dropped on the Dock icon, or `open -a Switchboard <folder>`.
    func application(_ application: NSApplication, open urls: [URL]) {
        for url in urls where url.hasDirectoryPath || url.pathExtension == "yaml" {
            EngineStore.shared.addProject(from: url)
        }
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { false }

    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        EngineStore.shared.prepareForTermination()
        let engine = EngineStore.shared.engine
        engine.stopControl()
        guard engine.anyAlive() else { return .terminateNow }
        DispatchQueue.global(qos: .userInitiated).async {
            engine.stopEverything()
            DispatchQueue.main.async { sender.reply(toApplicationShouldTerminate: true) }
        }
        return .terminateLater
    }

    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        if !flag {
            sender.windows.first { $0.identifier?.rawValue.contains("main") == true || $0.canBecomeMain }?
                .makeKeyAndOrderFront(nil)
        }
        return true
    }
}
