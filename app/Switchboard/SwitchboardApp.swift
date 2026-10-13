import SwiftUI

@main
struct SwitchboardApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate
    @StateObject private var store = EngineStore.shared
    @Environment(\.openWindow) private var openWindow

    @FocusedValue(\.windowFocus) private var focus

    private var selection: CommandRef? { focus?.selection }
    private var selectedIsAlive: Bool { selection.map { store.state($0).isAlive } ?? false }

    var body: some Scene {
        Window("Switchboard", id: "main") {
            ContentView(scope: .all)
                .environmentObject(store)
        }
        .defaultSize(width: 1180, height: 720)
        .commands { appCommands }

        // One window per project; opening the same project again focuses its window.
        WindowGroup("Project", id: "project", for: String.self) { $slug in
            if let slug {
                ContentView(scope: .project(slug))
                    .environmentObject(store)
            }
        }
        .defaultSize(width: 980, height: 680)
    }

    @CommandsBuilder
    private var appCommands: some Commands {
        CommandGroup(replacing: .newItem) {
            Button("Add Project…") { store.addProjectViaPanel() }
                .keyboardShortcut("o", modifiers: [.command])
            #if DEBUG
            Button("Install Sample Project (Kinpay)") { store.installSampleProject() }
            #endif
        }
        CommandMenu("Command") {
            Button("Start") { if let s = selection { store.start(s) } }
                .disabled(selection == nil || selectedIsAlive)
            Button("Stop") { if let s = selection { store.stop(s) } }
                .keyboardShortcut(".", modifiers: [.command])
                .disabled(!selectedIsAlive)
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
                .disabled(!store.engine.anyAlive())
        }
        CommandMenu("Project") {
            Button("Open in New Window") {
                if let s = selection { openWindow(id: "project", value: s.project) }
            }
            .keyboardShortcut("n", modifiers: [.command, .option])
            .disabled(selection == nil)
            Button("Show All Projects") { openWindow(id: "main") }
                .keyboardShortcut("0", modifiers: [.command])
            Divider()
            Button("Edit Config") { if let s = selection { store.editConfig(s.project) } }
                .keyboardShortcut("e", modifiers: [.command])
                .disabled(selection == nil)
            Button("Reveal in Finder") { if let s = selection { store.revealInFinder(s.project) } }
                .disabled(selection == nil)
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
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { false }

    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        EngineStore.shared.prepareForTermination()
        let engine = EngineStore.shared.engine
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
