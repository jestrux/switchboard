import SwiftUI
import UniformTypeIdentifiers

/// Board sidebar (rail + one project, or the full tree) and project-window sidebar.
struct SidebarView: View {
    @EnvironmentObject var store: EngineStore
    @EnvironmentObject var context: WindowContext
    @Environment(\.sbTheme) private var theme
    @Environment(\.colorScheme) private var scheme
    @Environment(\.openWindow) private var openWindow
    @AppStorage(ThemeSettings.railKey) private var railOn = true
    @State private var dropTargeted = false
    @State private var collapsedProjects: Set<String> = []

    private var look: Look { Look(theme: theme, scheme: scheme) }
    private var useRail: Bool { context.scope == .all && railOn }

    /// Design default: 236 beside the rail, 272 for the tree, 268 in a project window.
    static func defaultWidth(scope: WindowScope, rail: Bool) -> CGFloat {
        scope == .all ? (rail ? SBMetrics.sidebarWidthRail : SBMetrics.sidebarWidthTree) : 268
    }

    var body: some View {
        let p = look.p
        let projects = context.projects(in: store)
        HStack(spacing: 0) {
            if useRail {
                RailView(projects: projects, collapsedProjects: $collapsedProjects)
                Rectangle().fill(p.line).frame(width: 1)
            }
            VStack(spacing: 0) {
                if !useRail { trafficLightSpacer }
                sidebarHeader(projects)
                ScrollView {
                    VStack(alignment: .leading, spacing: 0) {
                        rows(projects)
                    }
                    .padding(.horizontal, 8)
                    .padding(.vertical, 6)
                }
                Spacer(minLength: 0)
                if let slug = noticeProject(projects), let project = store.project(slug) {
                    ProjectNotices(project: project)
                }
                SidebarFoot()
            }
            .frame(width: context.effectiveSidebarWidth(default: SidebarView.defaultWidth(scope: context.scope, rail: useRail)))
            .background(p.sidebar)
        }
        .overlay {
            if dropTargeted {
                RoundedRectangle(cornerRadius: 8).strokeBorder(p.accent, lineWidth: 2)
                    .background(p.accentSoft.opacity(0.5)).padding(4).allowsHitTesting(false)
            }
        }
        .dropDestination(for: URL.self) { urls, _ in
            guard context.scope == .all else { return false }
            for url in urls where url.isFileURL { store.addProject(from: url, window: context.id) }
            return !urls.isEmpty
        } isTargeted: { dropTargeted = $0 && context.scope == .all }
        // Selecting a command from a pane, the menu bar or a new terminal opens its group,
        // so the selected row is never hidden inside a collapsed one.
        .onChange(of: context.selection) { _, ref in
            guard let ref, let project = store.project(ref.project),
                  let group = project.groups.first(where: { $0.commands.contains { $0.address == ref.address } })
            else { return }
            store.setGroup(project: ref.project, group: group.id, expanded: true)
        }
    }

    /// The project whose repo notices show above the foot: the one this sidebar is about.
    private func noticeProject(_ projects: [ProjectInfo]) -> String? {
        switch context.scope {
        case .project(let slug): return slug
        case .all: return useRail ? (context.railProject ?? projects.first?.slug) : context.selection?.project
        }
    }

    /// Room for the traffic lights when the title bar is hidden and there is no rail:
    /// 52 pt in tree mode (with the add button), 40 pt in a project window.
    private var trafficLightSpacer: some View {
        HStack {
            Spacer()
            if context.scope == .all {
                Button { store.addProjectViaPanel() } label: {
                    Image(systemName: "plus").font(.system(size: 11, weight: .semibold))
                        .foregroundStyle(look.p.text2)
                        .frame(width: 26, height: 26)
                        .background(look.p.button, in: RoundedRectangle(cornerRadius: 7, style: .continuous))
                        .overlay { RoundedRectangle(cornerRadius: 7, style: .continuous).strokeBorder(look.p.buttonLine) }
                }
                .buttonStyle(.plain)
                .help("Add Project… (⌘O)")
            }
        }
        .frame(height: context.scope == .all ? 52 : 40)
        .padding(.horizontal, 14)
    }

    // MARK: Header

    @ViewBuilder
    private func sidebarHeader(_ projects: [ProjectInfo]) -> some View {
        switch context.scope {
        case .project:
            if let project = projects.first { ProjectWindowHeader(project: project) }
        case .all:
            if useRail, let project = projects.first(where: { $0.slug == context.railProject }) ?? projects.first {
                BoardProjectHeader(project: project)
            }
        }
    }

    // MARK: Rows

    @ViewBuilder
    private func rows(_ projects: [ProjectInfo]) -> some View {
        if projects.isEmpty, context.scope == .all {
            Group {
                if store.projects.isEmpty { EmptySidebar() } else { AllExtractedSidebar() }
            }
            .padding(.top, useRail ? 40 : 0)
        } else if useRail {
            if let project = projects.first(where: { $0.slug == context.railProject }) ?? projects.first {
                if let error = project.error {
                    BrokenProjectCard(project: project, error: error)
                } else {
                    projectContents(project, indent: 0)
                }
            }
        } else if case .project = context.scope {
            if let project = projects.first {
                if let error = project.error {
                    BrokenProjectCard(project: project, error: error)
                } else {
                    projectContents(project, indent: 0)
                }
            }
        } else {
            ForEach(projects, id: \.slug) { project in
                if let error = project.error {
                    BrokenProjectCard(project: project, error: error)
                } else {
                    let collapsed = collapsedProjects.contains(project.slug)
                    TreeProjectRow(project: project, collapsed: collapsed) {
                        if collapsed { collapsedProjects.remove(project.slug) } else { collapsedProjects.insert(project.slug) }
                    }
                    .draggable(DragPayload.project(project.slug))
                    .reorderTarget(edge: .top) { payload in
                        guard let slug = DragPayload.parseProject(payload) else { return false }
                        store.moveProject(slug, toSlotOf: project.slug)
                        return true
                    }
                    if !collapsed { projectContents(project, indent: 12) }
                }
            }
        }
    }

    @ViewBuilder
    private func projectContents(_ project: ProjectInfo, indent: CGFloat) -> some View {
        // The board first: what needs the author, then the commands (plans/06-board.md, B2).
        BoardRow(project: project.slug, indent: indent).id("\(project.slug):@board")
        // Ids are prefixed with the project so two projects' `frontend/dev` rows never share identity.
        ForEach(project.commands, id: \.address) { cmd in
            CommandRow(project: project.slug, command: cmd, indent: indent)
                .id("\(project.slug):\(cmd.address)")
        }
        ForEach(project.groups, id: \.id) { group in
            let collapsed = !store.isGroupExpanded(project: project.slug, group: group.id)
            GroupRow(project: project.slug, group: group, collapsed: collapsed, indent: indent) {
                store.setGroup(project: project.slug, group: group.id, expanded: collapsed)
            }
            .id("\(project.slug):group:\(group.id)")
            if !collapsed {
                ForEach(group.commands, id: \.address) { cmd in
                    CommandRow(project: project.slug, command: cmd, indent: indent + SBMetrics.commandIndent)
                        .id("\(project.slug):\(cmd.address)")
                }
            }
        }
    }
}

