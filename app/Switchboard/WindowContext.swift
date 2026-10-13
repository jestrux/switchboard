import Foundation
import SwiftUI

/// What a window shows: every project, or a single one.
enum WindowScope: Hashable {
    case all
    case project(String)

    var persistenceKey: String {
        switch self {
        case .all: return "main"
        case .project(let slug): return "project.\(slug)"
        }
    }
}

/// Per-window state: scope, the pane layout, which pane is focused, and the sidebar
/// selection (always the focused pane's command). One instance per open window.
@MainActor
final class WindowContext: ObservableObject, Identifiable {
    let id = UUID()
    let scope: WindowScope

    @Published private(set) var layout: PaneLayout {
        didSet { persistLayout() }
    }
    @Published private(set) var focusedPane: UUID
    /// Pane shown alone (⌘⇧↩); others stay in the tree and keep running.
    @Published var maximizedPane: UUID?
    /// Project shown in the sidebar when the rail is on (board only).
    @Published var railProject: String? {
        didSet { if let railProject { UserDefaults.standard.set(railProject, forKey: "rail.\(scope.persistenceKey)") } }
    }
    /// Measured pane sizes, for split refusal below the minimum.
    var paneSizes: [UUID: CGSize] = [:]
    /// Short-lived message shown over the pane area (refused split etc.).
    @Published var toast: String?

    /// Sidebar selection. Setting it shows the command in the focused pane, or focuses
    /// the pane that already shows it.
    @Published var selection: CommandRef? {
        didSet {
            guard !syncing, selection != oldValue else { return }
            if let sel = selection {
                if let existing = layout.leaf(showing: sel) {
                    setFocus(existing.id)
                } else {
                    layout = layout.updatingLeaf(focusedPane) { $0.ref = sel }
                }
                if scope == .all, railProject != sel.project { railProject = sel.project }
            }
        }
    }
    private var syncing = false

    init(scope: WindowScope) {
        self.scope = scope
        let key = "layout.\(scope.persistenceKey)"
        if let data = UserDefaults.standard.data(forKey: key),
           let saved = try? JSONDecoder().decode(PaneLayout.self, from: data),
           let first = saved.leaves.first {
            layout = saved
            focusedPane = first.id
            selection = first.ref
        } else {
            let l = PaneLayout.single()
            layout = l
            focusedPane = l.leaves[0].id
            selection = nil
        }
        railProject = UserDefaults.standard.string(forKey: "rail.\(scope.persistenceKey)")
        if let data = UserDefaults.standard.data(forKey: "focus.\(scope.persistenceKey)"),
           let id = try? JSONDecoder().decode(UUID.self, from: data), layout.leaf(id) != nil {
            focusedPane = id
            selection = layout.leaf(id)?.ref
        }
    }

    // MARK: Panes

    var focusedLeaf: PaneLeaf? { layout.leaf(focusedPane) }
    var canSplit: Bool { layout.leafCount < maxPanes }
    var canClosePane: Bool { layout.leafCount > 1 }

    func setFocus(_ id: UUID) {
        guard layout.leaf(id) != nil else { return }
        if focusedPane != id { focusedPane = id }
        syncing = true
        selection = layout.leaf(id)?.ref
        syncing = false
        if let ref = selection, scope == .all, railProject != ref.project { railProject = ref.project }
        if let data = try? JSONEncoder().encode(id) {
            UserDefaults.standard.set(data, forKey: "focus.\(scope.persistenceKey)")
        }
    }

    /// Why the focused pane (or `pane`) cannot be split along `axis`, or nil when it can.
    func splitRefusal(_ pane: UUID? = nil, axis: SplitAxis) -> String? {
        if layout.leafCount >= maxPanes { return "Four panes is the limit. Close a pane to split again." }
        let target = pane ?? focusedPane
        if let size = paneSizes[target] {
            let min = SBMetrics.paneMinimum
            let fits = axis == .horizontal ? size.width >= min.width * 2 + SBMetrics.dividerHit : size.height >= min.height * 2 + SBMetrics.dividerHit
            if !fits { return "Each pane needs at least \(Int(min.width)) × \(Int(min.height)) pt. Widen the window or close a pane." }
        }
        return nil
    }

    /// Split the focused pane (or `pane`) along `axis`; the new pane is empty and focused.
    func split(_ pane: UUID? = nil, axis: SplitAxis) {
        if let why = splitRefusal(pane, axis: axis) { showToast(why); return }
        maximizedPane = nil
        let target = pane ?? focusedPane
        let newLeaf = PaneLeaf(id: UUID(), ref: nil)
        layout = layout.splitting(target, axis: axis, newLeaf: newLeaf)
        setFocus(newLeaf.id)
    }

    func showToast(_ text: String) {
        toast = text
        Task { @MainActor in
            try? await Task.sleep(for: .seconds(3))
            if toast == text { toast = nil }
        }
    }

    func toggleMaximize(_ pane: UUID? = nil) {
        guard layout.leafCount > 1 else { return }
        let target = pane ?? focusedPane
        maximizedPane = maximizedPane == target ? nil : target
        setFocus(target)
    }

    func resetLayout() {
        let keep = focusedLeaf?.ref
        layout = .single(keep)
        maximizedPane = nil
        setFocus(layout.leaves[0].id)
    }

