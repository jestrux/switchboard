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
        if let data = try? JSONEncoder().encode(id) {
            UserDefaults.standard.set(data, forKey: "focus.\(scope.persistenceKey)")
        }
    }

    /// Split the focused pane (or `pane`) along `axis`; the new pane is empty and focused.
    func split(_ pane: UUID? = nil, axis: SplitAxis) {
        guard canSplit else { return }
        let target = pane ?? focusedPane
        let newLeaf = PaneLeaf(id: UUID(), ref: nil)
        layout = layout.splitting(target, axis: axis, newLeaf: newLeaf)
        setFocus(newLeaf.id)
    }

    func closePane(_ pane: UUID? = nil) {
        let target = pane ?? focusedPane
        guard canClosePane, let next = layout.closing(target) else { return }
        let wasFocused = target == focusedPane
        layout = next
        if wasFocused || layout.leaf(focusedPane) == nil {
            setFocus(layout.leaves.first!.id)
        }
    }

    func show(_ ref: CommandRef?, in pane: UUID) {
        if let ref, let existing = layout.leaf(showing: ref), existing.id != pane {
            // Only one pane per command: clear it there first.
            layout = layout.updatingLeaf(existing.id) { $0.ref = nil }
        }
        layout = layout.updatingLeaf(pane) { $0.ref = ref }
        setFocus(pane)
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
