import SwiftUI

@main
struct SwitchboardApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate
    @StateObject private var store = EngineStore.shared

    var body: some Scene {
        Window("Switchboard", id: "main") {
            ContentView()
                .environmentObject(store)
        }
        .defaultSize(width: 1180, height: 720)
        .commands {
            CommandGroup(replacing: .newItem) {
                Button("Add Project…") { store.addProjectViaPanel() }
                    .keyboardShortcut("o", modifiers: [.command])
                #if DEBUG
                Button("Install Sample Project (Kinpay)") { store.installSampleProject() }
                #endif
            }
            CommandMenu("Command") {
                Button("Start") { if let s = store.selection { store.start(s) } }
                    .disabled(store.selection == nil || store.state(store.selection!).isAlive)
                Button("Stop") { if let s = store.selection { store.stop(s) } }
                    .keyboardShortcut(".", modifiers: [.command])
                    .disabled(store.selection == nil || !store.state(store.selection!).isAlive)
                Button("Restart") { if let s = store.selection { store.restart(s) } }
                    .keyboardShortcut("r", modifiers: [.command])
                    .disabled(store.selection == nil)
                Divider()
                Button("Clear Terminal") {
                    if let s = store.selection { TerminalRegistry.shared.existingHost(for: s)?.clear() }
                }
                .keyboardShortcut("k", modifiers: [.command])
                .disabled(store.selection == nil)
                Divider()
                Button("Start All in Project") { if let s = store.selection { store.startAll(project: s.project) } }
                    .disabled(store.selection == nil)
                Button("Stop All in Project") { if let s = store.selection { store.stopAll(project: s.project) } }
                    .disabled(store.selection == nil)
                Button("Stop Everything") { store.engine.stopEverything() }
                    .disabled(!store.engine.anyAlive())
            }
            CommandMenu("Project") {
                Button("Edit Config") { if let s = store.selection { store.editConfig(s.project) } }
                    .keyboardShortcut("e", modifiers: [.command])
                    .disabled(store.selection == nil)
                Button("Reveal in Finder") { if let s = store.selection { store.revealInFinder(s.project) } }
                    .disabled(store.selection == nil)
                Divider()
                Button("Reload Projects") { store.engine.reload() }
                Button("Open Projects Folder") {
                    NSWorkspace.shared.open(URL(fileURLWithPath: store.engine.projectsDir()))
                }
            }
        }
    }
}

/// Stops every session before the process exits, and keeps running when the window closes
/// so sessions survive until the user quits.
final class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { false }

    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
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