    /// Split `pane` and show `ref` in the new half; `newFirst` puts the new pane on the
    /// left/top instead of the right/bottom. Refusals show a toast like `split`.
    func splitAndShow(_ pane: UUID, axis: SplitAxis, newFirst: Bool, ref: CommandRef) {
        if let existing = layout.leaf(showing: ref), existing.id != pane {
            // Already on screen: just move it into this pane's new half by swapping later.
            layout = layout.updatingLeaf(existing.id) { $0.ref = nil }
        }
        if let why = splitRefusal(pane, axis: axis) { showToast(why); return }
        maximizedPane = nil
        let newLeaf = PaneLeaf(id: UUID(), ref: ref)
        layout = layout.splitting(pane, axis: axis, newLeaf: newLeaf, newFirst: newFirst)
        setFocus(newLeaf.id)
    }

    func closePane(_ pane: UUID? = nil) {
        let target = pane ?? focusedPane
        guard canClosePane, let next = layout.closing(target) else { return }
        if maximizedPane == target { maximizedPane = nil }
        let wasFocused = target == focusedPane
        layout = next
        if wasFocused || layout.leaf(focusedPane) == nil {
            setFocus(layout.leaves.first!.id)
        }
    }

    func show(_ ref: CommandRef?, in pane: UUID) {
        if let ref, let existing = layout.leaf(showing: ref), existing.id != pane {
            // One view per command: the two panes swap contents.
            let mine = layout.leaf(pane)?.ref
            layout = layout.updatingLeaf(existing.id) { $0.ref = mine }
        }
        layout = layout.updatingLeaf(pane) { $0.ref = ref }
        setFocus(pane)
    }

    func focusPreviousPane() {
        let all = layout.leaves
        guard let i = all.firstIndex(where: { $0.id == focusedPane }) else { return }
        setFocus(all[(i - 1 + all.count) % all.count].id)
    }

    /// Project-pinned theme/appearance apply only in that project's own window.
    func resolveLook(store: EngineStore, themeID: SBThemeID, appearance: SBAppearance) -> WindowLook {
        var look = WindowLook(theme: themeID.theme, appearance: appearance, pinnedTheme: false, pinnedAppearance: false)
        if case .project(let slug) = scope, let p = store.project(slug) {
            if let custom = CustomThemeSpec.parse(p.themeCustom) {
                look.theme = custom.resolve(baseFallback: themeID)
                look.pinnedTheme = true
            } else if let t = SBThemeID.fromConfig(p.theme) {
                look.theme = t.theme
                look.pinnedTheme = true
            }
            if let a = SBAppearance.fromConfig(p.appearance) { look.appearance = a; look.pinnedAppearance = true }
        }
        return look
    }

    /// Pane number (1-based, visual order) for a command shown in this window.
    func paneNumber(of ref: CommandRef) -> Int? {
        layout.leaves.firstIndex { $0.ref == ref }.map { $0 + 1 }
    }

    func setFraction(_ fraction: Double, of split: UUID) {
        layout = layout.updatingSplit(split, fraction: fraction)
    }

    func focusNextPane() {
        if let next = layout.nextLeaf(after: focusedPane) { setFocus(next.id) }
    }

    // MARK: Helpers

    /// Human name used in "Showing in the … window" placeholders.
    func title(store: EngineStore) -> String {
        switch scope {
        case .all: return "main"
        case .project(let slug): return store.project(slug)?.name ?? slug
        }
    }

    /// Projects visible in this window. The main board hides projects that are open in
    /// their own window.
    func projects(in store: EngineStore) -> [ProjectInfo] {
        switch scope {
        case .all: return store.projects.filter { !store.openProjectWindows.contains($0.slug) }
        case .project(let slug): return store.projects.filter { $0.slug == slug }
        }
    }

    /// Drop pane commands that no longer exist here; pick a default for an empty focused pane.
    func reconcile(with store: EngineStore) {
        let visible = projects(in: store)
        func valid(_ ref: CommandRef) -> Bool {
            visible.contains { $0.slug == ref.project } && store.command(ref) != nil
        }
        for leaf in layout.leaves {
            if let ref = leaf.ref, !valid(ref) {
                layout = layout.updatingLeaf(leaf.id) { $0.ref = nil }
            }
        }
        if focusedLeaf?.ref == nil, layout.leafCount == 1,
           let p = visible.first(where: \.isValid), let c = p.allCommands.first {
            layout = layout.updatingLeaf(focusedPane) { $0.ref = CommandRef(project: p.slug, address: c.address) }
        }
        syncing = true
        selection = focusedLeaf?.ref
        syncing = false
        if scope == .all {
            if let rp = railProject, !visible.contains(where: { $0.slug == rp }) { railProject = nil }
            if railProject == nil { railProject = selection?.project ?? visible.first?.slug }
        }
    }

    private func persistLayout() {
        if let data = try? JSONEncoder().encode(layout) {
            UserDefaults.standard.set(data, forKey: "layout.\(scope.persistenceKey)")
        }
    }
}

/// Snapshot of the focused window, read by the menu bar commands.
struct WindowFocus {
    let context: WindowContext
    let selection: CommandRef?
    let canSplit: Bool
    let canClosePane: Bool
}

struct WindowFocusKey: FocusedValueKey {
    typealias Value = WindowFocus
}

extension FocusedValues {
    var windowFocus: WindowFocus? {
        get { self[WindowFocusKey.self] }
        set { self[WindowFocusKey.self] = newValue }
    }
}
