import AppKit
import Combine
import Foundation

/// The app's view of the engine: projects, live command states and selection.
/// All published properties change on the main thread.
@MainActor
final class EngineStore: ObservableObject {
    static let shared = EngineStore()

    let engine: Engine
    @Published private(set) var projects: [ProjectInfo] = []
    @Published private(set) var states: [CommandRef: CommandState] = [:]
    /// Last known terminal title per command (OSC 0/2), shown in the footer.
    @Published var terminalTitles: [CommandRef: String] = [:]
    /// Which window currently shows each command's terminal. A terminal view can only be
    /// in one window at a time; selecting it elsewhere moves it there.
    @Published private(set) var paneOwners: [CommandRef: UUID] = [:]
    private var windowNames: [UUID: String] = [:]
    private var offeredLeftovers = false
    /// Projects pulled out of the main board into their own window. The main window
    /// hides these; they come back when their window closes.
    @Published private(set) var openProjectWindows: Set<String> = []
    private var terminating = false
    private var reopenedWindows = false
    private static let reopenKey = "reopenProjectWindows"

    private var listener: Listener?
    private var subscription: UInt64 = 0

    private init() {
        engine = Engine.openDefault()
        let listener = Listener(store: self)
        self.listener = listener
        subscription = engine.subscribe(listener: listener)
        refreshProjects()
        do { try engine.watch() } catch { NSLog("watch failed: \(error)") }
    }

    // MARK: - Pane ownership (one terminal view per command, one window at a time)

    func registerWindow(_ context: WindowContext) {
        windowNames[context.id] = context.title(store: self)
        if case .project(let slug) = context.scope {
            openProjectWindows.insert(slug)
            persistOpenWindows()
        }
    }

    // MARK: - Reopening project windows across launches

    /// Called by the main window once: reopen the project windows from the last run.
    func projectWindowsToReopen() -> [String] {
        guard !reopenedWindows else { return [] }
        reopenedWindows = true
        let saved = UserDefaults.standard.stringArray(forKey: Self.reopenKey) ?? []
        return saved.filter { project($0) != nil }
    }

    /// Snapshot the open set before windows start closing at quit.
    func prepareForTermination() {
        terminating = true
        UserDefaults.standard.set(Array(openProjectWindows).sorted(), forKey: Self.reopenKey)
    }

    private func persistOpenWindows() {
        guard !terminating else { return }
        UserDefaults.standard.set(Array(openProjectWindows).sorted(), forKey: Self.reopenKey)
    }

    func claim(_ ref: CommandRef, for context: WindowContext) {
        windowNames[context.id] = context.title(store: self)
        if paneOwners[ref] != context.id { paneOwners[ref] = context.id }
    }

    func release(_ ref: CommandRef, from context: WindowContext) {
        if paneOwners[ref] == context.id { paneOwners[ref] = nil }
    }

    func releaseAll(from context: WindowContext) {
        paneOwners = paneOwners.filter { $0.value != context.id }
        windowNames[context.id] = nil
        if case .project(let slug) = context.scope {
            openProjectWindows.remove(slug)
            persistOpenWindows()
        }
    }

    func ownerName(of ref: CommandRef) -> String? {
        paneOwners[ref].flatMap { windowNames[$0] }
    }

    // MARK: - Lookups

    func project(_ slug: String) -> ProjectInfo? { projects.first { $0.slug == slug } }

    func command(_ ref: CommandRef) -> CommandInfo? {
        project(ref.project)?.allCommands.first { $0.address == ref.address }
    }

    func state(_ ref: CommandRef) -> CommandState {
        states[ref] ?? CommandState(status: .stopped, pid: nil, exitCode: nil, signal: nil,
                                    startedAt: nil, endedAt: nil, message: nil, needsRestart: false)
    }