// MARK: - Rail

struct RailView: View {
    @EnvironmentObject var store: EngineStore
    @EnvironmentObject var context: WindowContext
    @ObservedObject private var desk = Desk.shared
    @Environment(\.sbTheme) private var theme
    @Environment(\.colorScheme) private var scheme
    let projects: [ProjectInfo]
    @Binding var collapsedProjects: Set<String>

    var body: some View {
        let look = Look(theme: theme, scheme: scheme)
        let p = look.p
        VStack(spacing: 14) {
            Color.clear.frame(height: 11) // traffic lights row (margin-bottom 14 = the stack gap)
            // One icon per company: its products are picked in the sidebar (Desk, ProductPicker).
            ForEach(desk.entries(projects)) { entry in
                switch entry {
                case .project(let project):
                    tile(project)
                case .company(let company, let members):
                    let current = desk.current(company, members: members)
                    CompanyTile(company: company, members: members, icon: members.first, selected: members.contains { $0.slug == context.railProject })
                        .onTapGesture { if let current { context.railProject = current.slug } }
                        .contextMenu { if let current { ProjectMenu(project: current) } }
                        .draggable(DragPayload.project(members.first?.slug ?? ""))
                        .reorderTarget(edge: .top) { payload in
                            guard let slug = DragPayload.parseProject(payload), let first = members.first else { return false }
                            withAnimation(SBMotion.railSettle) { store.moveProject(slug, toSlotOf: first.slug) }
                            return true
                        }
                }
            }
            Rectangle().fill(p.line).frame(width: 28, height: 1)
            Button { store.addProjectViaPanel() } label: {
                Image(systemName: "plus").font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(p.text3)
                    .frame(width: 32, height: 32)
                    .overlay { RoundedRectangle(cornerRadius: 9).strokeBorder(p.line, style: StrokeStyle(lineWidth: 1, dash: [3, 3])) }
            }
            .buttonStyle(.plain)
            .help("Add Project… (⌘O)")
            Spacer()
        }
        .padding(.top, 18)
        .frame(width: SBMetrics.railWidth)
        .frame(maxHeight: .infinity)
        .background(p.rail)
    }

    private func tile(_ project: ProjectInfo) -> some View {
        RailTile(project: project, selected: context.railProject == project.slug)
            .onTapGesture { context.railProject = project.slug }
            .contextMenu { ProjectMenu(project: project) }
            .draggable(DragPayload.project(project.slug))
            .reorderTarget(edge: .top) { payload in
                guard let slug = DragPayload.parseProject(payload) else { return false }
                withAnimation(SBMotion.railSettle) { store.moveProject(slug, toSlotOf: project.slug) }
                return true
            }
    }
}

/// A company on the rail: one icon, with what its products add up to (running, needing you, a
/// crash). A tap opens the product last used in it.
struct CompanyTile: View {
    @EnvironmentObject var store: EngineStore
    @ObservedObject private var boards = BoardCache.shared
    @Environment(\.sbTheme) private var theme
    @Environment(\.colorScheme) private var scheme
    let company: String
    let members: [ProjectInfo]
    let icon: ProjectInfo?
    let selected: Bool

    var body: some View {
        let look = Look(theme: theme, scheme: scheme)
        let p = look.p
        let running = members.reduce(0) { $0 + store.runningCount(project: $1.slug) }
        let crashed = members.contains { store.crashedCount(project: $0.slug) > 0 }
        let needs = members.reduce(0) { $0 + (boards.data($1.slug)?.needsYou ?? 0) }
        ZStack(alignment: .bottomTrailing) {
            if let icon { ProjectIcon(project: icon, size: SBMetrics.railTile, radius: look.s.tileRadius) }
            if running > 0 { RailTile.badge("\(running)", bg: p.running, fg: look.isDark ? Color(hex: 0x0B0C0E) : .white, look: look) }
            if needs > 0 {
                RailTile.badge("\(needs)", bg: p.accent, fg: p.onAccent, look: look).offset(x: 0, y: -SBMetrics.railTile + 8)
                    .help("\(needs) on \(company)'s boards need you")
            }
            if crashed {
                Circle().fill(p.crashed).frame(width: 7, height: 7)
                    .shadow(color: look.glows ? p.crashed : .clear, radius: 3)
                    .offset(x: needs > 0 ? -SBMetrics.railTile + 4 : 2, y: -SBMetrics.railTile + 6)
            }
        }
        .onAppear { members.forEach { boards.watch($0.slug, engine: store.engine) } }
        .onDisappear { members.forEach { boards.unwatch($0.slug) } }
        .padding(4)
        .background {
            if selected {
                RoundedRectangle(cornerRadius: look.s.tileRadius + 4, style: .continuous).strokeBorder(p.accent, lineWidth: 2)
            }
        }
        .help("\(company): \(members.map(\.name).joined(separator: ", "))")
        .animation(SBMotion.railSettle, value: selected)
    }
}

struct RailTile: View {
    @EnvironmentObject var store: EngineStore
    @ObservedObject private var boards = BoardCache.shared
    @Environment(\.sbTheme) private var theme
    @Environment(\.colorScheme) private var scheme
    let project: ProjectInfo
    let selected: Bool

