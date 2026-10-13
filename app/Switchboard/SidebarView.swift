import SwiftUI
import UniformTypeIdentifiers

/// Projects → groups → commands, with status dots and hover actions. Accepts dropped folders.
struct SidebarView: View {
    @EnvironmentObject var store: EngineStore
    @State private var dropTargeted = false
    @State private var collapsedProjects: Set<String> = []
    @State private var collapsedGroups: Set<String> = []

    var body: some View {
        List(selection: $store.selection) {
            if store.projects.isEmpty {
                emptyState
            }
            ForEach(store.projects, id: \.slug) { project in
                if let error = project.error {
                    ProjectErrorRow(project: project, error: error)
                } else {
                    projectSection(project)
                }
            }
        }
        .listStyle(.sidebar)
        .overlay {
            if dropTargeted {
                RoundedRectangle(cornerRadius: 8)
                    .strokeBorder(Color.accentColor, lineWidth: 2)
                    .background(Color.accentColor.opacity(0.08))
                    .padding(4)
                    .allowsHitTesting(false)
            }
        }
        .dropDestination(for: URL.self) { urls, _ in
            var added = false
            for url in urls where url.isFileURL {
                if store.addProject(from: url) { added = true }
            }
            return added
        } isTargeted: { dropTargeted = $0 }
        .onChange(of: store.selection) { _, _ in store.persistSelection() }
    }

    private var emptyState: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("No projects yet").font(.headline)
            Text("Drop a folder containing config.yaml here, or use Add Project… (⌘O).")
                .font(.callout)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            Button("Add Project…") { store.addProjectViaPanel() }
                .buttonStyle(.link)
        }
        .padding(.vertical, 8)
        .selectionDisabled()
    }

    @ViewBuilder
    private func projectSection(_ project: ProjectInfo) -> some View {
        DisclosureGroup(isExpanded: expandedBinding(for: project.slug, in: $collapsedProjects)) {
            ForEach(project.commands, id: \.address) { cmd in
                CommandRow(project: project.slug, command: cmd)
            }
            ForEach(project.groups, id: \.id) { group in
                DisclosureGroup(isExpanded: expandedBinding(for: "\(project.slug)/\(group.id)", in: $collapsedGroups)) {
                    ForEach(group.commands, id: \.address) { cmd in
                        CommandRow(project: project.slug, command: cmd)
                    }
                } label: {
                    GroupRow(project: project.slug, group: group)
                }
            }
        } label: {
            ProjectRow(project: project)
        }
    }

    private func expandedBinding(for key: String, in set: Binding<Set<String>>) -> Binding<Bool> {
        Binding(
            get: { !set.wrappedValue.contains(key) },
            set: { expanded in
                if expanded { set.wrappedValue.remove(key) } else { set.wrappedValue.insert(key) }
            }
        )
    }
}

// MARK: - Rows

struct ProjectRow: View {
    @EnvironmentObject var store: EngineStore
    let project: ProjectInfo
    @State private var hovering = false

    var body: some View {
        let running = store.runningCount(project: project.slug)
        HStack(spacing: 8) {
            ProjectIcon(project: project, size: 22)
            Text(project.name).fontWeight(.semibold).lineLimit(1)
            if running > 0 {
                Text("\(running)")
                    .font(.caption2.monospacedDigit())
                    .padding(.horizontal, 5)
                    .padding(.vertical, 1)
                    .background(Capsule().fill(Color.green.opacity(0.2)))
                    .foregroundStyle(.green)
            }
            Spacer(minLength: 4)
            if hovering {
                RunControls(
                    anyRunning: running > 0,
                    start: { store.startAll(project: project.slug) },
                    stop: { store.stopAll(project: project.slug) },
                    restart: { store.restartAll(project: project.slug) }
                )
            }
        }
        .frame(height: 24)
        .contentShape(Rectangle())
        .onHover { hovering = $0 }
        .contextMenu { projectMenu }
        .help(project.dir.abbreviatingHome)
    }

    @ViewBuilder
    private var projectMenu: some View {
        Button("Start All") { store.startAll(project: project.slug) }
        Button("Stop All") { store.stopAll(project: project.slug) }
        Button("Restart All") { store.restartAll(project: project.slug) }
        Divider()
        Button("Edit Config") { store.editConfig(project.slug) }
        Button("Reveal in Finder") { store.revealInFinder(project.slug) }
        Divider()
        Button("Delete Project…") { store.deleteProject(project.slug) }
    }
}

