import SwiftUI

/// Renders a window's pane tree: splits with draggable dividers, leaves as panes.
struct PaneTreeView: View {
    @EnvironmentObject var context: WindowContext
    let layout: PaneLayout

    var body: some View {
        switch layout {
        case .leaf(let leaf):
            PaneView(leaf: leaf)
                .id(leaf.id)
        case .split(let split):
            SplitContainer(split: split)
        }
    }
}

/// Two children along an axis with a draggable divider.
struct SplitContainer: View {
    @EnvironmentObject var context: WindowContext
    let split: PaneSplit
    @State private var dragStart: Double?

    var body: some View {
        GeometryReader { geo in
            let total = split.axis == .horizontal ? geo.size.width : geo.size.height
            let firstSize = max(0, (total - dividerThickness) * split.fraction)
            let secondSize = max(0, total - firstSize - dividerThickness)
            layoutStack {
                PaneTreeView(layout: split.first)
                    .frame(width: split.axis == .horizontal ? firstSize : nil,
                           height: split.axis == .vertical ? firstSize : nil)
                divider(total: total)
                PaneTreeView(layout: split.second)
                    .frame(width: split.axis == .horizontal ? secondSize : nil,
                           height: split.axis == .vertical ? secondSize : nil)
            }
        }
    }

    @ViewBuilder
    private func layoutStack<Content: View>(@ViewBuilder _ content: () -> Content) -> some View {
        if split.axis == .horizontal {
            HStack(spacing: 0) { content() }
        } else {
            VStack(spacing: 0) { content() }
        }
    }

    /// A 6 pt strip that owns its own hit area (the terminals are AppKit views and would
    /// otherwise swallow the drag), drawn as a 1 pt line down its middle.
    private func divider(total: CGFloat) -> some View {
        ZStack {
            Color(nsColor: .windowBackgroundColor)
            Rectangle()
                .fill(Color(nsColor: .separatorColor))
                .frame(width: split.axis == .horizontal ? 1 : nil, height: split.axis == .vertical ? 1 : nil)
        }
        .frame(width: split.axis == .horizontal ? dividerThickness : nil,
               height: split.axis == .vertical ? dividerThickness : nil)
        .contentShape(Rectangle())
        .onHover { inside in
            if inside {
                split.axis == .horizontal ? NSCursor.resizeLeftRight.push() : NSCursor.resizeUpDown.push()
            } else {
                NSCursor.pop()
            }
        }
        .gesture(
            DragGesture(minimumDistance: 1, coordinateSpace: .global)
                .onChanged { value in
                    if dragStart == nil { dragStart = split.fraction }
                    let delta = split.axis == .horizontal ? value.translation.width : value.translation.height
                    context.setFraction((dragStart ?? split.fraction) + Double(delta / total), of: split.id)
                }
                .onEnded { _ in dragStart = nil }
        )
    }
}

private let dividerThickness: CGFloat = 6

/// One pane: header (identity + picker + split/close), terminal or placeholder, footer.
struct PaneView: View {
    @EnvironmentObject var store: EngineStore
    @EnvironmentObject var context: WindowContext
    let leaf: PaneLeaf

    private var focused: Bool { context.focusedPane == leaf.id }
    private var multi: Bool { context.layout.leafCount > 1 }

    var body: some View {
        VStack(spacing: 0) {
            if multi {
                PaneHeader(leaf: leaf, focused: focused)
                Divider()
            }
            if let ref = leaf.ref {
                PaneBody(leaf: leaf, ref: ref)
            } else {
                EmptyPane(leaf: leaf)
            }
        }
        .overlay {
            if multi && focused {
                Rectangle().strokeBorder(Color.accentColor.opacity(0.6), lineWidth: 1).allowsHitTesting(false)
            }
        }
        .contentShape(Rectangle())
        .onTapGesture { context.setFocus(leaf.id) }
    }
}

/// The terminal (or a placeholder when another pane/window has it) plus the footer.
struct PaneBody: View {
    @EnvironmentObject var store: EngineStore
    @EnvironmentObject var context: WindowContext
    let leaf: PaneLeaf
    let ref: CommandRef
    /// Identifies this view instance's claim on the terminal.
    @State private var token = UUID()

    private var ownedHere: Bool { store.paneOwners[ref]?.token == token }

    var body: some View {
        let state = store.state(ref)
        let command = store.command(ref)
        VStack(spacing: 0) {
            if ownedHere {
                if !state.isAlive && state.status == .stopped && !TerminalRegistry.shared.hasStarted(ref) {
                    idleBanner(command: command)
                }
                TerminalContainer(ref: ref) { context.setFocus(leaf.id) }
            } else {
                shownElsewhere(command: command)
            }
            Divider()
            PaneFooter(ref: ref, command: command, state: state)
        }
        .onAppear { store.claim(ref, pane: leaf.id, window: context, token: token) }
        .onDisappear { store.release(ref, token: token) }
        .onChange(of: store.paneOwners[ref]) { _, owner in
            // Nobody shows it any more (its pane closed elsewhere): take it.
            if owner == nil { store.claim(ref, pane: leaf.id, window: context, token: token) }
        }
    }