    var body: some View {
        let look = Look(theme: theme, scheme: scheme)
        let p = look.p
        let running = store.runningCount(project: project.slug)
        let crashed = store.crashedCount(project: project.slug)
        ZStack(alignment: .bottomTrailing) {
            if project.error != nil {
                RoundedRectangle(cornerRadius: look.s.tileRadius, style: .continuous)
                    .fill(p.errorSoft)
                    .overlay { RoundedRectangle(cornerRadius: look.s.tileRadius, style: .continuous).strokeBorder(p.error, style: StrokeStyle(lineWidth: 1, dash: [3, 2])) }
                    .overlay { Text(String(project.name.prefix(2)).lowercased()).font(.system(size: 11, weight: .bold)).foregroundStyle(p.error) }
                    .frame(width: SBMetrics.railTile, height: SBMetrics.railTile)
            } else {
                ProjectIcon(project: project, size: SBMetrics.railTile, radius: look.s.tileRadius)
            }
            if project.error != nil {
                badge("!", bg: p.error, fg: Color(hex: 0x1A1208), look: look)
            } else if running > 0 {
                badge("\(running)", bg: p.running, fg: look.isDark ? Color(hex: 0x0B0C0E) : .white, look: look)
            }
            // What needs the author on this project's board (plans/06-board.md, B2; w-modes: a
            // count in All Projects), top right; a crash dot moves to the top left beside it.
            let needs = boards.data(project.slug)?.needsYou ?? 0
            if needs > 0 && project.error == nil {
                badge("\(needs)", bg: p.accent, fg: p.onAccent, look: look)
                    .offset(x: 0, y: -SBMetrics.railTile + 8)
                    .help("\(needs) on the board need you")
            }
            if crashed > 0 && project.error == nil {
                Circle().fill(p.crashed).frame(width: 7, height: 7)
                    .shadow(color: look.glows ? p.crashed : .clear, radius: 3)
                    .offset(x: needs > 0 ? -SBMetrics.railTile + 4 : 2, y: -SBMetrics.railTile + 6)
            }
        }
        .onAppear { boards.watch(project.slug, engine: store.engine) }
        .onDisappear { boards.unwatch(project.slug) }
        .padding(4)
        .background {
            if selected {
                RoundedRectangle(cornerRadius: look.s.tileRadius + 4, style: .continuous)
                    .strokeBorder(p.accent, lineWidth: 2)
            }
        }
        .help(Desk.shared.displayName(project))
        .animation(SBMotion.railSettle, value: selected)
    }

    private func badge(_ text: String, bg: Color, fg: Color, look: Look) -> some View { Self.badge(text, bg: bg, fg: fg, look: look) }

    static func badge(_ text: String, bg: Color, fg: Color, look: Look) -> some View {
        Text(text).font(.system(size: 9, weight: .bold)).monospacedDigit()
            .foregroundStyle(fg)
            .padding(.horizontal, 4).frame(minWidth: 15, minHeight: 15)
            .background(bg, in: Capsule())
            .overlay(Capsule().strokeBorder(look.p.rail, lineWidth: 2))
            .shadow(color: look.glows ? bg : .clear, radius: 3)
            .offset(x: 4, y: 4)
    }
}

// MARK: - Headers

/// Rail mode: the selected project's name, Open in New Window and Start all, plus a sub line.
struct BoardProjectHeader: View {
    @EnvironmentObject var store: EngineStore
    @EnvironmentObject var context: WindowContext
    @Environment(\.sbTheme) private var theme
    @Environment(\.colorScheme) private var scheme
    @Environment(\.openWindow) private var openWindow
    @ObservedObject private var desk = Desk.shared
    @AppStorage(Desk.switcherKey) private var style = "tabs"
    let project: ProjectInfo

    var body: some View {
        let p = theme.palette(scheme)
        let running = store.runningCount(project: project.slug)
        let crashed = store.crashedCount(project: project.slug)
        let total = project.allCommands.count
        let panes = context.layout.leaves.filter { $0.ref?.project == project.slug }.count
        let products = desk.products(of: project, in: store.projects)
        let company = products.isEmpty ? nil : desk.company(project.slug)
        let tabs = ProductPicker.usesTabs(products.count, style: style)
        let pick: (ProjectInfo) -> Void = { prod in context.railProject = prod.slug; desk.remember(prod.slug) }
        let picker: Bool = company != nil
        VStack(alignment: .leading, spacing: 3) {
            HStack(spacing: 6) {
                let title = Text(company != nil && tabs ? company! : project.name).font(SBType.sidebarTitle).foregroundStyle(p.text).lineLimit(1)
                if let company, !tabs {
                    ProductMenuTitle(company: company, products: products, current: project, pick: pick) { title }
                } else { title.help(desk.displayName(project)) }
                Spacer(minLength: 4)
                IconButton("macwindow.on.rectangle", help: "Open in New Window (⌘⇧N)") {
                    openWindow(id: "project", value: project.slug)
                }
                .foregroundStyle(p.text3)
                // With the product menu as the title, Start all is just its icon, so the name has room.
                if picker && !tabs {
                    IconButton("play.fill", help: "Start all") { store.startAll(project: project.slug) }.foregroundStyle(p.text3)
                } else {
                    SmallButton(label: "Start all", icon: "play.fill") { store.startAll(project: project.slug) }.fixedSize()
                }
            }
            HStack(spacing: 4) {
                Text("\(running) of \(total) running").foregroundStyle(p.text3)
                if crashed > 0 { Text("·").foregroundStyle(p.text4); Text("\(crashed) crashed").foregroundStyle(p.crashed) }
                if panes > 0 { Text("·").foregroundStyle(p.text4); Text("\(panes) pane\(panes == 1 ? "" : "s")").foregroundStyle(p.text3) }
            }
            .font(SBType.secondary)
            if picker, tabs { ProductTabs(products: products, current: project, pick: pick).padding(.top, 8) }
        }
        .onAppear { desk.remember(project.slug) }
        .padding(.horizontal, 14)
        // The traffic lights sit over the rail, so this column needs no title-bar allowance.
        .padding(.top, 14)
        .padding(.bottom, 12)
        .contextMenu { ProjectMenu(project: project) }
    }
}

/// Project window: the project is the window. Icon, name, running count and whole-project actions.
struct ProjectWindowHeader: View {
    @EnvironmentObject var store: EngineStore
    @EnvironmentObject var context: WindowContext
    @Environment(\.sbTheme) private var theme
    @Environment(\.colorScheme) private var scheme
    @Environment(\.dismissWindow) private var dismissWindow
    @Environment(\.openWindow) private var openWindow
    @ObservedObject private var desk = Desk.shared
    @AppStorage(Desk.switcherKey) private var style = "tabs"
    let project: ProjectInfo
    @State private var hovering = false

