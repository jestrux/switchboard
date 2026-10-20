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
    /// Which pane (and window) currently shows each command's terminal. A terminal view
    /// can only be in one place at a time; showing it elsewhere moves it there.
    struct PaneOwner: Equatable { let pane: UUID; let window: UUID; let token: UUID }
    @Published private(set) var paneOwners: [CommandRef: PaneOwner] = [:]
    private var windowNames: [UUID: String] = [:]
    private var offeredLeftovers = false
    /// Projects pulled out of the main board into their own window. The main window
    /// hides these; they come back when their window closes.
    @Published private(set) var openProjectWindows: Set<String> = []
    private var terminating = false
    private var reopenedWindows = false
    private static let reopenKey = "reopenProjectWindows"
    private static let runningKey = "runningAtQuit"
    /// What was running when the app last quit, read once at launch before this run's
    /// own statuses start replacing the saved list.
    private var ranLastTime: [CommandRef] = (UserDefaults.standard.stringArray(forKey: "runningAtQuit") ?? []).compactMap {
        let parts = $0.split(separator: "\t", maxSplits: 1).map(String.init)
        return parts.count == 2 ? CommandRef(project: parts[0], address: parts[1]) : nil
    }

    private var listener: Listener?
    private var subscription: UInt64 = 0
    private var control: ControlBridge?

    private init() {
        engine = Engine.openDefault()
        let listener = Listener(store: self)
        self.listener = listener
        subscription = engine.subscribe(listener: listener)
        refreshProjects()
        do { try engine.watch() } catch { NSLog("watch failed: \(error)") }
        let control = ControlBridge(store: self)
        self.control = control
        do { try engine.serveControl(handler: control) } catch { NSLog("control socket: \(error)") }
    }

    // MARK: - The switchboard command

    /// A `switchboard open` request; the main window acts on it.
    struct OpenRequest: Equatable {
        let id = UUID()
        let project: String?
    }
    @Published var openRequest: OpenRequest?

    /// Link the bundled `switchboard` command onto the PATH and its skill into
    /// `~/.claude/skills`, then say what happened.
    func installTools() {
        let contents = Bundle.main.bundleURL.appendingPathComponent("Contents")
        let cli = contents.appendingPathComponent("Helpers/switchboard").path
        let skill = contents.appendingPathComponent("Resources/skills/switchboard").path
        Task { @MainActor in
            let result = await Task.detached { Result { try SwitchboardApp_installTools(cli: cli, skill: skill) } }.value
            let ok = DialogButton(label: "OK", kbd: "↩", style: .primary, value: 0)
            switch result {
            case .success(let got):
                var facts = [DialogFact(icon: "terminal", text: got.commandPath.abbreviatingHome, mono: true)]
                if let path = got.skillPath {
                    facts.append(DialogFact(icon: "sparkles", text: path.abbreviatingHome, mono: true))
                } else if let e = got.skillError {
                    facts.append(DialogFact(icon: "exclamationmark.triangle.fill", text: e, tint: .error))
                }
                if let path = got.claudeMdPath {
                    facts.append(DialogFact(icon: "text.badge.plus", text: path.abbreviatingHome,
                                            detail: got.claudeMdAdded ? "Added /switchboard to your skills" : "Already lists /switchboard",
                                            mono: true))
                } else if let e = got.claudeMdError {
                    facts.append(DialogFact(icon: "exclamationmark.triangle.fill", text: e, tint: .error))
                }
                let folder = (got.commandPath as NSString).deletingLastPathComponent.abbreviatingHome
                _ = await DialogCenter.shared.ask(DialogRequest(
                    kind: .toolsInstalled,
                    title: "The switchboard command is installed",
                    subtitle: "Agents and terminals can now start, stop and read your projects’ commands in this app. Try “switchboard status” in a new terminal.",
                    facts: facts,
                    note: got.onPath
                        ? "Both are links into the app, so they update when Switchboard does."
                        : "\(folder) is not on your PATH yet. Add it in your shell profile, then open a new terminal.",
                    buttons: [ok]))
            case .failure(let error):
                _ = await DialogCenter.shared.ask(DialogRequest(
                    kind: .invalidFolder, title: "Could not install the switchboard command",
                    subtitle: error.readableMessage, buttons: [ok]))
            }
        }
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

    /// Keeps the list of running commands on disk as it changes, so it is right after a
    /// crash too. Frozen once quitting starts: the stops that follow are not the user's.
    private func persistRunning() {
        guard !terminating, ranLastTime.isEmpty else { return }
        let alive = states.filter { $0.value.isAlive }.keys.map { "\($0.project)\t\($0.address)" }.sorted()
        guard alive != UserDefaults.standard.stringArray(forKey: Self.runningKey) ?? [] else { return }
        UserDefaults.standard.set(alive, forKey: Self.runningKey)
    }

    private func persistOpenWindows() {
        guard !terminating else { return }
        UserDefaults.standard.set(Array(openProjectWindows).sorted(), forKey: Self.reopenKey)
    }

    /// `token` identifies the view instance claiming, so a view being replaced during a
    /// layout change cannot release a claim its successor just made.
    func claim(_ ref: CommandRef, pane: UUID, window context: WindowContext, token: UUID) {
        windowNames[context.id] = context.title(store: self)
        let owner = PaneOwner(pane: pane, window: context.id, token: token)
        if paneOwners[ref] != owner { paneOwners[ref] = owner }
    }

    func release(_ ref: CommandRef, token: UUID) {
        if paneOwners[ref]?.token == token { paneOwners[ref] = nil }
    }

    func releaseAll(from context: WindowContext) {
        paneOwners = paneOwners.filter { $0.value.window != context.id }
        windowNames[context.id] = nil
        if case .project(let slug) = context.scope {
            openProjectWindows.remove(slug)
            persistOpenWindows()
        }
    }

    func ownerName(of ref: CommandRef) -> String? {
        paneOwners[ref].flatMap { windowNames[$0.window] }
    }

    // MARK: - Lookups

    func project(_ slug: String) -> ProjectInfo? { projects.first { $0.slug == slug } }

    func command(_ ref: CommandRef) -> CommandInfo? {
        project(ref.project)?.allCommands.first { $0.address == ref.address }
    }

    func state(_ ref: CommandRef) -> CommandState {
        states[ref] ?? .idle
    }

    /// Groups the user has opened, as `project/group`. Groups start collapsed. This is
    /// per-device state like the sidebar width, so it never goes into a project's YAML.
    @Published private(set) var expandedGroups: Set<String> =
        Set(UserDefaults.standard.stringArray(forKey: "expandedGroups") ?? [])

    func isGroupExpanded(project: String, group: String) -> Bool {
        expandedGroups.contains("\(project)/\(group)")
    }

    func setGroup(project: String, group: String, expanded: Bool) {
        let key = "\(project)/\(group)"
        guard expandedGroups.contains(key) != expanded else { return }
        if expanded { expandedGroups.insert(key) } else { expandedGroups.remove(key) }
        UserDefaults.standard.set(expandedGroups.sorted(), forKey: "expandedGroups")
    }

    /// The user's rail order (slugs). Projects not listed yet follow, alphabetically.
    private var projectOrder: [String] = UserDefaults.standard.stringArray(forKey: "projectOrder") ?? []

    private func ordered(_ list: [ProjectInfo]) -> [ProjectInfo] {
        let rank = Dictionary(uniqueKeysWithValues: projectOrder.enumerated().map { ($1, $0) })
        return list.enumerated().sorted { a, b in
            let ra = rank[a.element.slug] ?? (Int.max / 2 + a.offset), rb = rank[b.element.slug] ?? (Int.max / 2 + b.offset)
            return ra < rb
        }.map(\.element)
    }

    /// Move a project to the slot `target` occupies in the rail.
    func moveProject(_ slug: String, toSlotOf target: String) {
        var order = projects.map(\.slug)
        guard slug != target, let from = order.firstIndex(of: slug), let to = order.firstIndex(of: target) else { return }
        order.remove(at: from)
        order.insert(slug, at: min(to, order.count))
        projectOrder = order
        UserDefaults.standard.set(order, forKey: "projectOrder")
        projects = ordered(projects)
    }

    /// Move a group to the slot another group of the same project occupies.
    func moveGroup(project slug: String, _ id: String, toSlotOf target: String) {
        guard id != target, let p = project(slug), let to = p.groups.firstIndex(where: { $0.id == target }) else { return }
        try? engine.moveGroup(project: slug, group: id, toIndex: UInt32(to))
        refreshProjects()
    }

    /// Move a command to the slot a sibling occupies (same group, or both ungrouped).
    @discardableResult
    func moveCommand(_ ref: CommandRef, toSlotOf target: CommandRef) -> Bool {
        guard ref != target, ref.project == target.project, let p = project(ref.project),
              let a = command(ref), let b = command(target), a.groupId == b.groupId, !a.isTerminal, !b.isTerminal else { return false }
        let siblings = (a.groupId.flatMap { g in p.groups.first { $0.id == g }?.commands } ?? p.commands).filter { !$0.isTerminal }
        guard let to = siblings.firstIndex(where: { $0.address == target.address }) else { return false }
        try? engine.moveCommand(project: ref.project, address: ref.address, toIndex: UInt32(to))
        refreshProjects()
        return true
    }

    func setGroupIcon(project slug: String, group: String, icon: String?) {
        try? engine.setGroupIcon(project: slug, group: group, icon: icon)
        refreshProjects()
    }

    func refreshProjects() {
        projects = ordered(engine.projects())
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

    func crashedCount(project: String) -> Int {
        guard let p = self.project(project) else { return 0 }
        return p.allCommands.filter { state(CommandRef(project: project, address: $0.address)).status == .crashed }.count
    }

    var totalRunning: Int { states.values.filter(\.isAlive).count }
    var totalCrashed: Int { states.values.filter { $0.status == .crashed }.count }

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

    func deleteProject(_ slug: String, window: UUID? = nil) {
        guard let p = project(slug) else { return }
        Task { @MainActor in
            let live = p.allCommands.filter { state(CommandRef(project: slug, address: $0.address)).isAlive }
            let facts = live.map { c in
                DialogFact(icon: "circle.fill", text: c.displayName, detail: nil, tint: .running)
            }
            let choice = await DialogCenter.shared.ask(DialogRequest(
                kind: .deleteProject,
                title: "Delete “\(p.name)”?",
                subtitle: live.isEmpty
                    ? "Its folder moves to the Trash. Put it back from the Trash and it returns to All Projects."
                    : "Its \(live.count) running command\(live.count == 1 ? "" : "s") stop and its folder moves to the Trash. Put it back from the Trash and it returns to All Projects.",
                facts: facts,
                buttons: [DialogButton(label: "Cancel", kbd: "↩", style: .defaultSecondary, value: 0),
                          DialogButton(label: "Delete Project", style: .danger, value: 1)],
                window: window))
            guard choice == 1 else { return }
            try? engine.stopAll(project: slug, group: nil)
            do {
                try FileManager.default.trashItem(at: URL(fileURLWithPath: p.dir), resultingItemURL: nil)
                engine.reload()
            } catch {
                try? engine.deleteProject(slug: slug)
            }
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
    func addProject(from url: URL, window: UUID? = nil) -> Bool {
        Task { @MainActor in await addProjectAsync(from: url, window: window) }
        return true
    }

    func addProjectAsync(from url: URL, window: UUID? = nil) async {
        let source = url.path
        // A repo folder with .switchboard/project.yaml (and no config.yaml of its own)
        // sets the project up from the repo files.
        let seeds = engine.discoverSeeds(folder: source)
        if !seeds.isEmpty, !FileManager.default.fileExists(atPath: url.appendingPathComponent("config.yaml").path) {
            await importSeed(seeds, window: window)
            return
        }
        // A Solo project: convert its solo.yml and continue from there.
        if !FileManager.default.fileExists(atPath: url.appendingPathComponent("config.yaml").path),
           let solo = engine.previewSolo(folder: source) {
            await importSolo(solo, folder: source, window: window)
            return
        }
        let preview: InstallPreview
        do {
            preview = try engine.previewInstall(source: source)
        } catch {
            let choice = await DialogCenter.shared.ask(DialogRequest(
                kind: .invalidFolder,
                title: "This folder isn’t a Switchboard project",
                subtitle: "Nothing was copied. A project folder needs a config.yaml at its top level.",
                facts: [DialogFact(icon: "folder", text: source.abbreviatingHome, mono: true),
                        DialogFact(icon: "exclamationmark.triangle.fill", text: error.readableMessage, tint: .error)],
                alt: DialogButton(label: "Reveal in Finder", value: 2),
                buttons: [DialogButton(label: "OK", kbd: "↩", style: .primary, value: 0)],
                window: window))
            if choice == 2 { NSWorkspace.shared.activateFileViewerSelecting([url]) }
            return
        }
        var policy = ConflictPolicy.fail
        if preview.exists {
            let running = runningCount(project: preview.slug)
            let choice = await DialogCenter.shared.ask(DialogRequest(
                kind: .nameConflict,
                title: "A project named “\(preview.name)” is already here",
                subtitle: "You’re adding another folder with the same name. Pick what should happen.",
                facts: [
                    DialogFact(text: "Keep Both", detail: "Adds this one as “\(preview.slug)-2”. Nothing changes for the existing \(preview.name)."),
                    DialogFact(text: "Replace", detail: running > 0
                               ? "Stops \(preview.name)’s \(running) running command\(running == 1 ? "" : "s") and moves its folder to the Trash."
                               : "Moves the existing folder to the Trash and installs this one in its place.", tint: .crashed),
                ],
                buttons: [DialogButton(label: "Cancel", kbd: "esc", value: 0),
                          DialogButton(label: "Replace", style: .dangerOutline, value: 1),
                          DialogButton(label: "Keep Both", kbd: "↩", style: .primary, value: 2)],
                window: window))
            switch choice {
            case 1: policy = .replace
            case 2: policy = .keepBoth
            default: return
            }
            if policy == .replace { try? engine.stopAll(project: preview.slug, group: nil) }
        }
        do {
            let slug = try engine.install(source: source, conflict: policy)
            refreshProjects()
            lastInstalled = slug
        } catch {
            _ = await DialogCenter.shared.ask(DialogRequest(
                kind: .invalidFolder, title: "Could not add project", subtitle: error.readableMessage,
                buttons: [DialogButton(label: "OK", kbd: "↩", style: .primary, value: 0)], window: window))
        }
    }

    /// Confirm and install a project from repo seeds (newest first).
    private func importSeed(_ seeds: [SeedInfo], window: UUID?) async {
        var index = 0
        while true {
            let seed = seeds[index]
            var facts = seed.members.map { m in
                DialogFact(icon: m.exists ? "checkmark.circle.fill" : "exclamationmark.triangle.fill",
                           text: "\(m.group)  ·  \(Fmt.shortFolder(m.folder))",
                           detail: m.exists ? (m.commandCount == 0 ? "no commands.yaml yet" : "\(m.commandCount) command\(m.commandCount == 1 ? "" : "s")")
                                            : "folder not found on this Mac",
                           tint: m.exists ? .running : .error, iconOnlyTint: true)
            }
            if facts.isEmpty { facts = [DialogFact(icon: "doc.text", text: "No from: groups; commands are written in project.yaml")] }
            let other = seeds.count > 1 ? seeds[(index + 1) % seeds.count] : nil
            let when = Date(timeIntervalSince1970: seed.updatedAt).formatted(date: .abbreviated, time: .shortened)
            var buttons = [DialogButton(label: "Cancel", kbd: "esc", value: 0)]
            if seed.exists {
                buttons.append(DialogButton(label: "Replace", style: .dangerOutline, value: 1))
                buttons.append(DialogButton(label: "Keep Both", kbd: "↩", style: .primary, value: 2))
            } else {
                buttons.append(DialogButton(label: "Set Up Project", kbd: "↩", style: .primary, value: 3))
            }
            let choice = await DialogCenter.shared.ask(DialogRequest(
                kind: .nameConflict,
                title: seed.exists ? "“\(seed.name)” is already here" : "Set up “\(seed.name)” from this repo?",
                subtitle: "Settings come from \(Fmt.shortFolder(seed.home.abbreviatingHome))/.switchboard/project.yaml (updated \(when)). Each folder supplies its own commands.",
                facts: facts,
                note: seeds.count > 1 ? "\(seeds.count) copies of project.yaml were found across these folders; this is \(index == 0 ? "the newest" : "an older one")." : nil,
                alt: other.map { DialogButton(label: "Use the copy in \(URL(fileURLWithPath: $0.home).lastPathComponent)", value: 9) },
                buttons: buttons,
                window: window))
            if choice == 9 { index = (index + 1) % seeds.count; continue }
            let policy: ConflictPolicy
            switch choice {
            case 1: policy = .replace; try? engine.stopAll(project: seed.slug, group: nil)
            case 2: policy = .keepBoth
            case 3: policy = .fail
            default: return
            }
            do {
                let slug = try engine.installSeed(home: seed.home, conflict: policy)
                refreshProjects()
                lastInstalled = slug
            } catch {
                _ = await DialogCenter.shared.ask(DialogRequest(
                    kind: .invalidFolder, title: "Could not set up the project", subtitle: error.readableMessage,
                    buttons: [DialogButton(label: "OK", kbd: "↩", style: .primary, value: 0)], window: window))
            }
            return
        }
    }

    /// Confirm and install a project converted from Solo.
    private func importSolo(_ solo: SoloPreview, folder: String, window: UUID?) async {
        let count = solo.groups.reduce(0) { $0 + $1.commands.count }
        let facts = solo.groups.map { g in
            DialogFact(icon: "folder", text: "\(g.name)  ·  \(Fmt.shortFolder(g.folder))", detail: g.commands.joined(separator: ", "))
        }
        var buttons = [DialogButton(label: "Cancel", kbd: "esc", value: 0)]
        if solo.exists {
            buttons.append(DialogButton(label: "Replace", style: .dangerOutline, value: 1))
            buttons.append(DialogButton(label: "Keep Both", kbd: "↩", style: .primary, value: 2))
        } else {
            buttons.append(DialogButton(label: "Convert", kbd: "↩", style: .primary, value: 3))
        }
        let choice = await DialogCenter.shared.ask(DialogRequest(
            kind: .nameConflict,
            title: solo.exists ? "“\(solo.name)” is already here" : "Convert “\(solo.name)” from Solo?",
            subtitle: "Found solo.yml. Its \(count) process\(count == 1 ? "" : "es") become commands, grouped by folder. The Solo file is left as it is.",
            facts: facts,
            note: solo.notes.isEmpty ? "You can rename and reorder groups, and add a theme, afterwards." : solo.notes.joined(separator: "\n"),
            buttons: buttons, window: window))
        let policy: ConflictPolicy
        switch choice {
        case 1: policy = .replace; try? engine.stopAll(project: solo.slug, group: nil)
        case 2: policy = .keepBoth
        case 3: policy = .fail
        default: return
        }
        do {
            let slug = try engine.installSolo(folder: folder, conflict: policy)
            refreshProjects()
            lastInstalled = slug
        } catch {
            _ = await DialogCenter.shared.ask(DialogRequest(
                kind: .invalidFolder, title: "Could not convert the project", subtitle: error.readableMessage,
                buttons: [DialogButton(label: "OK", kbd: "↩", style: .primary, value: 0)], window: window))
        }
    }

    // MARK: - Settings in the repo

    /// Keep the project's settings in `folder/.switchboard/project.yaml` from now on.
    func keepSettingsInRepo(_ slug: String, folder: String? = nil) {
        var target = folder
        if target == nil {
            let panel = NSOpenPanel()
            panel.title = "Keep Settings in Repo"
            panel.message = "Choose the repo folder that should hold this project's .switchboard/project.yaml."
            panel.canChooseDirectories = true
            panel.canChooseFiles = false
            panel.prompt = "Keep Here"
            guard panel.runModal() == .OK, let url = panel.url else { return }
            target = url.path
        }
        guard let target else { return }
        do {
            try engine.adoptHome(slug: slug, folder: (target as NSString).expandingTildeInPath)
            refreshProjects()
        } catch {
            NSLog("adopt failed: \(error.readableMessage)")
        }
    }

    func applyRepoSettings(_ slug: String) {
        try? engine.applyRepoSettings(slug: slug)
        refreshProjects()
    }

    // MARK: - Ad-hoc terminals

    /// Open a terminal in a group's folder (or ungrouped) and return its ref.
    @discardableResult
    func newTerminal(project: String, group: String?) -> CommandRef? {
        guard let address = try? engine.addTerminal(project: project, group: group) else { return nil }
        refreshProjects()
        return CommandRef(project: project, address: address)
    }

    func closeTerminal(_ ref: CommandRef) {
        DispatchQueue.global(qos: .userInitiated).async {
            try? self.engine.removeTerminal(project: ref.project, address: ref.address)
            Task { @MainActor in self.refreshProjects() }
        }
    }

    func renameTerminal(_ ref: CommandRef, window: UUID? = nil) {
        guard let cmd = command(ref) else { return }
        Task { @MainActor in
            let choice = await DialogCenter.shared.ask(DialogRequest(
                kind: .nameConflict, title: "Rename terminal", subtitle: "The name shows in the sidebar and pane bar. It stays on this Mac.",
                buttons: [DialogButton(label: "Cancel", kbd: "esc", value: 0), DialogButton(label: "Rename", kbd: "↩", style: .primary, value: 1)],
                inputs: [DialogInput(label: "Name", value: cmd.name)], window: window))
            let name = DialogCenter.shared.inputValues.first?.trimmingCharacters(in: .whitespaces) ?? ""
            guard choice == 1, !name.isEmpty else { return }
            try? engine.renameTerminal(project: ref.project, address: ref.address, name: name)
            refreshProjects()
        }
    }

    /// Turn a terminal into a configured command in its group. Returns via `onDone`.
    func saveTerminalAsCommand(_ ref: CommandRef, window: UUID? = nil, onDone: @escaping (CommandRef) -> Void = { _ in }) {
        guard let cmd = command(ref), let project = project(ref.project) else { return }
        let group = cmd.groupId.flatMap { g in project.groups.first { $0.id == g } }
        let dest = group?.from != nil ? "\(group!.from!.abbreviatingHome)/.switchboard/commands.yaml" : "this project's config"
        Task { @MainActor in
            let choice = await DialogCenter.shared.ask(DialogRequest(
                kind: .nameConflict, title: "Save as command",
                subtitle: "Adds a command to \(group?.name ?? "the project") in \(dest) and closes this terminal.",
                buttons: [DialogButton(label: "Cancel", kbd: "esc", value: 0), DialogButton(label: "Save", kbd: "↩", style: .primary, value: 1)],
                inputs: [DialogInput(label: "Name", placeholder: "dev", value: cmd.name == "Terminal" ? "" : cmd.name),
                         DialogInput(label: "Command", placeholder: "npm run dev", mono: true)],
                window: window))
            let v = DialogCenter.shared.inputValues
            guard choice == 1, v.count == 2 else { return }
            let name = v[0].trimmingCharacters(in: .whitespaces), run = v[1].trimmingCharacters(in: .whitespaces)
            guard !name.isEmpty, !run.isEmpty else { return }
            do {
                let address = try engine.promoteTerminal(project: ref.project, address: ref.address, id: name, name: name, run: run)
                refreshProjects()
                onDone(CommandRef(project: ref.project, address: address))
            } catch {
                _ = await DialogCenter.shared.ask(DialogRequest(
                    kind: .invalidFolder, title: "Could not save the command", subtitle: error.readableMessage,
                    buttons: [DialogButton(label: "OK", kbd: "↩", style: .primary, value: 0)], window: window))
            }
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
        guard !leftovers.isEmpty else {
            Task { @MainActor in
                await offerToRunAgain()
                await offerToInstallTools()
            }
            return
        }
        Task { @MainActor in
            let n = leftovers.count
            let facts = leftovers.map { l -> DialogFact in
                let name = project(l.project).map { p in
                    (p.allCommands.first { $0.address == l.address }?.displayName ?? l.address) + " · " + p.name
                } ?? "\(l.project) / \(l.address)"
                return DialogFact(icon: "circle.fill", text: name, detail: "pid \(l.pid)", tint: .running)
            }
            let choice = await DialogCenter.shared.ask(DialogRequest(
                kind: .leftoverSessions,
                title: n == 1 ? "1 command is still running from last time" : "\(n) commands are still running from last time",
                subtitle: "Switchboard didn’t quit cleanly, so \(n == 1 ? "this process" : "these processes") outlived it.",
                facts: facts,
                note: "Left running, \(n == 1 ? "it keeps its port" : "they keep their ports") but Switchboard can’t show \(n == 1 ? "its" : "their") output. Start the command again later to replace \(n == 1 ? "it" : "them").",
                buttons: [DialogButton(label: "Leave Running", value: 0),
                          DialogButton(label: n == 1 ? "Stop It" : "Stop Them", kbd: "↩", style: .primary, value: 1)]))
            if choice == 1 { _ = engine.killLeftovers() }
            await offerToRunAgain()
            await offerToInstallTools()
        }
    }

    // MARK: - Running again after a relaunch

    /// Quitting stops every command, so on the next launch offer to start the ones that
    /// were running. Asked once per launch; either answer clears the offer.
    private func offerToRunAgain() async {
        let refs = ranLastTime.filter { command($0) != nil && !state($0).isAlive }
        // The saved list stays put until the question is answered, so quitting with the
        // dialog still open asks again next time.
        defer { ranLastTime = []; persistRunning() }
        guard !refs.isEmpty else { return }
        let n = refs.count
        let facts = refs.map { ref -> DialogFact in
            let name = (command(ref)?.displayName ?? ref.address) + " · " + (project(ref.project)?.name ?? ref.project)
            return DialogFact(icon: "play.fill", text: name, iconOnlyTint: true)
        }
        let choice = await DialogCenter.shared.ask(DialogRequest(
            kind: .runAgain,
            title: n == 1 ? "Start the command that was running?" : "Start the \(n) commands that were running?",
            subtitle: "\(n == 1 ? "It was" : "They were") running when Switchboard last quit.",
            facts: facts,
            note: "Each one starts fresh. A session inside a command, such as a Claude conversation, does not come back by itself.",
            buttons: [DialogButton(label: "Not Now", value: 0),
                      DialogButton(label: n == 1 ? "Start It" : "Start Them", kbd: "⌘↩", style: .primary, value: 1)]))
        if choice == 1 { refs.forEach(start) }
    }

    // MARK: - Offering the switchboard command

    private static let offeredToolsKey = "offeredTools"

    /// Once, on a launch where the command or skill is missing, offer to install them.
    /// Either answer is remembered; the menu item stays for later.
    private func offerToInstallTools() async {
        guard !UserDefaults.standard.bool(forKey: Self.offeredToolsKey) else { return }
        let installed = await Task.detached { toolsInstalled() }.value
        guard !installed else { return }
        let choice = await DialogCenter.shared.ask(DialogRequest(
            kind: .offerTools,
            title: "Let coding agents use Switchboard?",
            subtitle: "This installs the switchboard command and a Claude Code skill, so agents can start your projects’ commands here, wait for them and read their output.",
            facts: [DialogFact(icon: "terminal", text: "switchboard", detail: "linked onto your PATH", mono: true),
                    DialogFact(icon: "sparkles", text: "~/.claude/skills/switchboard", detail: "and listed in ~/.claude/CLAUDE.md if you have one", mono: true)],
            note: "You can do this later from Switchboard › Install switchboard Command….",
            buttons: [DialogButton(label: "Not Now", value: 0),
                      DialogButton(label: "Install", kbd: "⌘↩", style: .primary, value: 1)]))
        // -1: another dialog replaced this one before it was answered; ask next launch.
        guard choice >= 0 else { return }
        UserDefaults.standard.set(true, forKey: Self.offeredToolsKey)
        if choice == 1 { installTools() }
    }

    // MARK: - Engine events (arrive on engine threads)

    nonisolated fileprivate func handleProjectsChanged() {
        Task { @MainActor in self.refreshProjects() }
    }

    nonisolated fileprivate func handleStatus(_ ref: CommandRef, _ state: CommandState) {
        Task { @MainActor in
            self.states[ref] = state
            self.persistRunning()
        }
    }
}

/// Calls the free FFI function from a context where `installTools` is the store's method.
private func SwitchboardApp_installTools(cli: String, skill: String) throws -> ToolsInstalled {
    try installTools(cliPath: cli, skillDir: skill)
}

/// Requests from the `switchboard` command that need the windows.
private final class ControlBridge: ControlHandler, @unchecked Sendable {
    weak var store: EngineStore?
    init(store: EngineStore) { self.store = store }

    func openProject(project: String?) {
        DispatchQueue.main.async { [weak self] in
            NSApp.activate(ignoringOtherApps: true)
            self?.store?.openRequest = EngineStore.OpenRequest(project: project)
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