    @ViewBuilder
    private func idleBanner(command: CommandInfo?) -> some View {
        if let command {
            HStack(spacing: 10) {
                Image(systemName: "terminal").foregroundStyle(.secondary)
                VStack(alignment: .leading, spacing: 2) {
                    Text(command.run).font(.system(.body, design: .monospaced)).lineLimit(1)
                    Text(command.folderLabel).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                }
                Spacer()
                if !command.cwdExists {
                    Label("Folder not found", systemImage: "exclamationmark.triangle.fill")
                        .foregroundStyle(.orange).font(.caption)
                }
                Button("Start") { store.start(ref) }
                    .disabled(!command.cwdExists)
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 10)
            .background(.bar)
            Divider()
        }
    }

    private func shownElsewhere(command: CommandInfo?) -> some View {
        let owner = store.paneOwners[ref]
        let whereText = owner?.window == context.id ? "another pane" : "the \(store.ownerName(of: ref) ?? "other") window"
        return ContentUnavailableView {
            Label(command?.displayName ?? ref.address, systemImage: "macwindow.on.rectangle")
        } description: {
            Text("Showing in \(whereText).")
        } actions: {
            Button("Show Here") {
                if owner?.window == context.id, let pane = owner?.pane {
                    context.show(nil, in: pane)
                }
                store.claim(ref, pane: leaf.id, window: context, token: token)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

/// A pane with no command yet.
struct EmptyPane: View {
    @EnvironmentObject var store: EngineStore
    @EnvironmentObject var context: WindowContext
    let leaf: PaneLeaf

    var body: some View {
        ContentUnavailableView {
            Label("Empty pane", systemImage: "rectangle.dashed")
        } description: {
            Text("Pick a command, or select one in the sidebar.")
        } actions: {
            CommandPicker(leaf: leaf) {
                Label("Choose Command…", systemImage: "chevron.down")
            }
            if context.canClosePane {
                Button("Close Pane") { context.closePane(leaf.id) }
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

/// Menu of every command visible in this window, grouped by project and group.
struct CommandPicker<MenuLabel: View>: View {
    @EnvironmentObject var store: EngineStore
    @EnvironmentObject var context: WindowContext
    let leaf: PaneLeaf
    @ViewBuilder let label: () -> MenuLabel

    var body: some View {
        Menu {
            ForEach(context.projects(in: store).filter(\.isValid), id: \.slug) { project in
                Section(project.name) {
                    ForEach(project.commands, id: \.address) { cmd in
                        item(project: project, cmd: cmd)
                    }
                    ForEach(project.groups, id: \.id) { group in
                        Menu(group.name) {
                            ForEach(group.commands, id: \.address) { cmd in
                                item(project: project, cmd: cmd)
                            }
                        }
                    }
                }
            }
        } label: { label() }
    }

    private func item(project: ProjectInfo, cmd: CommandInfo) -> some View {
        let ref = CommandRef(project: project.slug, address: cmd.address)
        let state = store.state(ref)
        return Button {
            context.show(ref, in: leaf.id)
        } label: {
            Label {
                Text(cmd.name)
            } icon: {
                Image(systemName: state.isAlive ? "circle.fill" : "circle")
            }
        }
    }
}

/// Identity + picker + actions above each pane when the window is split.
struct PaneHeader: View {
    @EnvironmentObject var store: EngineStore
    @EnvironmentObject var context: WindowContext
    let leaf: PaneLeaf
    let focused: Bool

    var body: some View {
        HStack(spacing: 8) {
            if let ref = leaf.ref {
                let state = store.state(ref)
                let cmd = store.command(ref)
                StatusDot(status: state.status, size: 8)
                CommandPicker(leaf: leaf) {
                    HStack(spacing: 4) {
                        Text(cmd?.displayName ?? ref.address).font(.callout.weight(focused ? .semibold : .regular)).lineLimit(1)
                        Image(systemName: "chevron.down").font(.system(size: 8, weight: .bold)).foregroundStyle(.secondary)
                    }
                }
                .menuStyle(.borderlessButton)
                .menuIndicator(.hidden)
                .fixedSize()
                if context.scope == .all, let p = store.project(ref.project) {
                    Text(p.name).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                }
            } else {
                Text("Empty pane").font(.callout).foregroundStyle(.secondary)
            }
            Spacer(minLength: 4)
            HStack(spacing: 2) {
                IconButton("rectangle.split.2x1", help: "Split Right (⌘D)") { context.split(leaf.id, axis: .horizontal) }
                    .disabled(!context.canSplit)
                IconButton("rectangle.split.1x2", help: "Split Down (⇧⌘D)") { context.split(leaf.id, axis: .vertical) }
                    .disabled(!context.canSplit)
                IconButton("xmark", help: "Close Pane (⇧⌘W)") { context.closePane(leaf.id) }
            }
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 4)
        .background(focused ? Color.accentColor.opacity(0.08) : Color.clear)
        .background(.bar)
        .contentShape(Rectangle())
        .onTapGesture { context.setFocus(leaf.id) }
    }
}