    var body: some View {
        let look = Look(theme: theme, scheme: scheme)
        let p = look.p
        let running = store.runningCount(project: project.slug)
        let total = project.allCommands.count
        let products = desk.products(of: project, in: store.projects)
        let company = products.isEmpty ? nil : desk.company(project.slug)
        let tabs = ProductPicker.usesTabs(products.count, style: style)
        // A product's own window: picking another opens (or brings forward) that product's window.
        let pick: (ProjectInfo) -> Void = { prod in if prod.slug != project.slug { openWindow(id: "project", value: prod.slug) } }
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 10) {
                ProjectIcon(project: project, size: SBMetrics.projectHeaderIcon, radius: look.s.tileRadius)
                VStack(alignment: .leading, spacing: 2) {
                    let title = Text(company != nil && tabs ? company! : project.name).font(SBType.projectWindowTitle).tracking(-0.4).foregroundStyle(p.text).lineLimit(1)
                    if let company, !tabs {
                        ProductMenuTitle(company: company, products: products, current: project, pick: pick) { title }
                    } else { title.help(desk.displayName(project)) }
                    HStack(spacing: 4) {
                        Circle().fill(running > 0 ? p.running : p.stopped).frame(width: 6, height: 6)
                        Text("\(running) running").foregroundStyle(running > 0 ? p.running : p.text3)
                        Text("of \(total)").foregroundStyle(p.text3)
                    }
                    .font(SBType.secondary)
                }
                Spacer(minLength: 0)
            }
            HStack(spacing: 4) {
                // Labelled buttons when they fit; icons only in a narrow sidebar.
                ViewThatFits(in: .horizontal) {
                    actionButtons(running: running, labels: true)
                    actionButtons(running: running, labels: false)
                }
                Spacer(minLength: 0)
                Menu {
                    ProjectMenu(project: project)
                } label: {
                    Image(systemName: "ellipsis").font(.system(size: 12, weight: .semibold))
                        .frame(width: 26, height: 24)
                }
                .menuStyle(.borderlessButton).menuIndicator(.hidden).fixedSize()
                .foregroundStyle(p.text3)
            }
            if company != nil, tabs { ProductTabs(products: products, current: project, pick: pick) }
        }
        .padding(.top, 12).padding(.bottom, 12).padding(.leading, 12).padding(.trailing, 10)
        .background(hovering ? p.hover : .clear, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay { RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(hovering ? p.line : .clear) }
        .padding(.horizontal, 8).padding(.bottom, 8)
        .onHover { hovering = $0 }
        .contextMenu { ProjectMenu(project: project) }
    }

    private func actionButtons(running: Int, labels: Bool) -> some View {
        HStack(spacing: 4) {
            headerButton("play.fill", "Start all", bordered: hovering, showLabel: labels) { store.startAll(project: project.slug) }
            headerButton("stop.fill", "Stop all", bordered: hovering, showLabel: labels) { store.stopAll(project: project.slug) }
                .disabled(running == 0)
            headerButton("arrow.clockwise", "Restart all", bordered: hovering, showLabel: labels) { store.restartAll(project: project.slug) }
        }
    }

    private func headerButton(_ icon: String, _ label: String, bordered: Bool, showLabel: Bool = true, action: @escaping () -> Void) -> some View {
        let p = theme.palette(scheme)
        return Button(action: action) {
            HStack(spacing: 4) {
                Image(systemName: icon).font(.system(size: showLabel ? 8 : 9, weight: .bold))
                if showLabel { Text(label).font(.system(size: 11)) }
            }
            .fixedSize()
            .foregroundStyle(p.text2)
            .padding(.horizontal, 7).frame(height: 24)
            .background(bordered ? p.button : .clear, in: RoundedRectangle(cornerRadius: 6, style: .continuous))
            .overlay { if bordered { RoundedRectangle(cornerRadius: 6, style: .continuous).strokeBorder(p.buttonLine) } }
        }
        .buttonStyle(.plain)
        .help(label)
    }
}

/// Shared project context menu.
struct ProjectMenu: View {
    @EnvironmentObject var store: EngineStore
    @EnvironmentObject var context: WindowContext
    @Environment(\.openWindow) private var openWindow
    @Environment(\.dismissWindow) private var dismissWindow
    let project: ProjectInfo

    var body: some View {
        Button("Start All") { store.startAll(project: project.slug) }
        Button("Stop All") { store.stopAll(project: project.slug) }
        Button("Restart All") { store.restartAll(project: project.slug) }
        Divider()
        if context.scope == .all {
            Button("Open in New Window") { openWindow(id: "project", value: project.slug) }
        } else {
            Button("Move to All Projects") { dismissWindow(id: "project", value: project.slug) }
        }
        Button("New Terminal") {
            if let ref = store.newTerminal(project: project.slug, group: nil) { context.selection = ref }
        }
        Button("Edit Config") { store.editConfig(project.slug) }
        Button("Reveal in Finder") { store.revealInFinder(project.slug) }
        Divider()
        DeskMenu(project: project)
        if project.home == nil {
            if project.workingFolders.count > 1 {
                Menu("Keep Settings in Repo") { HomeFolderMenuItems(project: project) }
            } else {
                Button("Keep Settings in Repo…") { store.keepSettingsInRepo(project.slug) }
            }
        }
        Divider()
        Button("Delete Project…") { store.deleteProject(project.slug, window: context.id) }
    }
}

// MARK: - Rows

/// Tree mode: a project row with icon, name, run count; collapsed shows a row of LEDs.
struct TreeProjectRow: View {
    @EnvironmentObject var store: EngineStore
    @EnvironmentObject var context: WindowContext
    @Environment(\.sbTheme) private var theme
    @Environment(\.colorScheme) private var scheme
    let project: ProjectInfo
    let collapsed: Bool
    let toggle: () -> Void
    @State private var hovering = false

    var body: some View {
        let look = Look(theme: theme, scheme: scheme)
        let p = look.p
        let running = store.runningCount(project: project.slug)
        let crashed = store.crashedCount(project: project.slug)
        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 8) {
                Image(systemName: collapsed ? "chevron.right" : "chevron.down")
                    .font(.system(size: 9, weight: .bold)).foregroundStyle(p.text4).frame(width: 10)
                ProjectIcon(project: project, size: 20, radius: 6)
                Text(project.name).font(.system(size: 13, weight: .semibold)).foregroundStyle(running > 0 ? p.text : p.text2).lineLimit(1).help(project.name)
                if crashed > 0 { Text("\(crashed) crashed").font(SBType.meta).foregroundStyle(p.crashed).fixedSize() }
                Spacer(minLength: 4)
                if hovering {
                    RunControls(anyRunning: running > 0,
                                start: { store.startAll(project: project.slug) },
                                stop: { store.stopAll(project: project.slug) },
                                restart: { store.restartAll(project: project.slug) })
                } else if running > 0 {
                    HStack(spacing: 4) {
                        Circle().fill(p.running).frame(width: 6, height: 6)
                        Text("\(running)").font(SBType.meta).foregroundStyle(p.text2)
                    }
                } else {
                    Text("\(project.allCommands.count) idle").font(SBType.meta).foregroundStyle(p.text3)
                }
            }
            .frame(height: SBMetrics.projectRow)
            if collapsed {
                HStack(spacing: 4) {
                    ForEach(project.allCommands, id: \.address) { cmd in
                        let st = store.state(CommandRef(project: project.slug, address: cmd.address)).status
                        Circle().fill(ledColour(st, p)).frame(width: 5, height: 5)
                    }
                }
                .padding(.leading, 38).padding(.bottom, 6)
            }
        }
        .padding(.horizontal, 6)
        .contentShape(Rectangle())
        .onTapGesture { toggle() }
        .onHover { hovering = $0 }
        .contextMenu { ProjectMenu(project: project) }
    }

    private func ledColour(_ s: CommandStatus, _ p: SBPalette) -> Color {
        switch s {
        case .running: return p.running
        case .starting: return p.starting
        case .crashed: return p.crashed
        case .error: return p.error
        case .exited: return p.text2
        case .stopped: return p.stoppedSoft
        }
    }
}