struct ProjectErrorRow: View {
    @EnvironmentObject var store: EngineStore
    let project: ProjectInfo
    let error: String

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 8) {
                Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(.orange)
                Text(project.name).fontWeight(.semibold)
                Spacer()
                Button("Edit Config") { store.editConfig(project.slug) }
                    .buttonStyle(.link)
                    .font(.caption)
            }
            Text(error)
                .font(.caption)
                .foregroundStyle(.secondary)
                .lineLimit(6)
                .fixedSize(horizontal: false, vertical: true)
                .textSelection(.enabled)
        }
        .padding(.vertical, 4)
        .contextMenu {
            Button("Edit Config") { store.editConfig(project.slug) }
            Button("Reveal in Finder") { store.revealInFinder(project.slug) }
            Divider()
            Button("Delete Project…") { store.deleteProject(project.slug) }
        }
    }
}

struct GroupRow: View {
    @EnvironmentObject var store: EngineStore
    let project: String
    let group: GroupInfo
    @State private var hovering = false

    var body: some View {
        let running = store.runningCount(project: project, group: group.id)
        HStack(spacing: 6) {
            Text(group.name.uppercased())
                .font(.caption.weight(.semibold))
                .foregroundStyle(.secondary)
            Spacer(minLength: 4)
            if hovering {
                RunControls(
                    anyRunning: running > 0,
                    start: { store.startAll(project: project, group: group.id) },
                    stop: { store.stopAll(project: project, group: group.id) },
                    restart: { store.restartAll(project: project, group: group.id) }
                )
            }
        }
        .frame(height: 20)
        .contentShape(Rectangle())
        .onHover { hovering = $0 }
        .help(group.cwd ?? "")
        .contextMenu {
            Button("Start All") { store.startAll(project: project, group: group.id) }
            Button("Stop All") { store.stopAll(project: project, group: group.id) }
            Button("Restart All") { store.restartAll(project: project, group: group.id) }
        }
    }
}

struct CommandRow: View {
    @EnvironmentObject var store: EngineStore
    let project: String
    let command: CommandInfo
    @State private var hovering = false

    private var ref: CommandRef { CommandRef(project: project, address: command.address) }

    var body: some View {
        let state = store.state(ref)
        HStack(spacing: 8) {
            StatusDot(status: state.status)
            Text(command.name).lineLimit(1)
            if !command.cwdExists {
                Image(systemName: "folder.badge.questionmark")
                    .foregroundStyle(.orange)
                    .help("Folder not found: \(command.cwd ?? "")")
            }
            if state.needsRestart {
                Image(systemName: "arrow.clockwise.circle")
                    .foregroundStyle(.secondary)
                    .help("Config changed; restart to apply")
            }
            Spacer(minLength: 4)
            if hovering {
                HStack(spacing: 2) {
                    if state.isAlive {
                        IconButton("stop.fill", help: "Stop") { store.stop(ref) }
                        IconButton("arrow.clockwise", help: "Restart") { store.restart(ref) }
                    } else {
                        IconButton("play.fill", help: "Start") { store.start(ref) }
                            .disabled(!command.cwdExists)
                    }
                }
            }
        }
        .frame(height: 22)
        .contentShape(Rectangle())
        .onHover { hovering = $0 }
        .tag(ref)
        .help(state.summary)
        .contextMenu {
            if state.isAlive {
                Button("Stop") { store.stop(ref) }
                Button("Restart") { store.restart(ref) }
            } else {
                Button("Start") { store.start(ref) }.disabled(!command.cwdExists)
            }
            Divider()
            Text(command.run).font(.system(.caption, design: .monospaced))
            if let path = command.cwdPath {
                Button("Reveal Folder in Finder") { store.revealFolder(path) }.disabled(!command.cwdExists)
            }
        }
    }
}

// MARK: - Bits

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

    init(_ symbol: String, help: String, action: @escaping () -> Void) {
        self.symbol = symbol; self.help = help; self.action = action
    }

    var body: some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: 10, weight: .semibold))
                .frame(width: 18, height: 18)
        }
        .buttonStyle(.borderless)
        .help(help)
    }
}

/// `icon.png` when present, otherwise initials on the project's color.
struct ProjectIcon: View {
    let project: ProjectInfo
    var size: CGFloat = 22

    var body: some View {
        if let path = project.iconPath, let image = NSImage(contentsOfFile: path) {
            Image(nsImage: image)
                .resizable()
                .interpolation(.high)
                .frame(width: size, height: size)
                .clipShape(RoundedRectangle(cornerRadius: size * 0.22, style: .continuous))
        } else {
            let color = project.color.flatMap(Color.init(hex:)) ?? Color.fallback(for: project.name)
            ZStack {
                RoundedRectangle(cornerRadius: size * 0.22, style: .continuous).fill(color)
                Text(initials(project.name))
                    .font(.system(size: size * 0.45, weight: .bold, design: .rounded))
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
