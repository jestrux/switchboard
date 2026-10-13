import SwiftUI

/// One window: rail + sidebar (board) or project sidebar, beside the pane area.
/// Resolves the window's look: global settings, overridden by the project's config in
/// its own window.
struct ContentView: View {
    @EnvironmentObject var store: EngineStore
    @ObservedObject private var dialogs = DialogCenter.shared
    @Environment(\.openWindow) private var openWindow
    @Environment(\.dismissWindow) private var dismissWindow
    @AppStorage(ThemeSettings.themeKey) private var themeID: SBThemeID = .graphite
    @AppStorage(ThemeSettings.appearanceKey) private var appearance: SBAppearance = .system
    @StateObject private var context: WindowContext

    init(scope: WindowScope) {
        _context = StateObject(wrappedValue: WindowContext(scope: scope))
    }

    private var projectSlug: String? {
        if case .project(let slug) = context.scope { return slug }
        return nil
    }
    private var scopedProject: ProjectInfo? { projectSlug.flatMap(store.project) }
    private var look: WindowLook { context.resolveLook(store: store, themeID: themeID, appearance: appearance) }

    var body: some View {
        let look = self.look
        Group {
            if let slug = projectSlug, scopedProject == nil {
                ProjectGoneView(slug: slug)
            } else {
                shell
            }
        }
        .environment(\.sbTheme, look.theme)
        .preferredColorScheme(look.appearance.colorScheme)
        .focusedSceneValue(\.windowFocus, WindowFocus(context: context, selection: context.selection,
                                                     canSplit: context.canSplit, canClosePane: context.canClosePane))
        .environmentObject(context)
        .sheet(item: Binding(get: { dialogs.current.flatMap { ($0.window ?? mainWindowID) == context.id || ($0.window == nil && context.scope == .all) ? $0 : nil } },
                             set: { _ in })) { request in
            DialogCardView(request: request) { dialogs.answer($0) }
                .environment(\.sbTheme, look.theme)
                .environmentObject(store)
        }
        .onAppear {
            store.registerWindow(context)
            if context.scope == .all { mainWindowID = context.id }
            context.reconcile(with: store)
            if context.scope == .all {
                store.offerToKillLeftovers()
                for slug in store.projectWindowsToReopen() { openWindow(id: "project", value: slug) }
            }
        }
        .onChange(of: store.projects) { _, _ in context.reconcile(with: store) }
        .onChange(of: store.openProjectWindows) { _, _ in context.reconcile(with: store) }
        .onChange(of: store.lastInstalled) { _, slug in
            guard context.scope == .all, let slug, let p = store.project(slug), let c = p.allCommands.first else { return }
            context.railProject = slug
            context.selection = CommandRef(project: slug, address: c.address)
        }
        .onDisappear { store.releaseAll(from: context) }
    }

    private var shell: some View {
        ThemedShell(context: context, look: look)
    }
}

/// Window-level state shared so dialogs without a target land on the main window.
@MainActor private var mainWindowID: UUID?

private struct ThemedShell: View {
    @EnvironmentObject var store: EngineStore
    @ObservedObject var context: WindowContext
    let look: WindowLook
    @Environment(\.colorScheme) private var scheme
    @Environment(\.openWindow) private var openWindow
    @Environment(\.dismissWindow) private var dismissWindow