    func refreshProjects() {
        projects = engine.projects()
        // Seed states from the snapshot so dots are right after a reload.
        var merged = states
        for p in projects {
            for c in p.allCommands {
                let ref = CommandRef(project: p.slug, address: c.address)
                if merged[ref] == nil || c.state.isAlive || merged[ref]?.isAlive == false {
                    merged[ref] = c.state
                }
            }
        }
        states = merged
    }

    // MARK: - Actions

    func start(_ ref: CommandRef) {
        do { try engine.start(project: ref.project, address: ref.address) } catch {
            // Spawn errors land in the command's state; nothing else to do.
        }
    }
    func stop(_ ref: CommandRef) { engine.stop(project: ref.project, address: ref.address) }
    func restart(_ ref: CommandRef) { engine.restart(project: ref.project, address: ref.address) }

    func startAll(project: String, group: String? = nil) {
        try? engine.startAll(project: project, group: group)
    }
    func stopAll(project: String, group: String? = nil) {
        try? engine.stopAll(project: project, group: group)
    }
    func restartAll(project: String, group: String? = nil) {
        try? engine.restartAll(project: project, group: group)
    }

    func runningCount(project: String, group: String? = nil) -> Int {
        guard let p = self.project(project) else { return 0 }
        let cmds = group.map { g in p.groups.first { $0.id == g }?.commands ?? [] } ?? p.allCommands
        return cmds.filter { state(CommandRef(project: project, address: $0.address)).isAlive }.count
    }

    // MARK: - Files

    func editConfig(_ slug: String) {
        guard let p = project(slug) else { return }
        openInEditor(path: p.configPath)
    }

