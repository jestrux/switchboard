import SwiftUI
import UniformTypeIdentifiers

/// Renders a window's pane tree: splits with draggable dividers, leaves as panes.
struct PaneTreeView: View {
    @EnvironmentObject var context: WindowContext
    let layout: PaneLayout

    var body: some View {
        if let max = context.maximizedPane, let leaf = layout.leaf(max) {
            PaneView(leaf: leaf).id(leaf.id)
        } else {
            tree(layout)
        }
    }

    @ViewBuilder
    private func tree(_ layout: PaneLayout) -> some View {
        switch layout {
        case .leaf(let leaf):
            PaneView(leaf: leaf).id(leaf.id)
        case .split(let split):
            SplitContainer(split: split)
        }
    }
}

private let dividerThickness: CGFloat = SBMetrics.dividerHit

/// Two children along an axis with a draggable divider (1 pt line, 7 pt hit area).
struct SplitContainer: View {
    @EnvironmentObject var context: WindowContext
    @Environment(\.sbPalette) private var p
    let split: PaneSplit
    @State private var dragStart: Double?
    @State private var hovering = false

    var body: some View {
        GeometryReader { geo in
            let total = split.axis == .horizontal ? geo.size.width : geo.size.height
            let firstSize = max(0, (total - dividerThickness) * split.fraction)
            let secondSize = max(0, total - firstSize - dividerThickness)
            layoutStack {
                PaneTreeView(layout: split.first)
                    .frame(width: split.axis == .horizontal ? firstSize : nil, height: split.axis == .vertical ? firstSize : nil)
                divider(total: total)
                PaneTreeView(layout: split.second)
                    .frame(width: split.axis == .horizontal ? secondSize : nil, height: split.axis == .vertical ? secondSize : nil)
            }
        }
    }

    @ViewBuilder
    private func layoutStack<Content: View>(@ViewBuilder _ content: () -> Content) -> some View {
        if split.axis == .horizontal { HStack(spacing: 0) { content() } } else { VStack(spacing: 0) { content() } }
    }

    private func divider(total: CGFloat) -> some View {
        ZStack {
            Color.clear
            Rectangle()
                .fill(hovering || dragStart != nil ? p.accent : p.line)
                .frame(width: split.axis == .horizontal ? 1 : nil, height: split.axis == .vertical ? 1 : nil)
        }
        .frame(width: split.axis == .horizontal ? dividerThickness : nil, height: split.axis == .vertical ? dividerThickness : nil)
        .contentShape(Rectangle())
        .onHover { inside in
            hovering = inside
            if inside { split.axis == .horizontal ? NSCursor.resizeLeftRight.push() : NSCursor.resizeUpDown.push() } else { NSCursor.pop() }
        }
        .gesture(
            DragGesture(minimumDistance: 1, coordinateSpace: .global)
                .onChanged { value in
                    if dragStart == nil { dragStart = split.fraction }
                    let delta = split.axis == .horizontal ? value.translation.width : value.translation.height
                    var f = (dragStart ?? split.fraction) + Double(delta / total)
                    if abs(f - 0.5) < 0.02 { f = 0.5 } // snap at half
                    context.setFraction(f, of: split.id)
                }
                .onEnded { _ in dragStart = nil }
        )
        .onTapGesture(count: 2) { context.setFraction(0.5, of: split.id) }
    }
}

/// Width tiers: full ≥ 560 · mid 360–559 · min below.
enum PaneTier { case full, mid, min }

/// One pane: 36 pt bar + terminal (or card / picker / placeholder) + 22 pt footer.
struct PaneView: View {
    @EnvironmentObject var store: EngineStore
    @EnvironmentObject var context: WindowContext
    @Environment(\.sbTheme) private var theme
    @Environment(\.colorScheme) private var scheme
    let leaf: PaneLeaf
    @State private var hoveringBar = false

    private var focused: Bool { context.focusedPane == leaf.id }