struct GroupRow: View {
    @EnvironmentObject var store: EngineStore
    @Environment(\.sbTheme) private var theme
    @Environment(\.colorScheme) private var scheme
    let project: String
    let group: GroupInfo
    let collapsed: Bool
    let indent: CGFloat
    let toggle: () -> Void
    @State private var hovering = false

    var body: some View {
        let look = Look(theme: theme, scheme: scheme)
        let p = look.p
        let running = store.runningCount(project: project, group: group.id)
        let label = look.s.groupLabelUppercase ? group.name.uppercased() : group.name
        HStack(spacing: 6) {
            Image(systemName: collapsed ? "chevron.right" : "chevron.down")
                .font(.system(size: 8, weight: .bold)).foregroundStyle(p.text4).frame(width: 10)
            Image(systemName: GroupIcon.symbol(group.icon)).font(.system(size: 10.5, weight: .medium))
                .foregroundStyle(p.text3).frame(width: 14)
            Text(label).font(SBType.groupLabel(look.s)).tracking(look.s.groupLabelTracking).foregroundStyle(p.text3)
                .lineLimit(1).truncationMode(.tail)
            if let problem = group.problem {
                TriangleGlyph().fill(p.error).frame(width: 10, height: 10).help(problem)
            }
            Spacer(minLength: 4)
            if hovering {
                HStack(spacing: 2) {
                    IconButton("terminal", help: "New Terminal in \(group.name) (⌘T)") { newTerminal() }
                    RunControls(anyRunning: running > 0,
                                start: { store.startAll(project: project, group: group.id) },
                                stop: { store.stopAll(project: project, group: group.id) },
                                restart: { store.restartAll(project: project, group: group.id) })
                }
                .foregroundStyle(p.text2)
            } else if group.problem != nil {
                Text("unavailable").font(SBType.meta).foregroundStyle(p.error).fixedSize()
            } else if collapsed {
                Text(running > 0 ? "\(running) running" : "\(group.commands.count) idle").font(SBType.meta).foregroundStyle(p.text3).fixedSize()
            }
        }
        .frame(height: SBMetrics.groupRow)
        .padding(.leading, indent + 4)
        .padding(.trailing, 6)
        .contentShape(Rectangle())
        .onTapGesture { toggle() }
        .onHover { hovering = $0 }
        .help(group.name)
        .draggable(DragPayload.group(project, group.id))
        .reorderTarget(edge: .top) { payload in
            guard let g = DragPayload.parseGroup(payload), g.project == project else { return false }
            store.moveGroup(project: project, g.id, toSlotOf: group.id)
            return true
        }
        .contextMenu {
            Button("New Terminal") { newTerminal() }
            Menu("Icon") {
                Button {
                    store.setGroupIcon(project: project, group: group.id, icon: nil)
                } label: {
                    Label(group.iconIsExplicit ? "Automatic" : "Automatic  ✓", systemImage: "wand.and.stars")
                }
                Divider()
                ForEach(GroupIcon.keys, id: \.self) { key in
                    Button {
                        store.setGroupIcon(project: project, group: group.id, icon: key)
                    } label: {
                        Label(GroupIcon.label(key) + (group.iconIsExplicit && group.icon == key ? "  ✓" : ""), systemImage: GroupIcon.symbol(key))
                    }
                }
            }
            Divider()
            Button("Start All") { store.startAll(project: project, group: group.id) }
            Button("Stop All") { store.stopAll(project: project, group: group.id) }
            Button("Restart All") { store.restartAll(project: project, group: group.id) }
            if let cwd = group.cwd {
                Divider()
                Button("Reveal Folder in Finder") { store.revealFolder((cwd as NSString).expandingTildeInPath) }
            }
        }
    }

    @EnvironmentObject private var context: WindowContext
    private func newTerminal() {
        if let ref = store.newTerminal(project: project, group: group.id) { context.selection = ref }
    }
}

struct CommandRow: View {
    @EnvironmentObject var store: EngineStore
    @EnvironmentObject var context: WindowContext
    @Environment(\.sbTheme) private var theme
    @Environment(\.colorScheme) private var scheme
    let project: String
    let command: CommandInfo
    let indent: CGFloat
    @State private var hovering = false

    private var ref: CommandRef { CommandRef(project: project, address: command.address) }