    /// `open -a "Visual Studio Code" <path>`, falling back to the default app for .yaml.
    func openInEditor(path: String) {
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: "/usr/bin/open")
        proc.arguments = ["-a", "Visual Studio Code", path]
        proc.standardError = FileHandle.nullDevice
        proc.standardOutput = FileHandle.nullDevice
        do {
            try proc.run()
            proc.waitUntilExit()
            if proc.terminationStatus == 0 { return }
        } catch {}
        NSWorkspace.shared.open(URL(fileURLWithPath: path))
    }

    func revealInFinder(_ slug: String) {
        guard let p = project(slug) else { return }
        NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: p.configPath)])
    }

    func revealFolder(_ path: String) {
        NSWorkspace.shared.selectFile(nil, inFileViewerRootedAtPath: path)
    }

    func deleteProject(_ slug: String) {
        guard let p = project(slug) else { return }
        let alert = NSAlert()
        alert.messageText = "Delete “\(p.name)”?"
        alert.informativeText = "This stops its commands and moves the project folder\n\(p.dir.abbreviatingHome)\nto the Trash. The command folders themselves are not touched."
        alert.alertStyle = .warning
        alert.addButton(withTitle: "Delete")
        alert.addButton(withTitle: "Cancel")
        guard alert.runModal() == .alertFirstButtonReturn else { return }
        try? engine.stopAll(project: slug, group: nil)
        // Trash rather than rm so a mistake is recoverable; the watcher reloads.
        do {
            try FileManager.default.trashItem(at: URL(fileURLWithPath: p.dir), resultingItemURL: nil)
            engine.reload()
        } catch {
            try? engine.deleteProject(slug: slug)
        }
    }

    // MARK: - Add project

    func addProjectViaPanel() {
        let panel = NSOpenPanel()
        panel.title = "Add Project"
        panel.message = "Choose a project folder containing config.yaml (or the file itself)."
        panel.canChooseDirectories = true
        panel.canChooseFiles = true
        panel.allowsMultipleSelection = false
        panel.allowedContentTypes = [.yaml, .folder]
        panel.prompt = "Add"
        guard panel.runModal() == .OK, let url = panel.url else { return }
        addProject(from: url)
    }

    /// Validate, resolve name conflicts, copy into the Projects directory and select it.
    @discardableResult
    func addProject(from url: URL) -> Bool {
        let source = url.path
        let preview: InstallPreview
        do {
            preview = try engine.previewInstall(source: source)
        } catch {
            let alert = NSAlert()
            alert.messageText = "This folder is not a valid Switchboard project"
            alert.informativeText = "\(source.abbreviatingHome)\n\n\(error.readableMessage)\n\nNothing was copied."
            alert.alertStyle = .warning
            alert.addButton(withTitle: "OK")
            alert.runModal()
            return false
        }
        var policy = ConflictPolicy.fail
        if preview.exists {
            let alert = NSAlert()
            alert.messageText = "A project named “\(preview.name)” already exists"
            alert.informativeText = "Replace the existing “\(preview.slug)” folder, or keep both and add this one as “\(preview.slug)-2”?"
            alert.addButton(withTitle: "Replace")
            alert.addButton(withTitle: "Keep Both")
            alert.addButton(withTitle: "Cancel")
            switch alert.runModal() {
            case .alertFirstButtonReturn: policy = .replace
            case .alertSecondButtonReturn: policy = .keepBoth
            default: return false
            }
            if policy == .replace { try? engine.stopAll(project: preview.slug, group: nil) }
        }
        do {
            let slug = try engine.install(source: source, conflict: policy)
            refreshProjects()
            lastInstalled = slug
            return true
        } catch {
            let alert = NSAlert()
            alert.messageText = "Could not add project"
            alert.informativeText = error.readableMessage
            alert.alertStyle = .critical
            alert.runModal()
            return false
        }
    }

    /// Slug of the most recently added project, so the adding window can select it.
    @Published var lastInstalled: String?

    /// Debug helper: copy `samples/kinpay` from this repo into the Projects directory.
    func installSampleProject() {
        // app/Switchboard/EngineStore.swift -> repo root
        let here = URL(fileURLWithPath: #filePath)
        let root = here.deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        addProject(from: root.appendingPathComponent("samples/kinpay"))
    }

    // MARK: - Leftovers

    func offerToKillLeftovers() {
        guard !offeredLeftovers else { return }
        offeredLeftovers = true
        let leftovers = engine.leftoverSessions()
        guard !leftovers.isEmpty else { return }
        let alert = NSAlert()
        alert.messageText = "Switchboard left \(leftovers.count) session\(leftovers.count == 1 ? "" : "s") running"
        alert.informativeText = "A previous run did not quit cleanly. These processes are still alive:\n\n"
            + leftovers.map { "• \($0.project) / \($0.address) (pid \($0.pid))" }.joined(separator: "\n")
        alert.addButton(withTitle: "Stop Them")
        alert.addButton(withTitle: "Leave Running")
        if alert.runModal() == .alertFirstButtonReturn {
            _ = engine.killLeftovers()
        }
    }

    // MARK: - Engine events (arrive on engine threads)

    nonisolated fileprivate func handleProjectsChanged() {
        Task { @MainActor in self.refreshProjects() }
    }

    nonisolated fileprivate func handleStatus(_ ref: CommandRef, _ state: CommandState) {
        Task { @MainActor in
            self.states[ref] = state
        }
    }
}

/// Bridges engine callbacks to the store. Output bytes bypass the store and go straight
/// to the terminal registry, which is thread-safe.
private final class Listener: EngineListener, @unchecked Sendable {
    weak var store: EngineStore?
    init(store: EngineStore) { self.store = store }

    func onProjectsChanged() {
        store?.handleProjectsChanged()
    }

    func onStatus(project: String, address: String, state: CommandState) {
        let ref = CommandRef(project: project, address: address)
        if state.status == .starting && state.pid == nil {
            TerminalRegistry.shared.noteStart(ref)
        }
        store?.handleStatus(ref, state)
    }

    func onOutput(project: String, address: String, bytes: Data) {
        TerminalRegistry.shared.feed(CommandRef(project: project, address: address), bytes)
    }
}