    var body: some View {
        let look = Look(theme: theme, scheme: scheme)
        let p = look.p
        GeometryReader { geo in
            let tier: PaneTier = geo.size.width >= SBMetrics.paneFullWidth ? .full : (geo.size.width >= SBMetrics.paneMidWidth ? .mid : .min)
            VStack(spacing: 0) {
                PaneBar(leaf: leaf, tier: tier, focused: focused, hovering: hoveringBar)
                    .onHover { hoveringBar = $0 }
                if let ref = leaf.ref, let state = Optional(store.state(ref)), state.status == .crashed, let at = state.restartAt, let total = backoffTotal(state) {
                    BackoffBar(restartAt: at, total: total)
                }
                if let ref = leaf.ref {
                    PaneBody(leaf: leaf, ref: ref, tier: tier)
                } else {
                    EmptyPane(leaf: leaf, tier: tier)
                }
            }
            .background(p.pane)
            .clipShape(RoundedRectangle(cornerRadius: look.s.paneRadius, style: .continuous))
            .overlay {
                if look.s.paneInset > 0 {
                    RoundedRectangle(cornerRadius: look.s.paneRadius, style: .continuous).strokeBorder(p.line)
                }
            }
            .shadow(color: look.s.paneInset > 0 ? .black.opacity(look.isDark ? 0.3 : 0.05) : .clear, radius: 12, y: 6)
            .padding(look.s.paneInset)
            .onAppear { context.paneSizes[leaf.id] = geo.size }
            .onChange(of: geo.size) { _, size in context.paneSizes[leaf.id] = size }
        }
        // Drops land on the pane itself (the terminal NSView is not a drag destination, so
        // AppKit passes drags up to this view); the zone overlay only draws, never hit-tests.
        .onDrop(of: [.plainText, .utf8PlainText, .text], delegate: PaneDropDelegate(leaf: leaf, context: context, zone: $dropZone, size: paneSize))
        .overlay { DropZones(leaf: leaf, zone: $dropZone, size: paneSize).allowsHitTesting(false) }
        .background(GeometryReader { g in Color.clear.onAppear { paneSize = g.size }.onChange(of: g.size) { _, v in paneSize = v } })
    }

    @State private var dropZone: DropZone?
    @State private var paneSize: CGSize = .zero
}

/// Where a dragged command would land: the centre replaces, an edge splits on that side.
enum DropZone: Equatable { case center, left, right, top, bottom }

/// The highlight for the drop zone under the cursor (drawn only; never hit-tested).
struct DropZones: View {
    @Environment(\.sbTheme) private var theme
    @Environment(\.colorScheme) private var scheme
    let leaf: PaneLeaf
    @Binding var zone: DropZone?
    let size: CGSize