    var body: some View {
        let look = Look(theme: theme, scheme: scheme)
        let p = look.p
        let state = store.state(ref)
        let selected = context.selection == ref
        let live = state.isAlive
        HStack(spacing: 8) {
            StatusGlyph(status: state.status, restartAt: state.restartAt, restartTotal: backoffTotal(state))
            if command.isTerminal {
                Image(systemName: "terminal").font(.system(size: 10, weight: .medium)).foregroundStyle(p.text3)
            }
            Text(command.name)
                .font(.system(size: 13, weight: selected ? .semibold : .regular))
                .foregroundStyle(selected ? p.selectionText : (state.status == .stopped && !command.cwdExists ? p.text2 : (state.status == .stopped ? p.text2 : p.text)))
                .lineLimit(1).truncationMode(.tail)
            if state.needsRestart {
                Text("↻ apply").font(.system(size: 10, weight: .medium))
                    .foregroundStyle(p.accentSoftText)
                    .padding(.horizontal, 5).frame(height: 16)
                    .background(p.accentSoft, in: Capsule())
                    .fixedSize()
            }
            Spacer(minLength: 4)
            if hovering {
                HStack(spacing: 2) {
                    if live {
                        IconButton("stop.fill", help: "Stop (⌘.)") { store.stop(ref) }
                        IconButton("arrow.clockwise", help: "Restart (⌘R)") { store.restart(ref) }
                    } else {
                        IconButton("play.fill", help: command.cwdExists ? "Start" : "Folder not found") { store.start(ref) }
                            .disabled(!command.cwdExists)
                    }
                    if command.isTerminal {
                        IconButton("xmark", help: "Close Terminal") { store.closeTerminal(ref) }
                    }
                }
                .foregroundStyle(selected ? p.selectionText : p.text2)
            } else {
                MetaText(state: state, command: command).fixedSize()
            }
            // The port keeps its place at the trailing edge whether or not the row is
            // hovered, so reaching for it never slides a Stop button under the pointer.
            if live, let port = state.port {
                PortLink(port: port, url: state.url, selected: selected).fixedSize()
            }
            if !hovering {
                if let n = context.paneNumber(of: ref), context.layout.leafCount > 1 {
                    Text("\(n)").font(.system(size: 10, weight: .semibold)).monospacedDigit()
                        .foregroundStyle(p.text3)
                        .frame(width: 16, height: 16)
                        .background(p.hover, in: RoundedRectangle(cornerRadius: 4))
                        .help("Shown in pane \(n)")
                }
            }
        }
        .frame(height: SBMetrics.commandRow)
        .padding(.leading, indent + 6)
        .padding(.trailing, 8)
        .background {
            if selected {
                switch look.s.selection {
                case .accentBar:
                    RoundedRectangle(cornerRadius: look.s.rowRadius, style: .continuous).fill(p.selection)
                        .overlay(alignment: .leading) { Rectangle().fill(p.accent).frame(width: 2).padding(.vertical, 6).padding(.leading, 2) }
                case .card:
                    RoundedRectangle(cornerRadius: look.s.rowRadius, style: .continuous).fill(p.selection)
                        .shadow(color: .black.opacity(0.08), radius: 2, y: 1)
                        .overlay { RoundedRectangle(cornerRadius: look.s.rowRadius, style: .continuous).strokeBorder(p.line) }
                }
            } else if hovering {
                RoundedRectangle(cornerRadius: look.s.rowRadius, style: .continuous).fill(p.hover)
            }
        }
        .contentShape(Rectangle())
        .onTapGesture { context.selection = ref }
        .onHover { hovering = $0 }
        .draggable(ref.dragString)
        .reorderTarget(edge: .top) { payload in
            guard let dragged = CommandRef.fromDragString(payload) else { return false }
            return store.moveCommand(dragged, toSlotOf: ref)
        }
        // The name can be cut short in a narrow sidebar, so the tooltip always carries it whole.
        .help("\(command.name) · \(state.summary)")
        .contextMenu {
            if live {
                Button("Stop") { store.stop(ref) }
                Button("Restart") { store.restart(ref) }
            } else {
                Button("Start") { store.start(ref) }.disabled(!command.cwdExists)
            }
            Divider()
            if command.isTerminal {
                Button("Rename…") { store.renameTerminal(ref, window: context.id) }
                Button("Save as Command…") {
                    store.saveTerminalAsCommand(ref, window: context.id) { context.selection = $0 }
                }
                Button("Close Terminal") { store.closeTerminal(ref) }
                Divider()
            } else {
                Text(command.run)
            }
            if let path = command.cwdPath {
                Button("Reveal Folder in Finder") { store.revealFolder(path) }.disabled(!command.cwdExists)
            }
        }
    }
}

/// Total backoff seconds, recovered from the "restarting in N s" message.
func backoffTotal(_ state: CommandState) -> Double? {
    guard let m = state.message, let range = m.range(of: #"restarting in (\d+) s"#, options: .regularExpression) else { return nil }
    let digits = m[range].filter(\.isNumber)
    return Double(digits)
}

/// Row meta: uptime, `↻ 16s`, `exit 0`, `no folder`, mono 11.
struct MetaText: View {
    @Environment(\.sbTheme) private var theme
    @Environment(\.colorScheme) private var scheme
    let state: CommandState
    let command: CommandInfo

    var body: some View {
        let p = theme.palette(scheme)
        switch state.status {
        case .running:
            // The port (drawn by the row) is the useful thing here; uptime lives in the pane footer.
            EmptyView()
        case .starting:
            Text("starting").font(SBType.meta).foregroundStyle(p.starting)
        case .exited:
            Text("exit 0").font(SBType.meta).foregroundStyle(p.text3)
        case .crashed:
            if let at = state.restartAt {
                TimelineView(.periodic(from: .now, by: 1)) { ctx in
                    let left = max(0, Int(at - ctx.date.timeIntervalSince1970.rounded()))
                    Text("↻ \(left)s").font(SBType.meta).monospacedDigit().foregroundStyle(p.crashed)
                }
            } else {
                Text(state.exitCode.map { "exit \($0)" } ?? "crashed").font(SBType.meta).foregroundStyle(p.crashed)
            }
        case .error:
            Text("no folder").font(SBType.meta).foregroundStyle(p.error)
        case .stopped:
            if !command.cwdExists { Text("no folder").font(SBType.meta).foregroundStyle(p.error) }
        }
    }
}

/// `:3006`, the port a running command announced. Click to open it in the browser.
struct PortLink: View {
    @Environment(\.sbPalette) private var p
    let port: UInt16
    let url: String?
    var selected = false
    @State private var hovering = false

    var body: some View {
        Button {
            if let url { LinkHandling.open(url) }
        } label: {
            Text(verbatim: ":\(port)")
                .font(SBType.meta)
                .foregroundStyle(hovering ? p.accentSoftText : (selected ? p.selectionText.opacity(0.75) : p.text3))
                .underline(hovering)
        }
        .buttonStyle(.plain)
        .onHover { hovering = $0 }
        .help(url.map { "Open \($0)" } ?? "Port \(port)")
    }
}

// MARK: - Cards, foot, empty

struct BrokenProjectCard: View {
    @EnvironmentObject var store: EngineStore
    @EnvironmentObject var context: WindowContext
    @Environment(\.sbTheme) private var theme
    @Environment(\.colorScheme) private var scheme
    let project: ProjectInfo
    let error: String

    var body: some View {
        let look = Look(theme: theme, scheme: scheme)
        let p = look.p
        let lines = error.split(separator: "\n").map(String.init)
        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 8) {
                TriangleGlyph().fill(p.error).frame(width: 12, height: 12)
                Text(project.name).font(.system(size: 13, weight: .semibold)).foregroundStyle(p.text)
                Spacer()
                Button("Edit Config") { store.editConfig(project.slug) }.buttonStyle(.plain)
                    .font(.system(size: 12)).foregroundStyle(p.accentSoftText)
            }
            Text(lines.count > 1 ? "\(lines.count) problems · \(lines[0])" : (lines.first ?? ""))
                .font(SBType.secondary).foregroundStyle(p.text2).lineLimit(3)
                .fixedSize(horizontal: false, vertical: true)
        }
        .padding(10)
        .background(p.errorSoft, in: RoundedRectangle(cornerRadius: look.s.cardRadius, style: .continuous))
        .overlay {
            RoundedRectangle(cornerRadius: look.s.cardRadius, style: .continuous)
                .strokeBorder(p.error.opacity(0.6), style: StrokeStyle(lineWidth: 1, dash: look.s.dashedWarningBorder ? [4, 3] : []))
        }
        .padding(.vertical, 4)
        .contextMenu { ProjectMenu(project: project) }
    }
}