    var body: some View {
        let p = look.theme.palette(scheme)
        let projects = context.projects(in: store)
        HStack(spacing: 0) {
            SidebarView()
            Rectangle().fill(look.theme.style.paneInset > 0 ? .clear : p.line).frame(width: 1)
            ZStack {
                p.window
                if context.scope == .all && store.projects.isEmpty {
                    NoProjectsView()
                } else if context.scope == .all && projects.isEmpty {
                    BoardClearView()
                } else {
                    PaneTreeView(layout: context.layout)
                        .navigationTitle(windowTitle)
                }
                if let toast = context.toast {
                    VStack {
                        Spacer()
                        Text(toast).font(SBType.secondary).foregroundStyle(p.text)
                            .padding(.horizontal, 14).padding(.vertical, 8)
                            .background(p.button, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                            .hairline(p.line, radius: 8)
                            .shadow(color: .black.opacity(0.2), radius: 8, y: 3)
                            .padding(.bottom, 24)
                    }
                    .transition(.opacity)
                }
            }
            .animation(.easeInOut(duration: 0.15), value: context.toast)
        }
        .background(p.window)
        .frame(minWidth: SBMetrics.windowMinimum.width, minHeight: SBMetrics.windowMinimum.height)
        // The title bar is hidden; our chrome owns the full height and makes its own room
        // for the traffic lights (rail / sidebar top strips).
        .ignoresSafeArea(.container, edges: .top)
        .background(WindowConfigurator())
    }

    private var windowTitle: String {
        if case .project(let slug) = context.scope, let p = store.project(slug) {
            if let ref = context.selection, let c = store.command(ref) { return "\(c.displayName) – \(p.name)" }
            return p.name
        }
        guard let ref = context.selection else { return "Switchboard" }
        let project = store.project(ref.project)?.name ?? ref.project
        let name = store.command(ref)?.displayName ?? ref.address
        return "\(name) – \(project)"
    }
}

/// Hidden title bar with the traffic lights inside our chrome.
private struct WindowConfigurator: NSViewRepresentable {
    func makeNSView(context: Context) -> NSView {
        let v = NSView()
        DispatchQueue.main.async { configure(v.window) }
        return v
    }
    func updateNSView(_ nsView: NSView, context: Context) { configure(nsView.window) }
    private func configure(_ window: NSWindow?) {
        guard let window else { return }
        window.titlebarAppearsTransparent = true
        window.titleVisibility = .hidden
        window.acceptsMouseMovedEvents = true // ⌘-hover link underlines
        window.styleMask.insert(.fullSizeContentView)
        window.isMovableByWindowBackground = false
    }
}

/// First launch: nothing installed yet.
struct NoProjectsView: View {
    @EnvironmentObject var store: EngineStore
    @Environment(\.sbPalette) private var p
    var body: some View {
        VStack(spacing: 10) {
            Image(systemName: "switch.2").font(.system(size: 28)).foregroundStyle(p.text4)
            Text("Add a project to get started").font(.system(size: 15, weight: .semibold)).foregroundStyle(p.text)
            Text("Drop a folder with config.yaml anywhere here, or press ⌘O.").font(SBType.secondary).foregroundStyle(p.text3)
            SmallButton(label: "Add Project…  ⌘O", primary: true) { store.addProjectViaPanel() }.padding(.top, 6)
        }
    }
}

/// Every project is open in its own window.
struct BoardClearView: View {
    @EnvironmentObject var store: EngineStore
    @Environment(\.sbTheme) private var theme
    @Environment(\.colorScheme) private var scheme
    @Environment(\.openWindow) private var openWindow
    @Environment(\.dismissWindow) private var dismissWindow

    var body: some View {
        let look = Look(theme: theme, scheme: scheme)
        let p = look.p
        VStack(alignment: .leading, spacing: 14) {
            Text("The board is clear").font(.system(size: 17, weight: .semibold)).foregroundStyle(p.text)
            Text("Every project is open in its own window. Close a project's window, or bring it back, to put it on the board again.")
                .font(SBType.secondary).foregroundStyle(p.text2).fixedSize(horizontal: false, vertical: true)
            VStack(spacing: 6) {
                ForEach(store.openProjectWindows.sorted(), id: \.self) { slug in
                    if let project = store.project(slug) {
                        let running = store.runningCount(project: slug)
                        let crashed = store.crashedCount(project: slug)
                        HStack(spacing: 10) {
                            ProjectIcon(project: project, size: 24, radius: 6)
                            VStack(alignment: .leading, spacing: 1) {
                                Text(project.name).font(.system(size: 13, weight: .semibold)).foregroundStyle(p.text)
                                HStack(spacing: 4) {
                                    Circle().fill(running > 0 ? p.running : p.stopped).frame(width: 5, height: 5)
                                    Text("\(running) running").foregroundStyle(p.text3)
                                    if crashed > 0 { Text("· \(crashed) crashed").foregroundStyle(p.crashed) }
                                }
                                .font(SBType.meta)
                            }
                            Spacer()
                            SmallButton(label: "Bring Back") { dismissWindow(id: "project", value: slug) }
                            SmallButton(label: "Show Window") { openWindow(id: "project", value: slug) }
                        }
                        .padding(10)
                        .background(p.pane, in: RoundedRectangle(cornerRadius: look.s.cardRadius, style: .continuous))
                        .hairline(p.line, radius: look.s.cardRadius)
                    }
                }
            }
            HStack(spacing: 8) {
                SmallButton(label: "Add Project…  ⌘O") { store.addProjectViaPanel() }
                Text("or drop a project folder anywhere here").font(SBType.secondary).foregroundStyle(p.text3)
            }
        }
        .frame(width: 440)
    }
}

/// A project window whose project was deleted or renamed away.
struct ProjectGoneView: View {
    @Environment(\.sbPalette) private var p
    @Environment(\.openWindow) private var openWindow
    @Environment(\.dismissWindow) private var dismissWindow
    let slug: String
    var body: some View {
        VStack(spacing: 10) {
            Image(systemName: "folder.badge.questionmark").font(.system(size: 26)).foregroundStyle(p.text4)
            Text("Project removed").font(.system(size: 15, weight: .semibold)).foregroundStyle(p.text)
            Text("“\(slug)” is no longer in the Projects folder.").font(SBType.secondary).foregroundStyle(p.text3)
            HStack(spacing: 8) {
                SmallButton(label: "Close Window", primary: true) { dismissWindow() }
                SmallButton(label: "Show All Projects") { openWindow(id: "main") }
            }
            .padding(.top, 6)
        }
        .frame(minWidth: 480, minHeight: 320)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(p.window)
    }
}