    var body: some View {
        let p = theme.palette(scheme)
        ZStack {
            // Highlight of the zone the drop would use.
            if let zone {
                let r = rect(for: zone)
                RoundedRectangle(cornerRadius: 8, style: .continuous)
                    .fill(p.accentSoft.opacity(0.55))
                    .overlay { RoundedRectangle(cornerRadius: 8, style: .continuous).strokeBorder(p.accent, lineWidth: 2) }
                    .frame(width: r.width, height: r.height)
                    .position(x: r.midX, y: r.midY)
                    .overlay {
                        Text(label(zone)).font(SBType.secondary).foregroundStyle(p.text)
                            .padding(.horizontal, 10).padding(.vertical, 6)
                            .background(p.button, in: RoundedRectangle(cornerRadius: 7, style: .continuous))
                            .hairline(p.line, radius: 7)
                            .position(x: r.midX, y: r.midY)
                    }
                    .allowsHitTesting(false)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var edge: CGFloat { max(40, min(size.width, size.height) * 0.22) }

    private func rect(for z: DropZone) -> CGRect {
        switch z {
        case .center: return CGRect(x: edge, y: edge, width: size.width - 2 * edge, height: size.height - 2 * edge)
        case .left: return CGRect(x: 0, y: 0, width: size.width / 2, height: size.height)
        case .right: return CGRect(x: size.width / 2, y: 0, width: size.width / 2, height: size.height)
        case .top: return CGRect(x: 0, y: 0, width: size.width, height: size.height / 2)
        case .bottom: return CGRect(x: 0, y: size.height / 2, width: size.width, height: size.height / 2)
        }
    }

    private func label(_ z: DropZone) -> String {
        switch z {
        case .center: return leaf.ref == nil ? "Show here" : "Replace this pane"
        case .left: return "Split left"
        case .right: return "Split right"
        case .top: return "Split up"
        case .bottom: return "Split down"
        }
    }
}

/// Tracks the cursor over a pane during a drag and performs replace or split on drop.
struct PaneDropDelegate: DropDelegate {
    let leaf: PaneLeaf
    let context: WindowContext
    @Binding var zone: DropZone?
    let size: CGSize

    private func zone(at point: CGPoint) -> DropZone {
        let edge = max(40, min(size.width, size.height) * 0.22)
        if point.x < edge { return .left }
        if point.x > size.width - edge { return .right }
        if point.y < edge { return .top }
        if point.y > size.height - edge { return .bottom }
        return .center
    }

    func validateDrop(info: DropInfo) -> Bool { info.hasItemsConforming(to: [.plainText, .utf8PlainText, .text]) }
    func dropEntered(info: DropInfo) { zone = zone(at: info.location) }
    func dropUpdated(info: DropInfo) -> DropProposal? { zone = zone(at: info.location); return DropProposal(operation: .move) }
    func dropExited(info: DropInfo) { zone = nil }

    func performDrop(info: DropInfo) -> Bool {
        let target = zone(at: info.location)
        zone = nil
        guard let provider = info.itemProviders(for: [.plainText, .utf8PlainText, .text]).first else { return false }
        provider.loadItem(forTypeIdentifier: "public.utf8-plain-text", options: nil) { item, _ in
            let text: String? = (item as? String) ?? (item as? Data).flatMap { String(data: $0, encoding: .utf8) }
            guard let text, let ref = CommandRef.fromDragString(text) else { return }
            Task { @MainActor in
                switch target {
                case .center:
                    context.show(ref, in: leaf.id)
                case .left, .right:
                    context.splitAndShow(leaf.id, axis: .horizontal, newFirst: target == .left, ref: ref)
                case .top, .bottom:
                    context.splitAndShow(leaf.id, axis: .vertical, newFirst: target == .top, ref: ref)
                }
            }
        }
        return true
    }
}

/// The 36 pt bar: glyph · (project avatar) · Group / name ▾ · chips · status · actions.
struct PaneBar: View {
    @EnvironmentObject var store: EngineStore
    @EnvironmentObject var context: WindowContext
    @Environment(\.sbTheme) private var theme
    @Environment(\.colorScheme) private var scheme
    let leaf: PaneLeaf
    let tier: PaneTier
    let focused: Bool
    let hovering: Bool

    var body: some View {
        let look = Look(theme: theme, scheme: scheme)
        let p = look.p
        let ref = leaf.ref
        let state = ref.map(store.state) ?? CommandState(status: .stopped, pid: nil, exitCode: nil, signal: nil, startedAt: nil, endedAt: nil, message: nil, needsRestart: false, restartAt: nil)
        let cmd = ref.flatMap(store.command)
        let fg = focused || hovering ? p.text : p.text3
        HStack(spacing: 8) {
            if let ref {
                StatusGlyph(status: state.status, restartAt: state.restartAt, restartTotal: backoffTotal(state))
                if tier != .min, context.scope == .all || ref.project != scopeSlug, let proj = store.project(ref.project), showsProjectIcon(ref) {
                    ProjectIcon(project: proj, size: 16, radius: 4)
                }
            } else {
                EmptyGlyph()
            }
            CommandPicker(leaf: leaf) {
                HStack(spacing: 4) {
                    titleText(cmd: cmd, ref: ref, fg: fg, p: p)
                        .font(.system(size: 13, weight: focused ? .semibold : .regular))
                        .lineLimit(1)
                    Image(systemName: "chevron.down").font(.system(size: 8, weight: .bold)).foregroundStyle(p.text3)
                }
            }
            .menuStyle(.borderlessButton).menuIndicator(.hidden).fixedSize()
            .help("Choose command for this pane")
            if state.needsRestart {
                chip(tier == .full ? "↻ restart to apply" : "↻", bg: p.accentSoft, fg: p.accentSoftText)
            }
            if context.maximizedPane == leaf.id, context.layout.leafCount > 1 {
                chip(tier == .full ? "Maximized · \(context.layout.leafCount) panes" : "Max", bg: p.hover, fg: p.text2, icon: "arrow.down.right.and.arrow.up.left")
                    .help("Maximized: ⌘⇧↩ restores the split")
            }
            if tier != .min, ref != nil {
                TimelineView(.periodic(from: .now, by: 1)) { _ in
                    Text(statusText(state, cmd: cmd, full: tier == .full))
                        .font(SBType.secondary)
                        .foregroundStyle(statusColour(state, p))
                        .lineLimit(1)
                }
            }
            Spacer(minLength: 4)
            actions(ref: ref, state: state, cmd: cmd, fg: focused || hovering ? p.text2 : p.text3, p: p)
        }
        .padding(.horizontal, 10)
        .frame(height: SBMetrics.paneBar)
        .background(hovering ? p.hover : (focused ? p.button : p.sidebar))
        .overlay(alignment: .top) { if focused { Rectangle().fill(p.accent).frame(height: 2) } }
        .overlay(alignment: .bottom) { Rectangle().fill(p.line).frame(height: 1) }
        .contentShape(Rectangle())
        .onTapGesture { context.setFocus(leaf.id) }
    }

    private var scopeSlug: String? { if case .project(let s) = context.scope { return s }; return nil }

    /// `Group / name` as one Text so the menu label measures correctly.
    private func titleText(cmd: CommandInfo?, ref: CommandRef?, fg: Color, p: SBPalette) -> Text {
        guard let cmd else { return Text("Empty pane").foregroundStyle(p.text3) }
        var prefix = ""
        if tier == .full, let g = cmd.groupId, let proj = ref.flatMap({ store.project($0.project) }),
           let group = proj.groups.first(where: { $0.id == g }) {
            prefix = "\(group.name) / "
        }
        return Text(prefix).foregroundStyle(p.text3) + Text(cmd.name).foregroundStyle(fg)
    }

    /// Project icon only when the command belongs to a different project than the window's.
    private func showsProjectIcon(_ ref: CommandRef) -> Bool {
        switch context.scope {
        case .project(let slug): return ref.project != slug
        case .all: return ref.project != context.railProject
        }
    }

    private func chip(_ text: String, bg: Color, fg: Color, icon: String? = nil) -> some View {
        HStack(spacing: 4) {
            if let icon { Image(systemName: icon).font(.system(size: 8, weight: .bold)) }
            Text(text).font(.system(size: 10.5, weight: .medium))
        }
        .foregroundStyle(fg)
        .padding(.horizontal, 7).frame(height: SBMetrics.chipHeight)
        .background(bg, in: Capsule())
    }

    private func statusText(_ s: CommandState, cmd: CommandInfo?, full: Bool) -> String {
        switch s.status {
        case .running:
            let up = s.startedAt.map { Fmt.uptime(since: $0) } ?? ""
            return full ? "Running · \(up)" : up
        case .starting: return full ? "Starting" : "starting"
        case .exited: return full ? "Done · exit 0" : "✓ exit 0"
        case .crashed:
            if let at = s.restartAt {
                let left = max(0, Int(at - Date().timeIntervalSince1970))
                return full ? "Crashed · restarting in \(left) s" : "↻ \(left)s"
            }
            return full ? "Crashed · exit \(s.exitCode.map(String.init) ?? "?")" : "crashed"
        case .error: return full ? "Folder not found" : "no folder"
        case .stopped:
            if s.pid != nil { return full ? "Stopped" : "" }
            return full ? "Never run" : ""
        }
    }

    private func statusColour(_ s: CommandState, _ p: SBPalette) -> Color {
        switch s.status {
        case .starting: return p.starting
        case .crashed: return p.crashed
        case .error: return p.error
        default: return p.text3
        }
    }

    @ViewBuilder
    private func actions(ref: CommandRef?, state: CommandState, cmd: CommandInfo?, fg: Color, p: SBPalette) -> some View {
        let multi = context.layout.leafCount > 1
        HStack(spacing: 2) {
            if let ref {
                let live = state.isAlive
                // Primary action first.
                if live {
                    BarButton("stop.fill", "Stop", "Stop ⌘.", fg: fg) { store.stop(ref) }
                } else if state.status == .crashed, state.restartAt != nil {
                    BarButton("play.fill", "Start now", "Start now, skip the wait", fg: p.accentSoftText, bg: p.accentSoft) { store.start(ref) }
                } else if state.status == .error || cmd?.cwdExists == false {
                    BarButton("play.fill", "Start", "Folder not found", fg: fg) {}.opacity(0.35).disabled(true)
                } else {
                    BarButton("play.fill", "Start", "Start", fg: p.accentSoftText, bg: p.accentSoft) { store.start(ref) }
                }
                if tier == .full {
                    if state.status == .running { BarButton("arrow.clockwise", "Restart", "Restart ⌘R", fg: fg) { store.restart(ref) } }
                    if state.status == .crashed, state.restartAt != nil { BarButton("stop.fill", "Stop", "Stop, cancel the restart ⌘.", fg: fg) { store.stop(ref) } }
                    BarButton("rectangle.split.2x1", "Split right", "Split right ⌘D · ⌥-click splits down", fg: fg) {
                        context.split(leaf.id, axis: NSEvent.modifierFlags.contains(.option) ? .vertical : .horizontal)
                    }
                    if multi {
                        BarButton("arrow.up.left.and.arrow.down.right", "Maximize pane", "Maximize ⌘⇧↩", fg: fg) { context.toggleMaximize(leaf.id) }
                        BarButton("xmark", "Close pane", "Close pane ⌘⇧W", fg: fg) { context.closePane(leaf.id) }
                    }
                } else {
                    moreMenu(ref: ref, state: state, fg: fg, multi: multi)
                }
            } else if multi {
                BarButton("xmark", "Close pane", "Close pane ⌘⇧W", fg: fg) { context.closePane(leaf.id) }
            }
        }
    }

    private func moreMenu(ref: CommandRef, state: CommandState, fg: Color, multi: Bool) -> some View {
        Menu {
            if state.isAlive {
                Button("Stop") { store.stop(ref) }
                Button("Restart") { store.restart(ref) }
            } else {
                Button("Start") { store.start(ref) }
            }
            Divider()
            Button("Split Right") { context.split(leaf.id, axis: .horizontal) }
            Button("Split Down") { context.split(leaf.id, axis: .vertical) }
            if multi {
                Button(context.maximizedPane == leaf.id ? "Restore Split" : "Maximize Pane") { context.toggleMaximize(leaf.id) }
                Button("Close Pane") { context.closePane(leaf.id) }
            }
            Divider()
            Button("Clear Terminal") { TerminalRegistry.shared.existingHost(for: ref)?.clear() }
            Button("Edit Config") { store.editConfig(ref.project) }
            Button("Reveal in Finder") { store.revealInFinder(ref.project) }
        } label: {
            Image(systemName: "ellipsis").font(.system(size: 12, weight: .semibold)).foregroundStyle(fg)
                .frame(width: 24, height: 24)
        }
        .menuStyle(.borderlessButton).menuIndicator(.hidden).fixedSize()
        .help("More: restart, split, maximize, close")
    }
}

struct BarButton: View {
    let symbol: String
    let label: String
    let tip: String
    let fg: Color
    var bg: Color = .clear
    let action: () -> Void
    @State private var hovering = false
    @Environment(\.sbPalette) private var p

    init(_ symbol: String, _ label: String, _ tip: String, fg: Color, bg: Color = .clear, action: @escaping () -> Void) {
        self.symbol = symbol; self.label = label; self.tip = tip; self.fg = fg; self.bg = bg; self.action = action
    }

    var body: some View {
        Button(action: action) {
            Image(systemName: symbol).font(.system(size: 10, weight: .semibold))
                .foregroundStyle(fg)
                .frame(width: 24, height: 24)
                .background(hovering ? p.hover : bg, in: RoundedRectangle(cornerRadius: 6, style: .continuous))
        }
        .buttonStyle(.plain)
        .onHover { hovering = $0 }
        .help(tip)
        .accessibilityLabel(label)
    }
}

/// 2 pt bar under the pane bar: `crashedSoft` track, `crashed` fill drains over the delay.
struct BackoffBar: View {
    @Environment(\.sbPalette) private var p
    let restartAt: Double
    let total: Double
    var body: some View {
        TimelineView(.animation(minimumInterval: 0.1)) { ctx in
            let remaining = max(0, restartAt - ctx.date.timeIntervalSince1970)
            GeometryReader { geo in
                ZStack(alignment: .leading) {
                    p.crashedSoft
                    p.crashed.frame(width: geo.size.width * CGFloat(min(1, remaining / max(total, 0.1))))
                }
            }
            .frame(height: 2)
        }
    }
}

/// The terminal (or a placeholder when another pane/window has it) plus inline cards and footer.
struct PaneBody: View {
    @EnvironmentObject var store: EngineStore
    @EnvironmentObject var context: WindowContext
    @Environment(\.sbTheme) private var theme
    @Environment(\.colorScheme) private var scheme
    let leaf: PaneLeaf
    let ref: CommandRef
    let tier: PaneTier
    @State private var token = UUID()

    private var ownedHere: Bool { store.paneOwners[ref]?.token == token }

    var body: some View {
        let look = Look(theme: theme, scheme: scheme)
        let p = look.p
        let state = store.state(ref)
        let command = store.command(ref)
        VStack(spacing: 0) {
            if ownedHere {
                let showCard = (state.status == .stopped && state.pid == nil && !TerminalRegistry.shared.hasStarted(ref)) || state.status == .error
                ZStack(alignment: .top) {
                    p.terminal
                    TerminalContainer(ref: ref, look: look) { context.setFocus(leaf.id) }
                        .opacity(showCard ? 0 : 1)
                    if showCard, let command {
                        if state.status == .error {
                            errorCard(command, p: p, look: look)
                        } else {
                            neverRunCard(command, p: p, look: look)
                        }
                    }
                }
            } else {
                shownElsewhere(command: command, p: p)
            }
            if tier != .min {
                PaneFooter(ref: ref, state: state, command: command, tier: tier)
            }
        }
        .onAppear { store.claim(ref, pane: leaf.id, window: context, token: token) }
        .onDisappear { store.release(ref, token: token) }
        .onChange(of: store.paneOwners[ref]) { _, owner in
            if owner == nil { store.claim(ref, pane: leaf.id, window: context, token: token) }
        }
    }

    private func neverRunCard(_ command: CommandInfo, p: SBPalette, look: Look) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 10) {
                VStack(alignment: .leading, spacing: 3) {
                    HStack(spacing: 6) {
                        Text("$").foregroundStyle(p.terminalDim)
                        Text(command.run).foregroundStyle(p.text)
                    }
                    .font(.system(size: 12.5, design: .monospaced)).lineLimit(1)
                    HStack(spacing: 8) {
                        Text(command.cwd ?? "~").foregroundStyle(p.text3)
                        Text("·").foregroundStyle(p.text4)
                        Text(command.autostart ? "autostart on" : "autostart off").foregroundStyle(p.text3)
                    }
                    .font(SBType.secondary).lineLimit(1)
                }
                Spacer()
                SmallButton(label: "Start", icon: "play.fill", primary: true) { store.start(ref) }
                    .disabled(!command.cwdExists)
            }
            Text("Output appears here once it starts. It keeps running when you switch to another command.")
                .font(SBType.secondary).foregroundStyle(p.text3).fixedSize(horizontal: false, vertical: true)
        }
        .padding(14)
        .background(p.sidebar, in: RoundedRectangle(cornerRadius: look.s.cardRadius, style: .continuous))
        .hairline(p.line, radius: look.s.cardRadius)
        .padding(14)
        .frame(maxWidth: 520, alignment: .leading)
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func errorCard(_ command: CommandInfo, p: SBPalette, look: Look) -> some View {
        HStack(alignment: .top, spacing: 10) {
            TriangleGlyph().fill(p.error).frame(width: 18, height: 18)
            VStack(alignment: .leading, spacing: 4) {
                Text("This command's folder doesn't exist").font(.system(size: 13, weight: .semibold)).foregroundStyle(p.text)
                Text(command.cwd ?? "").font(SBType.meta).foregroundStyle(p.error)
                HStack(spacing: 4) {
                    Text("Fix").foregroundStyle(p.text2)
                    Text("cwd").font(SBType.meta).foregroundStyle(p.text)
                    Text("in config.yaml or create the folder. The sidebar reloads on save.").foregroundStyle(p.text2)
                }
                .font(SBType.secondary)
                SmallButton(label: "Edit Config  ⌘E") { store.editConfig(ref.project) }.padding(.top, 4)
            }
        }
        .padding(14)
        .background(p.errorSoft, in: RoundedRectangle(cornerRadius: look.s.cardRadius, style: .continuous))
        .overlay { RoundedRectangle(cornerRadius: look.s.cardRadius, style: .continuous).strokeBorder(p.error.opacity(0.5), style: StrokeStyle(lineWidth: 1, dash: look.s.dashedWarningBorder ? [4, 3] : [])) }
        .padding(14)
        .frame(maxWidth: 520, alignment: .leading)
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func shownElsewhere(command: CommandInfo?, p: SBPalette) -> some View {
        let owner = store.paneOwners[ref]
        let whereText = owner?.window == context.id ? "another pane" : "the \(store.ownerName(of: ref) ?? "other") window"
        return VStack {
            Spacer()
            HStack(spacing: 10) {
                Image(systemName: "macwindow.on.rectangle").foregroundStyle(p.text3)
                Text("Showing in \(whereText)").font(SBType.secondary).foregroundStyle(p.text3)
                SmallButton(label: "Show Here") {
                    if owner?.window == context.id, let pane = owner?.pane { context.show(nil, in: pane) }
                    store.claim(ref, pane: leaf.id, window: context, token: token)
                }
            }
            Spacer()
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(p.terminal)
    }
}

/// 22 pt mono footer: full = folder · pid · uptime (or exit · ran); mid drops the folder.
struct PaneFooter: View {
    @Environment(\.sbPalette) private var p
    let ref: CommandRef
    let state: CommandState
    let command: CommandInfo?
    let tier: PaneTier

    var body: some View {
        TimelineView(.periodic(from: .now, by: 1)) { ctx in
            HStack(spacing: 14) {
                if tier == .full, state.status != .error {
                    Text(Fmt.shortFolder(command?.cwd)).foregroundStyle(p.text3).lineLimit(1).truncationMode(.middle)
                }
                ForEach(items(now: ctx.date), id: \.0) { k, v, tint in
                    HStack(spacing: 4) {
                        if !k.isEmpty { Text(k).foregroundStyle(p.text3) }
                        Text(v).foregroundStyle(tint ?? p.text2)
                    }
                }
                Spacer()
                Text(right).foregroundStyle(p.text3)
            }
            .font(SBType.footer).monospacedDigit()
            .padding(.horizontal, 14)
            .frame(height: SBMetrics.paneFooter)
            .background(p.pane)
            .overlay(alignment: .top) { Rectangle().fill(p.line).frame(height: 1) }
        }
    }

    private var right: String {
        switch state.status {
        case .stopped where state.pid == nil: return command.map { $0.autostart ? "autostart · on" : "autostart · off" } ?? ""
        case .crashed: return command?.restartOnCrash == true ? "restart on crash · on" : "restart on crash · off"
        default: return ""
        }
    }

    private func items(now: Date) -> [(String, String, Color?)] {
        switch state.status {
        case .running:
            return [("pid", state.pid.map(String.init) ?? "", nil), ("up", state.startedAt.map { Fmt.uptime(since: $0, now: now) } ?? "", nil)]
        case .starting:
            return [("pid", state.pid.map(String.init) ?? "", nil), ("", "starting", p.starting)]
        case .exited:
            var out: [(String, String, Color?)] = []
            if let e = state.endedAt { out.append(("finished", Fmt.clock(e), nil)) }
            if let s = state.startedAt, let e = state.endedAt { out.append(("ran", Fmt.duration(e - s), nil)) }
            out.append(("exit", "0", nil))
            return out
        case .crashed:
            var out: [(String, String, Color?)] = [("last pid", state.pid.map(String.init) ?? "", nil)]
            if let s = state.startedAt, let e = state.endedAt { out.append(("ran", Fmt.duration(e - s), nil)) }
            out.append(("exit", state.signal ?? state.exitCode.map(String.init) ?? "?", p.crashed))
            return out
        case .error:
            return [("", "cwd not found", p.error)]
        case .stopped:
            if state.pid == nil { return [("", "not started", nil)] }
            var out: [(String, String, Color?)] = [("last pid", state.pid.map(String.init) ?? "", nil)]
            if let s = state.startedAt, let e = state.endedAt { out.append(("ran", Fmt.duration(e - s), nil)) }
            out.append(("", "stopped", nil))
            return out
        }
    }
}

/// A pane with no command yet: "Show a command" picker with a filter.
struct EmptyPane: View {
    @EnvironmentObject var store: EngineStore
    @EnvironmentObject var context: WindowContext
    @Environment(\.sbTheme) private var theme
    @Environment(\.colorScheme) private var scheme
    let leaf: PaneLeaf
    let tier: PaneTier
    @State private var filter = ""
    @State private var highlighted = 0
    @FocusState private var filterFocused: Bool

    private var candidates: [(ProjectInfo, CommandInfo)] {
        context.projects(in: store).filter(\.isValid).flatMap { p in p.allCommands.map { (p, $0) } }
            .filter { filter.isEmpty || $0.1.displayName.localizedCaseInsensitiveContains(filter) || $0.0.name.localizedCaseInsensitiveContains(filter) }
    }

    var body: some View {
        let look = Look(theme: theme, scheme: scheme)
        let p = look.p
        let list = candidates
        VStack {
            Spacer(minLength: 20)
            VStack(alignment: .leading, spacing: 0) {
                Text("Show a command").font(.system(size: 13, weight: .semibold)).foregroundStyle(p.text).padding(.bottom, 8)
                HStack(spacing: 6) {
                    Image(systemName: "magnifyingglass").font(.system(size: 11)).foregroundStyle(p.text3)
                    TextField("Filter", text: $filter)
                        .textFieldStyle(.plain).font(SBType.body).foregroundStyle(p.text)
                        .focused($filterFocused)
                        .onSubmit { pick(highlighted) }
                        .onKeyPress(.downArrow) { highlighted = min(highlighted + 1, max(0, list.count - 1)); return .handled }
                        .onKeyPress(.upArrow) { highlighted = max(0, highlighted - 1); return .handled }
                        .onKeyPress(.escape) { if context.canClosePane { context.closePane(leaf.id) }; return .handled }
                }
                .padding(.horizontal, 10).frame(height: 30)
                .background(p.sidebar, in: RoundedRectangle(cornerRadius: 7, style: .continuous))
                .hairline(p.line, radius: 7)
                ScrollView {
                    VStack(spacing: 1) {
                        ForEach(Array(list.enumerated()), id: \.offset) { i, item in
                            let (proj, cmd) = item
                            let ref = CommandRef(project: proj.slug, address: cmd.address)
                            let st = store.state(ref)
                            let paneNo = context.paneNumber(of: ref)
                            HStack(spacing: 8) {
                                Circle().fill(st.isAlive ? p.running : (st.status == .crashed ? p.crashed : .clear))
                                    .overlay(Circle().strokeBorder(st.isAlive || st.status == .crashed ? .clear : p.stopped, lineWidth: 1.5))
                                    .frame(width: 7, height: 7)
                                HStack(spacing: 0) {
                                    if tier != .min, let g = cmd.groupId, let group = proj.groups.first(where: { $0.id == g }) {
                                        Text("\(group.name) / ").foregroundStyle(p.text3)
                                    }
                                    Text(cmd.name).foregroundStyle(paneNo != nil ? p.text3 : p.text)
                                    if context.scope == .all, proj.slug != context.railProject {
                                        Text("  \(proj.name)").foregroundStyle(p.text4)
                                    }
                                }
                                .font(SBType.body).lineLimit(1)
                                Spacer()
                                Text(paneNo.map { "in pane \($0)" } ?? MetaLabel.short(st)).font(SBType.meta).foregroundStyle(p.text3)
                            }
                            .padding(.horizontal, 10).frame(height: 28)
                            .background(i == highlighted ? p.accentSoft : .clear, in: RoundedRectangle(cornerRadius: 6, style: .continuous))
                            .contentShape(Rectangle())
                            .onTapGesture { pick(i) }
                            .onHover { if $0 { highlighted = i } }
                        }
                    }
                    .padding(.top, 6)
                }
                .frame(maxHeight: 240)
                if tier != .min {
                    Text("↩ to show · or drag a command from the sidebar").font(SBType.secondary).foregroundStyle(p.text4).padding(.top, 8)
                }
            }
            .padding(14)
            .frame(width: min(420, max(240, 420)))
            .background(p.pane, in: RoundedRectangle(cornerRadius: look.s.cardRadius, style: .continuous))
            .hairline(p.line, radius: look.s.cardRadius)
            Spacer()
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(p.terminal)
        .onAppear { if context.focusedPane == leaf.id { filterFocused = true } }
        .onChange(of: context.focusedPane) { _, f in if f != leaf.id { filterFocused = false } }
    }

    private func pick(_ i: Int) {
        let list = candidates
        guard list.indices.contains(i) else { return }
        context.show(CommandRef(project: list[i].0.slug, address: list[i].1.address), in: leaf.id)
    }
}

enum MetaLabel {
    static func short(_ s: CommandState) -> String {
        switch s.status {
        case .running: return s.startedAt.map { Fmt.uptime(since: $0) } ?? "running"
        case .starting: return "starting"
        case .exited: return "done"
        case .crashed: return "crashed"
        case .error: return "no folder"
        case .stopped: return ""
        }
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
                    ForEach(project.commands, id: \.address) { cmd in item(project: project, cmd: cmd) }
                    ForEach(project.groups, id: \.id) { group in
                        Menu(group.name) {
                            ForEach(group.commands, id: \.address) { cmd in item(project: project, cmd: cmd) }
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
            Label { Text(cmd.name) } icon: { Image(systemName: state.isAlive ? "circle.fill" : "circle") }
        }
    }
}