/// Repo-related lines above the foot: settings changed in the repo, or the offer to keep
/// this project's settings in a repo folder.
struct ProjectNotices: View {
    @EnvironmentObject var store: EngineStore
    @Environment(\.sbTheme) private var theme
    @Environment(\.colorScheme) private var scheme
    let project: ProjectInfo
    @AppStorage private var dismissed: Bool

    init(project: ProjectInfo) {
        self.project = project
        _dismissed = AppStorage(wrappedValue: false, "homeOffer.dismissed.\(project.slug)")
    }

    var body: some View {
        let look = Look(theme: theme, scheme: scheme)
        let p = look.p
        if project.repoSettingsChanged {
            notice(p, look, icon: "arrow.triangle.2.circlepath", tint: p.accentSoftText, bg: p.accentSoft,
                   title: "Settings changed in the repo",
                   detail: Fmt.shortFolder((project.home ?? "").abbreviatingHome) + "/.switchboard/project.yaml") {
                SmallButton(label: "Apply", primary: true) { store.applyRepoSettings(project.slug) }
            }
        } else if project.error == nil, project.home == nil, !dismissed, let suggested = project.suggestedHome {
            notice(p, look, icon: "shippingbox", tint: p.text2, bg: p.hover,
                   title: "Keep \(project.name)’s settings in the repo?",
                   detail: Fmt.shortFolder(suggested) + "/.switchboard") {
                SmallButton(label: "Keep") { store.keepSettingsInRepo(project.slug, folder: suggested) }
                HomeFolderChooser(project: project)
                Button { dismissed = true } label: { Image(systemName: "xmark").font(.system(size: 9, weight: .bold)).foregroundStyle(p.text3) }
                    .buttonStyle(.plain).help("Not now. Project › Keep Settings in Repo… does this later.")
            }
        }
    }

    private func notice<Actions: View>(_ p: SBPalette, _ look: Look, icon: String, tint: Color, bg: Color, title: String, detail: String, @ViewBuilder actions: () -> Actions) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(alignment: .top, spacing: 7) {
                Image(systemName: icon).font(.system(size: 11)).foregroundStyle(tint).padding(.top, 1)
                VStack(alignment: .leading, spacing: 2) {
                    Text(title).font(.system(size: 12, weight: .medium)).foregroundStyle(p.text).fixedSize(horizontal: false, vertical: true)
                    Text(detail).font(SBType.meta).foregroundStyle(p.text3).lineLimit(1).truncationMode(.head)
                }
            }
            HStack(spacing: 6) { actions(); Spacer(minLength: 0) }
        }
        .padding(10)
        .background(bg, in: RoundedRectangle(cornerRadius: look.s.cardRadius, style: .continuous))
        .padding(.horizontal, 8).padding(.bottom, 8)
    }
}

/// "Choose…": the project's own working folders, then Other Folder… for anywhere else.
/// With a single working folder there is nothing to choose between, so it opens the picker.
struct HomeFolderChooser: View {
    @EnvironmentObject var store: EngineStore
    @Environment(\.sbPalette) private var p
    let project: ProjectInfo

    var body: some View {
        let folders = project.workingFolders
        if folders.count <= 1 {
            SmallButton(label: "Choose…") { store.keepSettingsInRepo(project.slug) }
        } else {
            Menu {
                HomeFolderMenuItems(project: project)
            } label: {
                Text("Choose…").font(.system(size: 12)).foregroundStyle(p.text)
            }
            .menuStyle(.borderlessButton)
            .menuIndicator(.hidden)
            .fixedSize()
            .padding(.horizontal, 9).frame(height: 24)
            .background(p.button, in: RoundedRectangle(cornerRadius: 6, style: .continuous))
            .overlay { RoundedRectangle(cornerRadius: 6, style: .continuous).strokeBorder(p.buttonLine) }
        }
    }
}

/// Menu entries shared by the sidebar offer and Project › Keep Settings in Repo.
struct HomeFolderMenuItems: View {
    @EnvironmentObject var store: EngineStore
    let project: ProjectInfo

    var body: some View {
        // The suggested folder first (sb_core::home: the main repo, or the e2e folder), marked;
        // it's added when it's a repo root that no command runs in directly.
        let suggested = store.engine.suggestHome(slug: project.slug)
        let rest = project.workingFolders.filter { $0.path != suggested }
        if let suggested {
            let label = project.workingFolders.first { $0.path == suggested }?.label ?? "Repo"
            Button {
                store.keepSettingsInRepo(project.slug, folder: suggested)
            } label: {
                Text("\(label)  —  \(suggested.abbreviatingHome)  (suggested)")
            }
            if !rest.isEmpty { Divider() }
        }
        ForEach(rest) { folder in
            Button {
                store.keepSettingsInRepo(project.slug, folder: folder.path)
            } label: {
                Text("\(folder.label)  —  \(folder.path.abbreviatingHome)")
            }
        }
        Divider()
        Button("Other Folder…") { store.keepSettingsInRepo(project.slug) }
    }
}

struct SidebarFoot: View {
    @EnvironmentObject var store: EngineStore
    @EnvironmentObject var context: WindowContext
    @Environment(\.sbTheme) private var theme
    @Environment(\.colorScheme) private var scheme
    @Environment(\.openWindow) private var openWindow
    @Environment(\.dismissWindow) private var dismissWindow

    var body: some View {
        let p = theme.palette(scheme)
        let away = store.openProjectWindows.sorted()
        VStack(spacing: 0) {
            if context.scope == .all, !away.isEmpty {
                Menu {
                    ForEach(away, id: \.self) { slug in
                        let name = store.project(slug)?.name ?? slug
                        Button("Show \(name) window") { openWindow(id: "project", value: slug) }
                        Button("Bring \(name) back") { dismissWindow(id: "project", value: slug) }
                        Divider()
                    }
                } label: {
                    HStack(spacing: 6) {
                        Image(systemName: "macwindow.on.rectangle").font(.system(size: 11))
                        Text(away.count == 1 ? "1 project in its own window" : "\(away.count) projects in their own windows").font(SBType.secondary)
                        Spacer()
                        Image(systemName: "chevron.down").font(.system(size: 8, weight: .bold))
                    }
                    .foregroundStyle(p.text3)
                    .padding(.horizontal, 14).frame(height: 28)
                }
                .menuStyle(.borderlessButton).menuIndicator(.hidden)
                Rectangle().fill(p.line).frame(height: 1)
            }
            HStack(spacing: 6) {
                Circle().fill(store.totalRunning > 0 ? p.running : p.stopped).frame(width: 6, height: 6)
                Text("\(store.totalRunning) running").foregroundStyle(p.text2)
                if store.totalCrashed > 0 {
                    Text("·").foregroundStyle(p.text4)
                    Text("\(store.totalCrashed) crashed").foregroundStyle(p.crashed)
                }
                Spacer()
                if store.totalRunning > 0 {
                    Button("Stop all") { store.engine.stopEverything() }.buttonStyle(.plain).foregroundStyle(p.text3)
                }
            }
            .font(SBType.secondary)
            .padding(.horizontal, 14).frame(height: 30)
        }
        .background(p.sidebar)
        .overlay(alignment: .top) { Rectangle().fill(p.line).frame(height: 1) }
    }
}

/// Every project is open in its own window; the pane area explains and offers Bring Back.
struct AllExtractedSidebar: View {
    @Environment(\.sbPalette) private var p
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("Board is clear").font(.system(size: 13, weight: .semibold)).foregroundStyle(p.text)
            Text("Every project is in its own window. Close one, or bring it back, to see it here.")
                .font(SBType.secondary).foregroundStyle(p.text3).fixedSize(horizontal: false, vertical: true)
        }
        .padding(12)
    }
}

struct EmptySidebar: View {
    @EnvironmentObject var store: EngineStore
    @Environment(\.sbPalette) private var p
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("No projects yet").font(.system(size: 13, weight: .semibold)).foregroundStyle(p.text)
            Text("Drop a folder containing config.yaml here, or use Add Project… (⌘O).")
                .font(SBType.secondary).foregroundStyle(p.text3).fixedSize(horizontal: false, vertical: true)
            Button("Add Project…") { store.addProjectViaPanel() }.buttonStyle(.plain).foregroundStyle(p.accentSoftText).font(SBType.secondary)
        }
        .padding(12)
    }
}

// MARK: - Bits

/// A row or tile that accepts a dragged sibling: shows an accent line where it will land
/// and hands the payload to `onDrop`, which returns false for payloads it does not take.
private struct ReorderTarget: ViewModifier {
    let edge: VerticalEdge
    let onDrop: (String) -> Bool
    @State private var targeted = false
    @Environment(\.sbPalette) private var p

    func body(content: Content) -> some View {
        content
            .overlay(alignment: edge == .top ? .top : .bottom) {
                if targeted {
                    Capsule().fill(p.accent).frame(height: 2).padding(.horizontal, 4).offset(y: edge == .top ? -1 : 1)
                }
            }
            .dropDestination(for: String.self) { items, _ in
                guard let first = items.first else { return false }
                return onDrop(first)
            } isTargeted: { targeted = $0 }
    }
}

extension View {
    func reorderTarget(edge: VerticalEdge, onDrop: @escaping (String) -> Bool) -> some View {
        modifier(ReorderTarget(edge: edge, onDrop: onDrop))
    }
}

struct RunControls: View {
    let anyRunning: Bool
    let start: () -> Void
    let stop: () -> Void
    let restart: () -> Void

    var body: some View {
        HStack(spacing: 2) {
            IconButton("play.fill", help: "Start all autostart commands", action: start)
            IconButton("stop.fill", help: "Stop all", action: stop).disabled(!anyRunning)
            IconButton("arrow.clockwise", help: "Restart all", action: restart)
        }
    }
}

struct IconButton: View {
    let symbol: String
    let help: String
    let action: () -> Void
    @State private var hovering = false
    @Environment(\.sbPalette) private var p

    init(_ symbol: String, help: String, action: @escaping () -> Void) {
        self.symbol = symbol; self.help = help; self.action = action
    }

    var body: some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: 10, weight: .semibold))
                .frame(width: 20, height: 20)
                .background(hovering ? p.hover : .clear, in: RoundedRectangle(cornerRadius: 5))
        }
        .buttonStyle(.plain)
        .onHover { hovering = $0 }
        .help(help)
    }
}

struct SmallButton: View {
    let label: String
    var icon: String? = nil
    var primary = false
    let action: () -> Void
    @Environment(\.sbPalette) private var p

    var body: some View {
        Button(action: action) {
            HStack(spacing: 5) {
                if let icon { Image(systemName: icon).font(.system(size: 8, weight: .bold)) }
                Text(label).font(.system(size: 12, weight: primary ? .semibold : .regular))
            }
            .foregroundStyle(primary ? p.onAccent : p.text)
            .padding(.horizontal, 9).frame(height: 24)
            .background(primary ? p.accent : p.button, in: RoundedRectangle(cornerRadius: 6, style: .continuous))
            .overlay { RoundedRectangle(cornerRadius: 6, style: .continuous).strokeBorder(primary ? p.accent : p.buttonLine) }
        }
        .buttonStyle(.plain)
    }
}

/// `icon.png` when present, otherwise initials on the project's colour.
struct ProjectIcon: View {
    let project: ProjectInfo
    var size: CGFloat = 22
    var radius: CGFloat? = nil

    var body: some View {
        let r = radius ?? size * 0.22
        if let path = project.iconPath, let image = NSImage(contentsOfFile: path) {
            Image(nsImage: image)
                .resizable()
                .interpolation(.high)
                .frame(width: size, height: size)
                .clipShape(RoundedRectangle(cornerRadius: r, style: .continuous))
        } else {
            let color = project.color.flatMap(Color.init(hex:)) ?? Color.fallback(for: project.name)
            ZStack {
                RoundedRectangle(cornerRadius: r, style: .continuous).fill(color)
                Text(initials(project.name))
                    .font(.system(size: size * 0.42, weight: .bold, design: .rounded))
                    .foregroundStyle(.white)
            }
            .frame(width: size, height: size)
        }
    }

    private func initials(_ name: String) -> String {
        let words = name.split(whereSeparator: { $0 == " " || $0 == "-" || $0 == "_" })
        let letters = words.prefix(2).compactMap { $0.first }.map { String($0).uppercased() }
        return letters.isEmpty ? "?" : letters.joined()
    }
}

extension CommandRef {
    /// Transferable form for dragging rows onto panes.
    var dragString: String { "sbcmd:\(project)|\(address)" }
    static func fromDragString(_ s: String) -> CommandRef? {
        guard s.hasPrefix("sbcmd:") else { return nil }
        let parts = s.dropFirst(6).split(separator: "|", maxSplits: 1).map(String.init)
        guard parts.count == 2 else { return nil }
        return CommandRef(project: parts[0], address: parts[1])
    }
}
